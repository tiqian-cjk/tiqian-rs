use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tiqian::common::HashMap;

use tiqian::core::geometry::{text_range, LayoutConstraints};
use tiqian::core::text::Text;
use tiqian::core::text_model::{
    DecorationKind, DecorationSpan, InlineBoxSpan, LayoutInput, ParagraphStyle, RubySpan,
    TextStyle, TiqianTextContent,
};
use tiqian::core::units::Ic;
use tiqian::layout::paragraph_layout_engine::{
    ExplainableStubParagraphLayoutEngine, ParagraphLayoutEngine,
};
use tiqian::layout::width_independent_annotation_cache::{
    LruWidthIndependentAnnotationCache, WidthIndependentAnnotationCache,
    WidthIndependentAnnotationKey, WidthIndependentParagraphAnnotation,
    to_width_independent_annotation_key,
};
use tiqian::shaping::text_shaper::{
    ExplainableStubTextShaper, ShapingInput, ShapingResult, TextShaper,
};

struct CountingTextShaper {
    count: Arc<AtomicUsize>,
}

impl TextShaper for CountingTextShaper {
    fn shape(&self, input: &ShapingInput) -> ShapingResult {
        self.count.fetch_add(1, Ordering::SeqCst);
        ExplainableStubTextShaper.shape(input)
    }
}

struct SharedLruCache {
    entries: Arc<Mutex<LruWidthIndependentAnnotationCache>>,
}

impl WidthIndependentAnnotationCache for SharedLruCache {
    fn get(
        &mut self,
        key: &WidthIndependentAnnotationKey,
    ) -> Option<Arc<WidthIndependentParagraphAnnotation>> {
        self.entries.lock().unwrap().get(key)
    }

    fn put(
        &mut self,
        key: WidthIndependentAnnotationKey,
        annotation: Arc<WidthIndependentParagraphAnnotation>,
    ) {
        self.entries.lock().unwrap().put(key, annotation);
    }

    fn clear(&mut self) {
        self.entries.lock().unwrap().clear();
    }

    fn size(&self) -> usize {
        self.entries.lock().unwrap().size()
    }
}

struct DisabledCache;

impl WidthIndependentAnnotationCache for DisabledCache {
    fn get(
        &mut self,
        _: &WidthIndependentAnnotationKey,
    ) -> Option<Arc<WidthIndependentParagraphAnnotation>> {
        None
    }

    fn put(
        &mut self,
        _: WidthIndependentAnnotationKey,
        _: Arc<WidthIndependentParagraphAnnotation>,
    ) {
    }

    fn clear(&mut self) {}

    fn size(&self) -> usize {
        0
    }
}

fn input(text: &str, width: f32) -> LayoutInput {
    LayoutInput::builder(
        TiqianTextContent::new(Text::from(text)),
        LayoutConstraints::with_defaults(width),
    )
    .paragraph_style(
        ParagraphStyle::builder()
            .first_line_indent(Some(Ic::ZERO))
            .build(),
    )
    .build()
}

#[test]
fn relayout_at_three_widths_hits_annotation_cache_without_reshaping() {
    let calls = Arc::new(AtomicUsize::new(0));
    let entries = Arc::new(Mutex::new(LruWidthIndependentAnnotationCache::new(64)));
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.text_shaper = Box::new(CountingTextShaper {
        count: calls.clone(),
    });
    engine.annotation_cache = Box::new(SharedLruCache {
        entries: entries.clone(),
    });
    let normal = input("提椠是一个面向中文正文的 CJK 段落布局引擎。", 300.0);

    let normal_result = engine.layout(normal.clone());
    let initial_calls = calls.load(Ordering::SeqCst);
    assert!(initial_calls > 0);
    assert_eq!(1, entries.lock().unwrap().size());

    let narrow_result = engine.layout(input(normal.content.text.as_str(), 180.0));
    let wide_result = engine.layout(input(normal.content.text.as_str(), 500.0));
    assert_eq!(initial_calls, calls.load(Ordering::SeqCst));
    assert!(narrow_result.lines.len() >= normal_result.lines.len());
    assert!(normal_result.lines.len() >= wide_result.lines.len());
}

#[test]
fn cache_key_distinguishes_text_style_decoration_ruby_and_inline_box() {
    let entries = Arc::new(Mutex::new(LruWidthIndependentAnnotationCache::new(64)));
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.annotation_cache = Box::new(SharedLruCache {
        entries: entries.clone(),
    });
    let base = input("中西混合排版与测试文本。", 300.0);

    engine.layout(base.clone());
    engine.layout(input("中西混合排版与变动文本。", 300.0));
    let mut font_changed = base.clone();
    font_changed.text_style = TextStyle::builder().font_size(24.0).build();
    engine.layout(font_changed);
    let mut emphasis_changed = base.clone();
    emphasis_changed.decorations = vec![DecorationSpan {
        range: text_range(0, 4),
        kind: DecorationKind::Emphasis,
    }];
    engine.layout(emphasis_changed);
    let mut ruby_changed = base.clone();
    ruby_changed.ruby_spans = vec![RubySpan::new(text_range(0, 2), Text::from("zhōngxī"))];
    engine.layout(ruby_changed);
    let mut inline_box_changed = base;
    inline_box_changed.inline_boxes =
        vec![InlineBoxSpan::with_edges(text_range(2, 4), 4.0, 4.0)];
    engine.layout(inline_box_changed);

    assert_eq!(6, entries.lock().unwrap().size());
}

#[test]
fn lru_refreshes_accessed_entry_before_evicting_least_recently_used_entry() {
    let entries = Arc::new(Mutex::new(LruWidthIndependentAnnotationCache::new(2)));
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.annotation_cache = Box::new(SharedLruCache {
        entries: entries.clone(),
    });
    let first = input("段落一文本内容", 300.0);
    let second = input("段落二文本内容", 300.0);
    let third = input("段落三文本内容", 300.0);
    let first_key = to_width_independent_annotation_key(&first, HashMap::new());
    let second_key = to_width_independent_annotation_key(&second, HashMap::new());
    let third_key = to_width_independent_annotation_key(&third, HashMap::new());

    engine.layout(first);
    engine.layout(second);
    {
        let mut cache = entries.lock().unwrap();
        assert!(cache.get(&first_key).is_some());
    }
    engine.layout(third);
    let mut cache = entries.lock().unwrap();
    assert_eq!(2, cache.size());
    assert!(cache.get(&third_key).is_some());
    assert!(cache.get(&first_key).is_some());
    assert!(cache.get(&second_key).is_none());
}

#[test]
fn lru_cache_evicts_oldest_entries_when_capacity_exceeded() {
    let entries = Arc::new(Mutex::new(LruWidthIndependentAnnotationCache::new(2)));
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.annotation_cache = Box::new(SharedLruCache {
        entries: entries.clone(),
    });
    let first = input("段落一文本内容", 300.0);
    let second = input("段落二文本内容", 300.0);
    let third = input("段落三文本内容", 300.0);
    let first_key = to_width_independent_annotation_key(&first, HashMap::new());
    let second_key = to_width_independent_annotation_key(&second, HashMap::new());
    let third_key = to_width_independent_annotation_key(&third, HashMap::new());

    engine.layout(first);
    engine.layout(second);
    {
        let mut cache = entries.lock().unwrap();
        assert_eq!(2, cache.size());
        assert!(cache.get(&first_key).is_some());
        assert!(cache.get(&second_key).is_some());
    }

    engine.layout(third);
    let mut cache = entries.lock().unwrap();
    assert_eq!(2, cache.size());
    assert!(cache.get(&third_key).is_some());
    assert!(cache.get(&second_key).is_some());
    assert!(cache.get(&first_key).is_none());
}

#[test]
fn cached_and_uncached_layouts_match_at_narrow_normal_and_wide_widths() {
    let text =
        "提椠是一个面向中文正文的段落排版引擎，遵循中文排版需求规范，支持两端对齐与标点挤压。";
    let mut cached = ExplainableStubParagraphLayoutEngine::default();
    let mut uncached = ExplainableStubParagraphLayoutEngine::default();
    uncached.annotation_cache = Box::new(DisabledCache);

    for width in [80.0, 300.0, 650.0] {
        let expected = uncached.layout(input(text, width));
        let actual = cached.layout(input(text, width));
        assert_eq!(expected.lines.len(), actual.lines.len(), "width {width}");
        for (expected_line, actual_line) in expected.lines.iter().zip(&actual.lines) {
            assert_eq!(expected_line.range, actual_line.range, "width {width}");
            assert!(
                (expected_line.visual_width - actual_line.visual_width).abs() < 0.001,
                "width {width}"
            );
            assert!(
                (expected_line.adjusted_width - actual_line.adjusted_width).abs() < 0.001,
                "width {width}"
            );
        }
    }
}

fn body_input(text: &str, width: f32) -> LayoutInput {
    LayoutInput::builder(
        TiqianTextContent::new(Text::from(text)),
        LayoutConstraints::with_defaults(width),
    )
    .paragraph_style(
        ParagraphStyle::builder()
            .first_line_indent(Some(Ic { count: 2.0 }))
            .build(),
    )
    .build()
}

fn assert_cached_and_uncached_match(expected: &tiqian::core::layout_model::LayoutResult, actual: &tiqian::core::layout_model::LayoutResult, width: f32) {
    assert_eq!(expected.lines.len(), actual.lines.len(), "line count at width {width}");
    for (index, (expected_line, actual_line)) in expected.lines.iter().zip(&actual.lines).enumerate() {
        assert_eq!(expected_line.range, actual_line.range, "line {index} range at width {width}");
        assert!((expected_line.visual_width - actual_line.visual_width).abs() < 0.001, "line {index} visual width at width {width}");
        assert!((expected_line.adjusted_width - actual_line.adjusted_width).abs() < 0.001, "line {index} adjusted width at width {width}");
        assert!((expected_line.natural_width - actual_line.natural_width).abs() < 0.001, "line {index} natural width at width {width}");
        assert!((expected_line.indent - actual_line.indent).abs() < 0.001, "line {index} indent at width {width}");
        assert!((expected_line.hanging_punctuation_advance - actual_line.hanging_punctuation_advance).abs() < 0.001, "line {index} hanging punctuation at width {width}");
        assert_eq!(expected_line.end_reason, actual_line.end_reason, "line {index} end reason at width {width}");
    }
}

#[test]
fn cached_and_uncached_engines_produce_identical_layout_results_across_widths() {
    let fixtures = [
        "提椠是一个面向中文正文的段落排版引擎，遵循中文排版需求规范，支持两端对齐与标点挤压。",
        "在《中文排版需求》（CLREQ）中，要求正文「两端对齐」；当遇到『标点符号』与西文（如 OpenType / CSS Grid）混排时，应正确执行挤压与推入推出——即使在 120Hz 高频拖拽下也是如此！",
        "第一行缩进两个字身框。标点符号如……省略号、破折号——不应出现在行首，逗号、句号。也不得出现在行首。这就是避头尾（Kinsoku）规则的严格要求。",
    ];
    let mut cached = ExplainableStubParagraphLayoutEngine::default();
    let mut uncached = ExplainableStubParagraphLayoutEngine::default();
    uncached.annotation_cache = Box::new(DisabledCache);

    let mut width = 80.0;
    while width <= 650.0 {
        for fixture in fixtures {
            let expected = uncached.layout(body_input(fixture, width));
            let actual = cached.layout(body_input(fixture, width));
            assert_cached_and_uncached_match(&expected, &actual, width);
        }
        width += 7.3;
    }
}

#[test]
fn reflow_fuzzing_random_sequence_produces_exact_output() {
    let fixture = "提椠段落排版：严格遵循简体中文 CLREQ 规范。包含“双引号”、‘单引号’、以及（括号）与【括号】；汉字与 English words 混排时自动添加 0.25em 间距，最后一行保持左对齐。";
    let mut cached = ExplainableStubParagraphLayoutEngine::default();
    let mut uncached = ExplainableStubParagraphLayoutEngine::default();
    uncached.annotation_cache = Box::new(DisabledCache);

    for width in [
        320.0, 150.0, 480.5, 95.2, 210.0, 600.0, 120.3, 450.0, 180.7, 300.0,
        75.0, 520.0, 133.3, 266.6, 399.9, 110.0, 470.0, 195.0, 345.0, 580.0,
    ] {
        let expected = uncached.layout(body_input(fixture, width));
        let actual = cached.layout(body_input(fixture, width));
        assert_cached_and_uncached_match(&expected, &actual, width);
    }
}
