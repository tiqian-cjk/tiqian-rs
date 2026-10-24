use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;
use tiqian::api::ParagraphLayoutEngineBuilder;
use tiqian::core::geometry::LayoutConstraints;
use tiqian::core::text::Text;
use tiqian::core::text_model::{
    LayoutInput, LineLengthGrid, ParagraphStyle, TiqianTextContent, WhiteSpaceMode,
};
use tiqian::core::units::Ic;

#[allow(dead_code)]
#[path = "paragraph_demo/font_backend.rs"]
mod font_backend;

struct CountingAllocator;
static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
static BYTES: AtomicUsize = AtomicUsize::new(0);
// 只统计布局调用期间的申请量；两份构建使用相同的系统分配器和计数逻辑。
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        BYTES.fetch_add(layout.size(), Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        BYTES.fetch_add(size, Ordering::Relaxed);
        unsafe { System.realloc(pointer, layout, size) }
    }
}
#[global_allocator]
static GLOBAL: CountingAllocator = CountingAllocator;

fn main() -> Result<(), String> {
    let workload = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "plain".to_owned());
    let repeats: usize = std::env::args()
        .nth(2)
        .unwrap_or_else(|| "64".to_owned())
        .parse()
        .map_err(|_| "invalid repeats")?;
    let text = match workload.as_str() {
        "plain" => "中文排版需要清晰稳定的几何与断行结果".repeat(repeats),
        "tabs" => "中文\t中文 \t中文\t".repeat(repeats),
        "spaces" => format!("中{}文", " ".repeat(repeats * 32)),
        _ => return Err("unknown workload".to_owned()),
    };
    let catalog = font_backend::DemoFontCatalog::load()?;
    let mut paragraph = ParagraphStyle::builder()
        .first_line_indent(Some(Ic::ZERO))
        .line_length_grid(LineLengthGrid::with_enabled(false))
        .build();
    if workload != "plain" {
        paragraph.white_space = WhiteSpaceMode::BreakSpaces;
    }
    let input = LayoutInput::builder(
        TiqianTextContent::new(Text::from(text.as_str())),
        LayoutConstraints::with_defaults(672.0),
    )
    .paragraph_style(paragraph)
    .build();
    for cold in [false, true] {
        let mut engine = ParagraphLayoutEngineBuilder::new(Box::new(catalog.clone())).build();
        let mut samples = Vec::new();
        let mut counts = (0, 0, 0, 0);
        for iteration in 0..120 {
            if cold {
                engine = ParagraphLayoutEngineBuilder::new(Box::new(catalog.clone())).build();
            }
            let input = input.clone();
            ALLOCATIONS.store(0, Ordering::Relaxed);
            BYTES.store(0, Ordering::Relaxed);
            let start = Instant::now();
            let result = std::hint::black_box(engine.layout(std::hint::black_box(input)));
            let elapsed = start.elapsed().as_secs_f64() * 1000.0;
            counts = (
                ALLOCATIONS.load(Ordering::Relaxed),
                BYTES.load(Ordering::Relaxed),
                result.lines.len(),
                result.clusters.len(),
            );
            if iteration >= 20 {
                samples.push(elapsed);
            }
        }
        samples.sort_by(f64::total_cmp);
        println!(
            "workload={workload} repeats={repeats} scalars={} cold={cold} min={:.6} median={:.6} ms allocations={} bytes={} lines={} clusters={}",
            text.chars().count(),
            samples[0],
            samples[50],
            counts.0,
            counts.1,
            counts.2,
            counts.3
        );
    }
    Ok(())
}
