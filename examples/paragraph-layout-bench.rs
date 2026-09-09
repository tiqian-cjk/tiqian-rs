use std::hint::black_box;
use std::time::{Duration, Instant};

use tiqian::core::geometry::LayoutConstraints;
use tiqian::core::layout_model::LayoutResult;
use tiqian::core::text_model::{LayoutInput, LineLengthGrid};
use tiqian::layout::paragraph_layout_engine::{
    ExplainableStubParagraphLayoutEngine, ParagraphLayoutEngine,
};

#[allow(dead_code)]
#[path = "paragraph_demo/font_backend.rs"]
mod font_backend;
#[allow(dead_code)]
#[path = "paragraph_demo/sample.rs"]
mod sample;

struct Options {
    replay: bool,
    iterations: usize,
    warmup: usize,
    widths: Vec<f32>,
    scale: f32,
}

impl Options {
    fn parse() -> Result<Option<Self>, String> {
        let mut options = Self {
            replay: false,
            iterations: 200,
            warmup: 20,
            widths: vec![672.0, 360.0, 960.0],
            scale: 1.0,
        };
        let mut args = std::env::args().skip(1);
        while let Some(flag) = args.next() {
            if flag == "--replay" {
                options.replay = true;
                continue;
            }
            if flag == "--help" || flag == "-h" {
                println!("paragraph-layout-bench [--replay] [--iterations 200] [--warmup 20] [--widths 672,360,960] [--scale 1]");
                println!("One iteration visits every width in order. Widths are physical pixels.");
                return Ok(None);
            }
            if !matches!(flag.as_str(), "--iterations" | "--warmup" | "--widths" | "--scale") {
                return Err(format!("unknown option: {flag}"));
            }
            let value = args.next().ok_or_else(|| format!("missing value for {flag}"))?;
            match flag.as_str() {
                "--iterations" => options.iterations = value.parse().map_err(|_| "invalid iterations")?,
                "--warmup" => options.warmup = value.parse().map_err(|_| "invalid warmup")?,
                "--widths" => {
                    options.widths = value.split(',').map(|width| {
                        width.parse::<f32>().map_err(|_| format!("invalid width: {width}"))
                    }).collect::<Result<_, _>>()?;
                }
                "--scale" => options.scale = value.parse().map_err(|_| "invalid scale")?,
                _ => unreachable!(),
            }
        }
        if options.iterations == 0 {
            return Err("iterations must be positive".to_owned());
        }
        if !options.scale.is_finite() || options.scale <= 0.0
            || options.widths.iter().any(|width| !width.is_finite() || *width <= 0.0)
        {
            return Err("widths and scale must be finite and positive".to_owned());
        }
        Ok(Some(options))
    }
}

#[derive(Default)]
struct PageMeasurement {
    replay: Duration,
    drop_replay: Duration,
    layout: Duration,
    drop_results: Duration,
    total: Duration,
    calls: usize,
    lines: usize,
    clusters: usize,
    glyphs: usize,
}

fn timed_layout(
    engine: &mut ExplainableStubParagraphLayoutEngine,
    input: LayoutInput,
    measurement: &mut PageMeasurement,
) -> LayoutResult {
    let start = Instant::now();
    let result = engine.layout(black_box(input));
    measurement.layout += start.elapsed();
    measurement.calls += 1;
    black_box(result)
}

// 保留整页输出，使每次布局的计时不包含先前结果的析构。
fn measure_page(
    engine: &mut ExplainableStubParagraphLayoutEngine,
    width: f32,
    scale: f32,
    replay: bool,
) -> PageMeasurement {
    let start = Instant::now();
    let document = sample::build_document_demo(width, scale);
    let mut results = Vec::with_capacity(document.blocks.len() * 3);
    let mut measurement = PageMeasurement::default();
    let mut measurement_only = std::collections::HashSet::new();
    for block in document.blocks {
        match block {
            sample::DemoDocumentDemoBlock::Paragraph(document) => {
                results.push(timed_layout(engine, document.input, &mut measurement));
            }
            sample::DemoDocumentDemoBlock::NarrowParagraph { mut document, max_width } => {
                document.input.constraints = LayoutConstraints::with_defaults(max_width);
                results.push(timed_layout(engine, document.input, &mut measurement));
            }
            sample::DemoDocumentDemoBlock::ListItem { mut marker, mut body } => {
                let font_size = body.input.text_style.font_size;
                let mut marker_input = marker.input.clone();
                marker_input.paragraph_style.line_length_grid = LineLengthGrid::with_enabled(false);
                marker_input.constraints = LayoutConstraints::with_defaults(100_000.0);
                let marker_measurement = timed_layout(engine, marker_input, &mut measurement);
                let gutter = (marker_measurement.size.width / font_size).ceil().max(1.0) * font_size;
                measurement_only.insert(results.len());
                results.push(marker_measurement);
                marker.input.constraints = LayoutConstraints::with_defaults(gutter);
                body.input.constraints = LayoutConstraints::with_defaults((width - gutter).max(1.0));
                results.push(timed_layout(engine, marker.input, &mut measurement));
                results.push(timed_layout(engine, body.input, &mut measurement));
            }
            sample::DemoDocumentDemoBlock::Section { .. } => {}
        }
    }
    for result in black_box(&results) {
        measurement.lines += result.lines.len();
        measurement.clusters += result.clusters.len();
        measurement.glyphs += result.glyph_runs.iter().map(|run| run.glyphs.len()).sum::<usize>();
    }
    if replay {
        let replay_start = Instant::now();
        let indices: Vec<_> = results.iter().enumerate()
            .filter(|(index, _)| !measurement_only.contains(index))
            .map(|(_, result)| tiqian::core::layout_result_replay_index::to_replay_index(black_box(result)))
            .collect();
        measurement.replay = replay_start.elapsed();
        let drop_start = Instant::now();
        drop(black_box(indices));
        measurement.drop_replay = drop_start.elapsed();
    }
    let drop_start = Instant::now();
    drop(black_box(results));
    measurement.drop_results = drop_start.elapsed();
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
    println!("{label:<16} n={n:<5} min={:.3} median={median:.3} p95={p95:.3} mean={:.3} max={:.3} ms",
        values[0], values.iter().sum::<f64>() / n as f64, values[n - 1]);
}

fn main() -> Result<(), String> {
    let Some(options) = Options::parse()? else { return Ok(()); };
    if cfg!(debug_assertions) {
        eprintln!("Warning: debug build; use --release for performance comparisons.");
    }
    let start = Instant::now();
    let catalog = font_backend::DemoFontCatalog::load()?;
    catalog.validate_demo_faces()?;
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.fallback_resolver = Box::new(catalog.clone());
    engine.font_metrics_resolver = Box::new(catalog.clone());
    engine.text_shaper = Box::new(catalog);
    println!("font/engine setup: {:.3} ms", start.elapsed().as_secs_f64() * 1000.0);
    println!("widths={:?} scale={} warmup={} iterations={} profile={} arch={}",
        options.widths, options.scale, options.warmup, options.iterations,
        if cfg!(debug_assertions) { "debug" } else { "release" }, std::env::consts::ARCH);
    println!("Times are per sample page, not per paragraph. Layout includes full debug output and font backend calls.");
    println!("Total includes input preparation, layout, output counting and result drop; no GUI or rendering.");
    if options.replay {
        println!("Replay enabled: total also includes replay construction and drop, excluding temporary marker measurements.");
    }
    println!("First sequence (shared engine; later widths may reuse caches):");
    let mut expected = Vec::new();
    for &width in &options.widths {
        let page = measure_page(&mut engine, width, options.scale, options.replay);
        println!("width={width} layout={:.3} drop={:.3} total={:.3} ms calls={} lines={} clusters={} body_glyphs={}",
            page.layout.as_secs_f64() * 1000.0, page.drop_results.as_secs_f64() * 1000.0,
            page.total.as_secs_f64() * 1000.0, page.calls, page.lines, page.clusters, page.glyphs);
        expected.push((page.calls, page.lines, page.clusters, page.glyphs));
    }
    for _ in 0..options.warmup {
        for &width in &options.widths {
            black_box(measure_page(&mut engine, width, options.scale, options.replay));
        }
    }
    let mut samples: Vec<Vec<PageMeasurement>> = options.widths.iter()
        .map(|_| Vec::with_capacity(options.iterations)).collect();
    for _ in 0..options.iterations {
        for (index, &width) in options.widths.iter().enumerate() {
            let page = measure_page(&mut engine, width, options.scale, options.replay);
            if (page.calls, page.lines, page.clusters, page.glyphs) != expected[index] {
                return Err(format!("layout workload changed after warmup at width {width}"));
            }
            samples[index].push(page);
        }
    }
    for (index, &width) in options.widths.iter().enumerate() {
        println!("Measured width={width}:");
        print_stats("layout", samples[index].iter().map(|page| page.layout));
        if options.replay {
            print_stats("replay", samples[index].iter().map(|page| page.replay));
            print_stats("replay_drop", samples[index].iter().map(|page| page.drop_replay));
        }
        print_stats("result_drop", samples[index].iter().map(|page| page.drop_results));
        print_stats("total", samples[index].iter().map(|page| page.total));
    }
    println!("All measured pages:");
    print_stats("layout", samples.iter().flatten().map(|page| page.layout));
    print_stats("result_drop", samples.iter().flatten().map(|page| page.drop_results));
    print_stats("total", samples.iter().flatten().map(|page| page.total));
    println!("Measured layout calls: {}", samples.iter().flatten().map(|page| page.calls).sum::<usize>());
    Ok(())
}