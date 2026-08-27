use std::collections::HashSet;

use tiqian::org::tiqian::core::Geometry::{LayoutConstraints, Rect, TextRange};
use tiqian::org::tiqian::core::LayoutModel::{Cluster, Glyph, GlyphRun, LineEndReason};
use tiqian::org::tiqian::core::TextModel::{
    LayoutInput, ParagraphStyle, TextSpan, TextStyle, TiqianTextContent,
};
use tiqian::org::tiqian::core::Units::Ic;
use tiqian::org::tiqian::layout::LineBreaker::LookaheadLineBreaker;
use tiqian::org::tiqian::layout::ParagraphLayoutEngine::{
    ExplainableStubParagraphLayoutEngine, ParagraphLayoutEngine,
};
use tiqian::org::tiqian::shaping::TextShaper::{ShapingInput, ShapingResult, TextShaper};

fn input(text: &str) -> LayoutInput {
    LayoutInput::builder(
        TiqianTextContent::new(text.to_owned()),
        LayoutConstraints::with_defaults(240.0),
    )
    .paragraph_style(ParagraphStyle::builder().first_line_indent(Some(Ic::ZERO)).build())
    .build()
}

#[test]
fn paragraph_entry_returns_single_line_and_records_selected_breaker() {
    let mut greedy = ExplainableStubParagraphLayoutEngine::default();
    let result = greedy.layout(input("提椠"));
    assert_eq!(2, result.clusters.len());
    assert_eq!(1, result.lines.len());
    assert_eq!("greedy", result.debug.line_decisions[0].kind);

    let mut lookahead = ExplainableStubParagraphLayoutEngine::default();
    lookahead.line_breaker = Box::new(LookaheadLineBreaker::default());
    let result = lookahead.layout(input("提椠"));
    assert_eq!("lookahead", result.debug.line_decisions[0].kind);
}

#[test]
fn mandatory_break_controls_are_unshaped_and_preserve_crlf_and_trailing_blank_line() {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    let result = engine.layout(input("甲\r\n乙\n"));

    assert_eq!(3, result.lines.len());
    assert_eq!(LineEndReason::MandatoryBreak, result.lines[0].end_reason);
    assert_eq!(LineEndReason::MandatoryBreak, result.lines[1].end_reason);
    assert_eq!(LineEndReason::ParagraphEnd, result.lines[2].end_reason);
    let crlf = result.clusters.iter().find(|cluster| cluster.text == "\r\n").unwrap();
    assert_eq!(TextRange::new(1, 3), crlf.range);
    assert_eq!("", crlf.display_text);
    assert_eq!(0.0, crlf.advance);
    assert!(result.glyph_runs.iter().flat_map(|run| &run.glyphs).all(|glyph| glyph.cluster_range != crlf.range));
    assert_eq!(TextRange::new(5, 5), result.lines[2].range);
}

struct BoundsTextShaper;

impl TextShaper for BoundsTextShaper {
    fn shape(&self, input: &ShapingInput) -> ShapingResult {
        let bounds = Rect { left: 1.0, top: -10.0, right: 12.0, bottom: 2.0 };
        ShapingResult::new(
            vec![Cluster::with_display_text(
                input.range,
                input.display_text.clone(),
                input.display_text.clone(),
                input.font_decision.candidate.key.clone(),
                20.0,
            )],
            vec![GlyphRun::new(
                input.range,
                input.font_decision.candidate.key.clone(),
                vec![Glyph::builder(42, input.range, 20.0).bounds(Some(bounds)).build()],
                20.0,
            )],
        )
    }
}

#[test]
fn paragraph_entry_preserves_shaper_glyph_bounds() {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.text_shaper = Box::new(BoundsTextShaper);
    let result = engine.layout(input("A"));

    let glyph = &result.glyph_runs[0].glyphs[0];
    assert_eq!(42, glyph.id);
    assert_eq!(20.0, glyph.advance);
    assert_eq!(Some(Rect { left: 1.0, top: -10.0, right: 12.0, bottom: 2.0 }), glyph.bounds);
}

#[test]
fn paragraph_entry_records_fallback_and_substituted_shaping_ranges() {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    let result = engine.layout(input("提椠……English——世界。"));

    assert!(result.debug.font_decisions.iter().any(|decision| {
        decision.source_text == "……"
            && decision.display_text == "⋯⋯"
            && decision.role == "CjkPunctuation"
            && decision.font_key == "cjk-primary"
    }));
    assert!(result.debug.font_decisions.iter().any(|decision| {
        decision.source_text == "English"
            && decision.role == "LatinText"
            && decision.font_key == "latin-primary"
    }));
    assert!(result.debug.shaping_decisions.iter().any(|decision| {
        decision.source_text == "——"
            && decision.display_text == "⸺"
            && decision.advance == 32.0
            && decision.source == "Stub"
    }));
}

#[test]
fn combining_marks_remain_in_their_base_shaping_runs() {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    let result = engine.layout(input("༎ຶ Ỏ̷"));

    assert!(result.debug.shaping_decisions.iter().any(|decision| decision.source_text == "༎ຶ"));
    assert!(result.debug.shaping_decisions.iter().any(|decision| decision.source_text == "Ỏ̷"));
    assert!(result.debug.shaping_decisions.iter().all(|decision| decision.source_text != "ຶ" && decision.source_text != "̷"));
}

#[test]
fn complex_emoji_remains_atomic_until_style_boundary_requires_a_split() {
    let text = "👩🏽‍💻";
    let length = text.encode_utf16().count() as i32;
    let atomic_content = TiqianTextContent::builder(text.to_owned())
        .source_boundaries(HashSet::from([2]))
        .build();
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    let atomic = engine.layout(
        LayoutInput::builder(atomic_content, LayoutConstraints::with_defaults(320.0))
            .paragraph_style(ParagraphStyle::builder().first_line_indent(Some(Ic::ZERO)).build())
            .build(),
    );
    assert_eq!(vec![TextRange::new(0, length)], atomic.debug.shaping_decisions.iter().map(|decision| decision.range).collect::<Vec<_>>());

    let styled_content = TiqianTextContent::builder(text.to_owned())
        .spans(vec![TextSpan {
            range: TextRange::new(2, length),
            style: TextStyle::builder().font_weight(700).build(),
        }])
        .source_boundaries(HashSet::from([2]))
        .build();
    let styled = engine.layout(
        LayoutInput::builder(styled_content, LayoutConstraints::with_defaults(320.0))
            .paragraph_style(ParagraphStyle::builder().first_line_indent(Some(Ic::ZERO)).build())
            .build(),
    );
    assert_eq!(vec![TextRange::new(0, 2), TextRange::new(2, length)], styled.debug.shaping_decisions.iter().map(|decision| decision.range).collect::<Vec<_>>());
}
