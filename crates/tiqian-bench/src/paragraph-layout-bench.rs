use mimalloc::MiMalloc;
use std::alloc::{GlobalAlloc, Layout as AllocLayout};
use std::hint::black_box;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tiqian::api::{ParagraphLayoutEngine, ParagraphLayoutEngineBuilder};
use tiqian::core::geometry::LayoutConstraints;
use tiqian::core::layout_model::LayoutResult;
use tiqian::core::text_model::{LayoutInput, LineLengthGrid};
use tiqian::layout::line_breaker::LookaheadLineBreaker;
use tiqian::layout::width_independent_annotation_cache::{
    LruWidthIndependentAnnotationCache, WidthIndependentAnnotationCache,
    WidthIndependentAnnotationKey, WidthIndependentParagraphAnnotation,
};

/// 分配计数开关。关闭时分配器只转发，不计入任何原子操作。
static COUNT_ENABLED: AtomicBool = AtomicBool::new(false);
static ALLOC_CALLS: AtomicUsize = AtomicUsize::new(0);
static REALLOC_CALLS: AtomicUsize = AtomicUsize::new(0);
static DEALLOC_CALLS: AtomicUsize = AtomicUsize::new(0);
/// 累计请求字节：alloc/alloc_zeroed 计入自身尺寸，realloc 计入新尺寸。
static ALLOCATED_BYTES: AtomicUsize = AtomicUsize::new(0);
/// 累计释放字节：dealloc 计入自身尺寸，realloc 计入旧尺寸。
static FREED_BYTES: AtomicUsize = AtomicUsize::new(0);
/// 当前存活字节。直接按每次 alloc/dealloc 的差值维护，因此在任意时刻采样都有意义。
///
/// 进程启动时已有分配，所以这是全进程绝对值；比较时只看两点之间的差值。
/// benchmark 单线程运行，读数不受并发竞争影响。
static LIVE_BYTES: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);

/// 包装 `MiMalloc` 的计数分配器。
///
/// 只统计 Rust 全局分配器看到的分配，字体库等原生代码的直接分配不计入，
/// 因此读数不能当作进程总内存。回调内部不做格式化或分配。
struct CountingAllocator;

impl CountingAllocator {
    /// 记录一次新分配。`COUNT_ENABLED` 关闭时立即返回。
    #[inline]
    fn record_alloc(size: usize) {
        if COUNT_ENABLED.load(Ordering::Relaxed) {
            ALLOC_CALLS.fetch_add(1, Ordering::Relaxed);
            ALLOCATED_BYTES.fetch_add(size, Ordering::Relaxed);
            LIVE_BYTES.fetch_add(size as isize, Ordering::Relaxed);
        }
    }
}

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: AllocLayout) -> *mut u8 {
        Self::record_alloc(layout.size());
        unsafe { MiMalloc.alloc(layout) }
    }

    /// 必须显式转发，否则会退回 trait 默认实现并绕开 mimalloc 的零填充分配路径。
    unsafe fn alloc_zeroed(&self, layout: AllocLayout) -> *mut u8 {
        Self::record_alloc(layout.size());
        unsafe { MiMalloc.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: AllocLayout) {
        if COUNT_ENABLED.load(Ordering::Relaxed) {
            DEALLOC_CALLS.fetch_add(1, Ordering::Relaxed);
            FREED_BYTES.fetch_add(layout.size(), Ordering::Relaxed);
            LIVE_BYTES.fetch_sub(layout.size() as isize, Ordering::Relaxed);
        }
        unsafe { MiMalloc.dealloc(pointer, layout) }
    }

    unsafe fn realloc(
        &self,
        pointer: *mut u8,
        layout: AllocLayout,
        new_size: usize,
    ) -> *mut u8 {
        let counting = COUNT_ENABLED.load(Ordering::Relaxed);
        if counting {
            REALLOC_CALLS.fetch_add(1, Ordering::Relaxed);
        }
        let result = unsafe { MiMalloc.realloc(pointer, layout, new_size) };
        // 只在 realloc 成功时调整存活字节；失败时旧分配仍然有效。
        if counting && !result.is_null() {
            ALLOCATED_BYTES.fetch_add(new_size, Ordering::Relaxed);
            FREED_BYTES.fetch_add(layout.size(), Ordering::Relaxed);
            LIVE_BYTES.fetch_add(
                new_size as isize - layout.size() as isize,
                Ordering::Relaxed,
            );
        }
        result
    }
}

#[global_allocator]
static GLOBAL: CountingAllocator = CountingAllocator;

/// 分配计数的一次快照。字节为累计请求量，不是物理内存占用。
#[derive(Clone, Copy, Default, PartialEq, Eq)]
struct AllocSnapshot {
    alloc_calls: usize,
    realloc_calls: usize,
    dealloc_calls: usize,
    allocated_bytes: usize,
    freed_bytes: usize,
    /// 全进程存活字节绝对值。
    live_bytes: isize,
}

impl AllocSnapshot {
    fn read() -> Self {
        Self {
            alloc_calls: ALLOC_CALLS.load(Ordering::Relaxed),
            realloc_calls: REALLOC_CALLS.load(Ordering::Relaxed),
            dealloc_calls: DEALLOC_CALLS.load(Ordering::Relaxed),
            allocated_bytes: ALLOCATED_BYTES.load(Ordering::Relaxed),
            freed_bytes: FREED_BYTES.load(Ordering::Relaxed),
            live_bytes: LIVE_BYTES.load(Ordering::Relaxed),
        }
    }

    /// 用一个窗口的起止快照求差，所有计数都按差值表达，避免跨窗口混算。
    fn since(self, earlier: Self) -> Self {
        Self {
            alloc_calls: self.alloc_calls.saturating_sub(earlier.alloc_calls),
            realloc_calls: self.realloc_calls.saturating_sub(earlier.realloc_calls),
            dealloc_calls: self.dealloc_calls.saturating_sub(earlier.dealloc_calls),
            allocated_bytes: self.allocated_bytes.saturating_sub(earlier.allocated_bytes),
            freed_bytes: self.freed_bytes.saturating_sub(earlier.freed_bytes),
            live_bytes: self.live_bytes,
        }
    }

    /// 窗口内尚未释放的请求字节。跨窗口的先行分配被释放时会被扣除，
    /// 因此这是窗口内的下界估计，不是进程存活内存。
    fn window_live_bytes(self) -> usize {
        self.allocated_bytes.saturating_sub(self.freed_bytes)
    }

    fn accumulate(&mut self, later: Self) {
        self.alloc_calls += later.alloc_calls;
        self.realloc_calls += later.realloc_calls;
        self.dealloc_calls += later.dealloc_calls;
        self.allocated_bytes += later.allocated_bytes;
        self.freed_bytes += later.freed_bytes;
    }
}

use tiqian_demo::font_backend;
use tiqian_demo::sample;

struct Options {
    replay: bool,
    cluster_queries: bool,
    iterations: usize,
    warmup: usize,
    widths: Vec<f32>,
    scale: f32,
    strategy: String,
    /// 计数模式：统计分配，不再采信耗时。
    count_allocations: bool,
    /// 每个样本重建引擎，同时清空 annotation 缓存（字体装载不计入计时）。
    fresh_engine: bool,
    /// 每个样本清空 annotation 缓存，但保留引擎与 backend。
    annotation_cold: bool,
}

impl Options {
    fn parse() -> Result<Option<Self>, String> {
        let mut options = Self {
            replay: false,
            cluster_queries: false,
            iterations: 200,
            warmup: 20,
            widths: vec![672.0, 360.0, 960.0],
            scale: 1.0,
            strategy: "greedy".to_owned(),
            count_allocations: false,
            fresh_engine: false,
            annotation_cold: false,
        };
        let mut args = std::env::args().skip(1);
        while let Some(flag) = args.next() {
            if flag == "--replay" {
                options.replay = true;
                continue;
            }
            if flag == "--cluster-queries" {
                options.cluster_queries = true;
                continue;
            }
            if flag == "--count-allocations" {
                options.count_allocations = true;
                continue;
            }
            if flag == "--fresh-engine" {
                options.fresh_engine = true;
                continue;
            }
            if flag == "--annotation-cold" {
                options.annotation_cold = true;
                continue;
            }
            if flag == "--live-bytes" {
                // 兼容旧参数：存活字节现在在计数模式下总是记录。
                continue;
            }
            if flag == "--help" || flag == "-h" {
                println!(
                    "paragraph-layout-bench [--replay] [--cluster-queries] [--count-allocations] [--fresh-engine] [--annotation-cold] [--iterations 200] [--warmup 20] [--widths 672,360,960] [--scale 1] [--strategy greedy|lookahead]"
                );
                println!("One iteration visits every width in order. Widths are physical pixels.");
                println!(
                    "Cluster queries time rich_text_layer_cluster_segments and decoration_cluster_segments per page."
                );
                println!(
                    "Allocation counting wraps mimalloc and covers engine.layout calls only; run it separately from timing."
                );
                println!(
                    "--fresh-engine rebuilds the engine per sample; --annotation-cold clears only the annotation cache."
                );
                println!(
                    "Both are separate from dynamic shaping, which narrow widths and technical spans can still trigger."
                );
                return Ok(None);
            }
            if !matches!(
                flag.as_str(),
                "--iterations" | "--warmup" | "--widths" | "--scale" | "--strategy"
            ) {
                return Err(format!("unknown option: {flag}"));
            }
            let value = args
                .next()
                .ok_or_else(|| format!("missing value for {flag}"))?;
            match flag.as_str() {
                "--iterations" => {
                    options.iterations = value.parse().map_err(|_| "invalid iterations")?
                }
                "--warmup" => options.warmup = value.parse().map_err(|_| "invalid warmup")?,
                "--widths" => {
                    options.widths = value
                        .split(',')
                        .map(|width| {
                            width
                                .parse::<f32>()
                                .map_err(|_| format!("invalid width: {width}"))
                        })
                        .collect::<Result<_, _>>()?;
                }
                "--scale" => options.scale = value.parse().map_err(|_| "invalid scale")?,
                "--strategy" => options.strategy = value,
                _ => unreachable!(),
            }
        }
        if options.iterations == 0 {
            return Err("iterations must be positive".to_owned());
        }
        if !options.scale.is_finite()
            || options.scale <= 0.0
            || options
                .widths
                .iter()
                .any(|width| !width.is_finite() || *width <= 0.0)
        {
            return Err("widths and scale must be finite and positive".to_owned());
        }
        if !matches!(options.strategy.as_str(), "greedy" | "lookahead") {
            return Err("strategy must be greedy or lookahead".to_owned());
        }
        if options.fresh_engine && options.annotation_cold {
            return Err(
                "--fresh-engine already discards the annotation cache; do not combine both"
                    .to_owned(),
            );
        }
        Ok(Some(options))
    }
}

/// 记录每次 annotation cache 查询结果，用于区分命中与未命中。
#[derive(Default)]
struct CacheCounters {
    hits: usize,
    misses: usize,
    puts: usize,
}

impl CacheCounters {
    fn snapshot(&self) -> (usize, usize, usize) {
        (self.hits, self.misses, self.puts)
    }
}

/// 可被 benchmark 从外部清空的 cache 状态。
///
/// 引擎拥有 cache，benchmark 无法直接访问；`--annotation-cold` 需要在不重建引擎的前提下清空，
/// 因此让包装层共享一份状态句柄。这属于 benchmark 内部实现，生产 API 不变。
#[derive(Default)]
struct SharedCacheState {
    inner: LruWidthIndependentAnnotationCache,
    counters: CacheCounters,
}

/// 包装真实 cache 的统计与清空层。
struct CountingAnnotationCache {
    state: Arc<std::sync::Mutex<SharedCacheState>>,
}

impl WidthIndependentAnnotationCache for CountingAnnotationCache {
    fn get(
        &mut self,
        key: &WidthIndependentAnnotationKey,
    ) -> Option<Arc<WidthIndependentParagraphAnnotation>> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let value = state.inner.get(key);
        if value.is_some() {
            state.counters.hits += 1;
        } else {
            state.counters.misses += 1;
        }
        value
    }

    fn put(
        &mut self,
        key: WidthIndependentAnnotationKey,
        annotation: Arc<WidthIndependentParagraphAnnotation>,
    ) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.counters.puts += 1;
        state.inner.put(key, annotation)
    }

    fn clear(&mut self) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.inner.clear()
    }

    fn size(&self) -> usize {
        let state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.inner.size()
    }
}

/// 清空共享 cache 并返回清理前的大小。
fn clear_annotation_cache(state: &Arc<std::sync::Mutex<SharedCacheState>>) -> usize {
    let mut state = state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let size = state.inner.size();
    state.inner.clear();
    size
}

/// 读取 cache 计数快照；`--annotation-cold` 清空时不重置它，以便观测全程累计值。
fn cache_counters(state: &Arc<std::sync::Mutex<SharedCacheState>>) -> (usize, usize, usize) {
    let state = state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    state.counters.snapshot()
}

#[derive(Default)]
struct PageMeasurement {
    replay: Duration,
    drop_replay: Duration,
    cluster_queries: Duration,
    cluster_segments: usize,
    layout: Duration,
    drop_results: Duration,
    total: Duration,
    calls: usize,
    lines: usize,
    clusters: usize,
    glyphs: usize,
    /// 仅计数窗口内的分配统计。
    allocations: AllocSnapshot,
    /// `--live-bytes`：结果仍存活时全进程存活字节的绝对值。
    live_bytes: isize,
    /// 各 decision 向量的记录数，用于把分配量按记录数比较。
    debug_records: DebugRecordCounts,
    cache_hits: usize,
    cache_misses: usize,
    cache_puts: usize,
}

/// 布局结果携带的结构化 decision 数量。
///
/// 覆盖审计涉及的全部集合，不只限于修复相关的几类。
#[derive(Clone, Copy, Default, PartialEq, Eq)]
struct DebugRecordCounts {
    font: usize,
    shaping: usize,
    metric: usize,
    punctuation: usize,
    geometry: usize,
    spacing: usize,
    role_override: usize,
    line: usize,
    justification: usize,
    /// 嵌套在 justification decisions 里的 allocation 总数。
    justification_allocations: usize,
    auto_space: usize,
    line_edge_trim: usize,
    decoration: usize,
    decoration_segment: usize,
    ruby: usize,
    bopomofo: usize,
    mandatory_break: usize,
    contextual_kinsoku: usize,
    inline_box: usize,
    inline_object: usize,
    zero_width_break: usize,
    break_opportunity: usize,
    emergency_tracking: usize,
    line_white_space: usize,
    white_space_unit: usize,
    /// 所有集合的记录总数。
    total: usize,
}

impl DebugRecordCounts {
    /// 诊断记录关闭时没有可统计的对象，返回全零。
    #[cfg(not(feature = "debug-info"))]
    fn of(_result: &LayoutResult) -> Self {
        Self::default()
    }

    #[cfg(feature = "debug-info")]
    fn of(result: &LayoutResult) -> Self {
        let debug = &result.debug;
        let justification_allocations = debug
            .justification_decisions
            .iter()
            .map(|decision| decision.allocations.len())
            .sum();
        let total = debug.font_decisions.len()
            + debug.shaping_decisions.len()
            + debug.metric_decisions.len()
            + debug.punctuation_decisions.len()
            + debug.geometry_decisions.len()
            + debug.spacing_decisions.len()
            + debug.role_overrides.len()
            + debug.line_decisions.len()
            + debug.justification_decisions.len()
            + justification_allocations
            + debug.auto_space_decisions.len()
            + debug.line_edge_trim_decisions.len()
            + debug.decoration_decisions.len()
            + debug.decoration_segments.len()
            + debug.ruby_decisions.len()
            + debug.bopomofo_decisions.len()
            + debug.mandatory_break_decisions.len()
            + debug.contextual_kinsoku_decisions.len()
            + debug.inline_box_decisions.len()
            + debug.inline_object_decisions.len()
            + debug.zero_width_break_decisions.len()
            + debug.break_opportunity_decisions.len()
            + debug.emergency_tracking_eligibility_decisions.len()
            + debug.line_white_space_adjustments.len()
            + debug.white_space_units.len();
        Self {
            font: debug.font_decisions.len(),
            shaping: debug.shaping_decisions.len(),
            metric: debug.metric_decisions.len(),
            punctuation: debug.punctuation_decisions.len(),
            geometry: debug.geometry_decisions.len(),
            spacing: debug.spacing_decisions.len(),
            role_override: debug.role_overrides.len(),
            line: debug.line_decisions.len(),
            justification: debug.justification_decisions.len(),
            justification_allocations,
            auto_space: debug.auto_space_decisions.len(),
            line_edge_trim: debug.line_edge_trim_decisions.len(),
            decoration: debug.decoration_decisions.len(),
            decoration_segment: debug.decoration_segments.len(),
            ruby: debug.ruby_decisions.len(),
            bopomofo: debug.bopomofo_decisions.len(),
            mandatory_break: debug.mandatory_break_decisions.len(),
            contextual_kinsoku: debug.contextual_kinsoku_decisions.len(),
            inline_box: debug.inline_box_decisions.len(),
            inline_object: debug.inline_object_decisions.len(),
            zero_width_break: debug.zero_width_break_decisions.len(),
            break_opportunity: debug.break_opportunity_decisions.len(),
            emergency_tracking: debug.emergency_tracking_eligibility_decisions.len(),
            line_white_space: debug.line_white_space_adjustments.len(),
            white_space_unit: debug.white_space_units.len(),
            total,
        }
    }

    fn accumulate(&mut self, later: Self) {
        self.font += later.font;
        self.shaping += later.shaping;
        self.metric += later.metric;
        self.punctuation += later.punctuation;
        self.geometry += later.geometry;
        self.spacing += later.spacing;
        self.role_override += later.role_override;
        self.line += later.line;
        self.justification += later.justification;
        self.justification_allocations += later.justification_allocations;
        self.auto_space += later.auto_space;
        self.line_edge_trim += later.line_edge_trim;
        self.decoration += later.decoration;
        self.decoration_segment += later.decoration_segment;
        self.ruby += later.ruby;
        self.bopomofo += later.bopomofo;
        self.mandatory_break += later.mandatory_break;
        self.contextual_kinsoku += later.contextual_kinsoku;
        self.inline_box += later.inline_box;
        self.inline_object += later.inline_object;
        self.zero_width_break += later.zero_width_break;
        self.break_opportunity += later.break_opportunity;
        self.emergency_tracking += later.emergency_tracking;
        self.line_white_space += later.line_white_space;
        self.white_space_unit += later.white_space_unit;
        self.total += later.total;
    }

    fn summary(&self) -> String {
        format!(
            "total={} font={} shaping={} metric={} punct={} geom={} spacing={} role={} line={} justify={}+{} autospace={} trim={} deco={} deco_seg={} ruby={} bopomofo={} mandatory={} kinsoku={} box={} object={} zwb={} break={} tracking={} line_ws={} ws_unit={}",
            self.total,
            self.font,
            self.shaping,
            self.metric,
            self.punctuation,
            self.geometry,
            self.spacing,
            self.role_override,
            self.line,
            self.justification,
            self.justification_allocations,
            self.auto_space,
            self.line_edge_trim,
            self.decoration,
            self.decoration_segment,
            self.ruby,
            self.bopomofo,
            self.mandatory_break,
            self.contextual_kinsoku,
            self.inline_box,
            self.inline_object,
            self.zero_width_break,
            self.break_opportunity,
            self.emergency_tracking,
            self.line_white_space,
            self.white_space_unit
        )
    }
}

/// 一次 `engine.layout()` 调用。
///
/// 计时与计数始终按同一开关状态进行，保证两者的覆盖范围一致：
/// 计数模式下不采信耗时，但 `layout` 仍累计同一段区间，便于确认两次运行的调用工作量相同。
fn timed_layout(
    engine: &mut ParagraphLayoutEngine,
    input: LayoutInput,
    measurement: &mut PageMeasurement,
    counting: bool,
) -> LayoutResult {
    let start = Instant::now();
    let before = if counting {
        Some(AllocSnapshot::read())
    } else {
        None
    };
    let result = engine.layout(black_box(input));
    if let Some(before) = before {
        measurement.allocations.accumulate(AllocSnapshot::read().since(before));
    }
    measurement.layout += start.elapsed();
    measurement.calls += 1;
    black_box(result)
}

/// marker 页只用于测量 gutter 宽度，因此从 replay、cluster query 与分配统计中一并排除，
/// 与 `calls`、`layout` 的既有处理保持一致。
fn measure_page(
    engine: &mut ParagraphLayoutEngine,
    width: f32,
    scale: f32,
    options: &Options,
    cache_state: &Arc<std::sync::Mutex<SharedCacheState>>,
) -> PageMeasurement {
    let start = Instant::now();
    let document = sample::build_document_demo(width, scale);
    let mut results = Vec::with_capacity(document.blocks.len() * 3);
    let mut measurement = PageMeasurement::default();
    let mut measurement_only = std::collections::HashSet::new();
    let counting = options.count_allocations;
    for block in document.blocks {
        match block {
            sample::DemoDocumentDemoBlock::Paragraph(document) => {
                results.push(timed_layout(
                    engine,
                    document.input,
                    &mut measurement,
                    counting,
                ));
            }
            sample::DemoDocumentDemoBlock::NarrowParagraph {
                mut document,
                max_width,
            } => {
                document.input.constraints = LayoutConstraints::with_defaults(max_width);
                results.push(timed_layout(
                    engine,
                    document.input,
                    &mut measurement,
                    counting,
                ));
            }
            sample::DemoDocumentDemoBlock::ListItem {
                mut marker,
                mut body,
            } => {
                let font_size = body.input.text_style.font_size;
                let mut marker_input = marker.input.clone();
                marker_input.paragraph_style.line_length_grid = LineLengthGrid::with_enabled(false);
                marker_input.constraints = LayoutConstraints::with_defaults(100_000.0);
                let marker_measurement =
                    timed_layout(engine, marker_input, &mut measurement, counting);
                let gutter =
                    (marker_measurement.size.width / font_size).ceil().max(1.0) * font_size;
                measurement_only.insert(results.len());
                results.push(marker_measurement);
                marker.input.constraints = LayoutConstraints::with_defaults(gutter);
                body.input.constraints =
                    LayoutConstraints::with_defaults((width - gutter).max(1.0));
                results.push(timed_layout(
                    engine,
                    marker.input,
                    &mut measurement,
                    counting,
                ));
                results.push(timed_layout(
                    engine,
                    body.input,
                    &mut measurement,
                    counting,
                ));
            }
            sample::DemoDocumentDemoBlock::Section { .. } => {}
        }
    }
    if counting {
        // 全进程存活字节绝对值；只比较两个时点之间的差值。
        measurement.live_bytes = AllocSnapshot::read().live_bytes;
    }
    for result in black_box(&results) {
        measurement.lines += result.lines.len();
        measurement.clusters += result.clusters.len();
        measurement.glyphs += result
            .glyph_runs
            .iter()
            .map(|run| run.glyphs.len())
            .sum::<usize>();
        if counting {
            measurement
                .debug_records
                .accumulate(DebugRecordCounts::of(result));
        }
    }
    if options.replay {
        let replay_start = Instant::now();
        let indices: Vec<_> = results
            .iter()
            .enumerate()
            .filter(|(index, _)| !measurement_only.contains(index))
            .map(|(_, result)| {
                tiqian::core::layout_result_replay_index::to_replay_index(black_box(result))
            })
            .collect();
        measurement.replay = replay_start.elapsed();
        let drop_start = Instant::now();
        drop(black_box(indices));
        measurement.drop_replay = drop_start.elapsed();
    }
    if options.cluster_queries {
        let query_start = Instant::now();
        let mut segment_count = 0;
        for (index, result) in results.iter().enumerate() {
            if measurement_only.contains(&index) {
                continue;
            }
            segment_count += black_box(result.rich_text_layer_cluster_segments()).len();
            segment_count += black_box(result.decoration_cluster_segments()).len();
        }
        measurement.cluster_queries = query_start.elapsed();
        measurement.cluster_segments = segment_count;
    }
    let drop_start = Instant::now();
    drop(black_box(results));
    measurement.drop_results = drop_start.elapsed();
    // cache 计数是全程累计值，在页面结束时读取，供按页比较缓存路径。
    let (hits, misses, puts) = cache_counters(cache_state);
    measurement.cache_hits = hits;
    measurement.cache_misses = misses;
    measurement.cache_puts = puts;
    measurement.total = start.elapsed();
    measurement
}

fn print_stats(label: &str, values: impl Iterator<Item = Duration>) {
    let mut values: Vec<_> = values.map(|value| value.as_secs_f64() * 1000.0).collect();
    values.sort_by(f64::total_cmp);
    let n = values.len();
    let median = if n % 2 == 0 {
        (values[n / 2 - 1] + values[n / 2]) / 2.0
    } else {
        values[n / 2]
    };
    let p95 = values[(n * 95).div_ceil(100) - 1];
    println!(
        "{label:<16} n={n:<5} min={:.3} median={median:.3} p95={p95:.3} mean={:.3} max={:.3} ms",
        values[0],
        values.iter().sum::<f64>() / n as f64,
        values[n - 1]
    );
}

fn main() -> Result<(), String> {
    let Some(options) = Options::parse()? else {
        return Ok(());
    };
    if cfg!(debug_assertions) {
        eprintln!("Warning: debug build; use --release for performance comparisons.");
    }
    if options.count_allocations && options.warmup < 1 {
        eprintln!(
            "Warning: allocation counting without warmup includes first-use initialization in every sample."
        );
    }
    // 计数模式下布局时间不受采信，但仍避免在计数回调内有耗时分支。
    COUNT_ENABLED.store(options.count_allocations, Ordering::Relaxed);
    let start = Instant::now();
    let catalog = font_backend::DemoFontCatalog::load()?;
    catalog.validate_demo_faces()?;
    let cache_state = Arc::new(std::sync::Mutex::new(SharedCacheState::default()));
    let build_engine = |state: &Arc<std::sync::Mutex<SharedCacheState>>| {
        let builder = ParagraphLayoutEngineBuilder::new(Box::new(catalog.clone()))
            .annotation_cache(Box::new(CountingAnnotationCache {
                state: state.clone(),
            }));
        if options.strategy == "lookahead" {
            builder
                .line_breaker(Box::new(LookaheadLineBreaker::default()))
                .build()
        } else {
            builder.build()
        }
    };
    let mut engine = build_engine(&cache_state);
    // fresh-engine 每次重建引擎时换上一份全新的 cache 状态，否则缓存会跨样本复用。
    let mut fresh_state = cache_state.clone();
    println!(
        "font/engine setup: {:.3} ms",
        start.elapsed().as_secs_f64() * 1000.0
    );
    println!(
        "widths={:?} scale={} strategy={} warmup={} iterations={} profile={} arch={}",
        options.widths,
        options.scale,
        options.strategy,
        options.warmup,
        options.iterations,
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        },
        std::env::consts::ARCH
    );
    println!(
        "Times are per sample page, not per paragraph. Layout includes full debug output and font backend calls."
    );
    println!(
        "Total includes input preparation, layout, output counting and result drop; no GUI or rendering."
    );
    if options.replay {
        println!(
            "Replay enabled: total also includes replay construction and drop, excluding temporary marker measurements."
        );
    }
    if options.cluster_queries {
        println!(
            "Cluster queries enabled: rich_text_layer_cluster_segments and decoration_cluster_segments run after each page layout."
        );
    }
    if options.count_allocations {
        println!(
            "Allocation counting enabled: counters wrap mimalloc and cover engine.layout only. Times in this run are not comparable."
        );
        println!(
            "Bytes are cumulative requested sizes per page; live bytes are a window lower bound, not resident memory."
        );
    }
    if options.fresh_engine {
        println!(
            "Fresh engine per sample: annotation cache is empty for every sample; font loading is excluded from timing."
        );
    } else if options.annotation_cold {
        println!(
            "Annotation cold per sample: only the annotation cache is cleared; backend and font state stay warm."
        );
    }
    println!("First sequence (shared engine; later widths may reuse caches):");
    let mut expected = Vec::new();
    for &width in &options.widths {
        let page = measure_page(&mut engine, width, options.scale, &options, &cache_state);
        println!(
            "width={width} layout={:.3} drop={:.3} total={:.3} ms calls={} lines={} clusters={} body_glyphs={} cluster_segments={}",
            page.layout.as_secs_f64() * 1000.0,
            page.drop_results.as_secs_f64() * 1000.0,
            page.total.as_secs_f64() * 1000.0,
            page.calls,
            page.lines,
            page.clusters,
            page.glyphs,
            page.cluster_segments
        );
        if options.count_allocations {
            println!(
                "  alloc alloc_calls={} realloc_calls={} allocated_bytes={} freed_bytes={} window_live_bytes={} process_live_bytes={} cache_hits={} cache_misses={} cache_puts={} debug_records={}",
                page.allocations.alloc_calls,
                page.allocations.realloc_calls,
                page.allocations.allocated_bytes,
                page.allocations.freed_bytes,
                page.allocations.window_live_bytes(),
                page.live_bytes,
                page.cache_hits,
                page.cache_misses,
                page.cache_puts,
                page.debug_records.summary()
            );
        }
        expected.push((
            page.calls,
            page.lines,
            page.clusters,
            page.glyphs,
            page.cluster_segments,
        ));
    }
    for _ in 0..options.warmup {
        for &width in &options.widths {
            if options.fresh_engine {
                fresh_state = Arc::new(std::sync::Mutex::new(SharedCacheState::default()));
                engine = build_engine(&fresh_state);
            } else if options.annotation_cold {
                clear_annotation_cache(&cache_state);
            }
            black_box(measure_page(
                &mut engine,
                width,
                options.scale,
                &options,
                &fresh_state,
            ));
        }
    }
    let mut samples: Vec<Vec<PageMeasurement>> = options
        .widths
        .iter()
        .map(|_| Vec::with_capacity(options.iterations))
        .collect();
    for _ in 0..options.iterations {
        for (index, &width) in options.widths.iter().enumerate() {
            if options.fresh_engine {
                fresh_state = Arc::new(std::sync::Mutex::new(SharedCacheState::default()));
                engine = build_engine(&fresh_state);
            } else if options.annotation_cold {
                clear_annotation_cache(&cache_state);
            }
            let page = measure_page(
                &mut engine,
                width,
                options.scale,
                &options,
                &fresh_state,
            );
            if (
                page.calls,
                page.lines,
                page.clusters,
                page.glyphs,
                page.cluster_segments,
            ) != expected[index]
            {
                return Err(format!(
                    "layout workload changed after warmup at width {width}"
                ));
            }
            samples[index].push(page);
        }
    }
    for (index, &width) in options.widths.iter().enumerate() {
        println!("Measured width={width}:");
        print_stats("layout", samples[index].iter().map(|page| page.layout));
        if options.replay {
            print_stats("replay", samples[index].iter().map(|page| page.replay));
            print_stats(
                "replay_drop",
                samples[index].iter().map(|page| page.drop_replay),
            );
        }
        if options.cluster_queries {
            print_stats(
                "cluster_query",
                samples[index].iter().map(|page| page.cluster_queries),
            );
        }
        print_stats(
            "result_drop",
            samples[index].iter().map(|page| page.drop_results),
        );
        print_stats("total", samples[index].iter().map(|page| page.total));
        if options.count_allocations {
            print_alloc_stats(&format!("alloc@{width}"), samples[index].iter());
        }
    }
    println!("All measured pages:");
    print_stats("layout", samples.iter().flatten().map(|page| page.layout));
    if options.cluster_queries {
        print_stats(
            "cluster_query",
            samples.iter().flatten().map(|page| page.cluster_queries),
        );
    }
    print_stats(
        "result_drop",
        samples.iter().flatten().map(|page| page.drop_results),
    );
    print_stats("total", samples.iter().flatten().map(|page| page.total));
    if options.count_allocations {
        // 记录数与记录的工作量绑定，因此按整页样本打印逐页中位数再乘以调用次数做归一。
        print_alloc_stats("alloc(all)", samples.iter().flatten());
        let calls: usize = samples.iter().flatten().map(|page| page.calls).sum();
        if calls > 0 {
            let records: usize = samples
                .iter()
                .flatten()
                .map(|page| page.debug_records.total)
                .sum();
            println!(
                "debug records over all samples: total={records} per_call={:.2}",
                records as f64 / calls as f64
            );
        }
    }
    if options.annotation_cold || options.fresh_engine {
        // fresh-engine 只统计最后一个引擎的状态，跨样本累计无意义。
        let state = &fresh_state;
        let (hits, misses, puts) = cache_counters(state);
        println!("annotation cache: hits={hits} misses={misses} puts={puts}");
    }
    println!(
        "Measured layout calls: {}",
        samples
            .iter()
            .flatten()
            .map(|page| page.calls)
            .sum::<usize>()
    );
    Ok(())
}

/// 打印计数统计。取中位数与均值，避免单页离群值影响判断。
fn print_alloc_stats<'a>(label: &str, pages: impl Iterator<Item = &'a PageMeasurement>) {
    let mut alloc_calls = Vec::new();
    let mut realloc_calls = Vec::new();
    let mut allocated = Vec::new();
    let mut window_live = Vec::new();
    let mut live = Vec::new();
    let mut records = Vec::new();
    for page in pages {
        alloc_calls.push(page.allocations.alloc_calls);
        realloc_calls.push(page.allocations.realloc_calls);
        allocated.push(page.allocations.allocated_bytes);
        window_live.push(page.allocations.window_live_bytes());
        live.push(page.live_bytes);
        records.push(page.debug_records.total);
    }
    if alloc_calls.is_empty() {
        return;
    }
    println!(
        "{label:<16} n={:<5} alloc_calls median={} mean={:.1} realloc_calls median={} bytes median={} mean={:.0} window_live median={} process_live median={} records median={}",
        alloc_calls.len(),
        median_usize(&mut alloc_calls),
        mean_usize(&alloc_calls),
        median_usize(&mut realloc_calls),
        median_usize(&mut allocated),
        mean_usize(&allocated),
        median_usize(&mut window_live),
        median_isize(&mut live),
        median_usize(&mut records)
    );
}

fn median_usize(values: &mut [usize]) -> usize {
    values.sort_unstable();
    let n = values.len();
    if n % 2 == 0 {
        (values[n / 2 - 1] + values[n / 2]) / 2
    } else {
        values[n / 2]
    }
}

fn median_isize(values: &mut [isize]) -> isize {
    values.sort_unstable();
    let n = values.len();
    if n % 2 == 0 {
        (values[n / 2 - 1] + values[n / 2]) / 2
    } else {
        values[n / 2]
    }
}

fn mean_usize(values: &[usize]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<usize>() as f64 / values.len() as f64
}
