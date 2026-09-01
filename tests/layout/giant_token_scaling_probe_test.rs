use std::time::Instant;

use tiqian::core::geometry::LayoutConstraints;
use tiqian::core::text::Text;
use tiqian::core::text_model::{LayoutInput, TiqianTextContent};
use tiqian::layout::line_breaker::{GreedyLineBreaker, LookaheadLineBreaker};
use tiqian::layout::paragraph_layout_engine::{
    ExplainableStubParagraphLayoutEngine, ParagraphLayoutEngine,
};
use tiqian::layout::width_independent_annotation_cache::LruWidthIndependentAnnotationCache;

fn giant_token_input(length: usize) -> LayoutInput {
    let unit = r"\rlap{\color{#BB9}{\rule{4px}{320px}}}{";
    let token = unit.repeat(length.div_ceil(unit.len()));
    LayoutInput::builder(
        TiqianTextContent::new(Text::from(&token[..length])),
        LayoutConstraints::with_defaults(1248.0),
    )
    .build()
}

fn engine(use_lookahead: bool) -> ExplainableStubParagraphLayoutEngine {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.line_breaker = if use_lookahead {
        Box::new(LookaheadLineBreaker::default())
    } else {
        Box::new(GreedyLineBreaker::default())
    };
    engine.annotation_cache = Box::new(LruWidthIndependentAnnotationCache::new(8));
    engine
}

#[test]
fn giant_token_layout_stays_far_below_the_quadratic_ceiling() {
    let input = giant_token_input(80_000);
    let mut engine = engine(false);
    engine.layout(input.clone());
    let warm = (0..3)
        .map(|_| {
            let started = Instant::now();
            engine.layout(input.clone());
            started.elapsed()
        })
        .min()
        .unwrap();
    println!("giant-token 80k warm layout: {:.3} ms", warm.as_secs_f64() * 1_000.0);
    assert!(
        warm.as_millis() < 1_500,
        "80k single-token layout took {:.3} ms; the quadratic planning regression is back",
        warm.as_secs_f64() * 1_000.0,
    );
}

#[test]
fn measure_giant_token_scaling_matrix() {
    if std::env::var("TIQIAN_RUN_EXPERIMENTS").as_deref() != Ok("1") {
        println!("GiantTokenScalingProbe: set TIQIAN_RUN_EXPERIMENTS=1 to run the matrix.");
        return;
    }
    for (name, use_lookahead) in [("lookahead", true), ("greedy", false)] {
        for length in [5_000, 10_000, 20_000, 40_000, 80_000] {
            let input = giant_token_input(length);
            let mut engine = engine(use_lookahead);
            for _ in 0..if length <= 10_000 { 2 } else { 1 } {
                engine.layout(input.clone());
            }
            let warm = (0..3)
                .map(|_| {
                    let started = Instant::now();
                    engine.layout(input.clone());
                    started.elapsed()
                })
                .min()
                .unwrap();
            engine.annotation_cache.clear();
            let started = Instant::now();
            engine.layout(input);
            let cold = started.elapsed();
            println!(
                "giant-token[{name}] length={length} warm={:.3} ms cold={:.3} ms",
                warm.as_secs_f64() * 1_000.0,
                cold.as_secs_f64() * 1_000.0,
            );
        }
    }
}
