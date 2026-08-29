use tiqian::core::Geometry::LayoutConstraints;
use tiqian::core::Text::Text;
use tiqian::core::TextModel::{LayoutInput, LineLengthGrid, ParagraphStyle, TiqianTextContent};
use tiqian::core::Units::Ic;
use tiqian::layout::LineBreaker::{GreedyLineBreaker, LookaheadLineBreaker};
use tiqian::layout::ParagraphLayoutEngine::{
    ExplainableStubParagraphLayoutEngine, ParagraphLayoutEngine,
};
use tiqian::linebreak::Hyphenation::NoHyphenator;

fn layout_with_greedy(text: &str, max_width: f32) -> tiqian::core::LayoutModel::LayoutResult {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.line_breaker = Box::new(GreedyLineBreaker::default());
    engine.hyphenator = &NoHyphenator;
    engine.layout(input(text, max_width))
}

fn layout_with_lookahead(text: &str, max_width: f32) -> tiqian::core::LayoutModel::LayoutResult {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.line_breaker = Box::new(LookaheadLineBreaker::default());
    engine.hyphenator = &NoHyphenator;
    engine.layout(input(text, max_width))
}

fn input(text: &str, max_width: f32) -> LayoutInput {
    LayoutInput::builder(
        TiqianTextContent::new(Text::from(text)),
        LayoutConstraints::with_defaults(max_width),
    )
    .paragraph_style(
        ParagraphStyle::builder()
            .first_line_indent(Some(Ic::ZERO))
            .line_length_grid(LineLengthGrid::with_enabled(false))
            .build(),
    )
    .build()
}

#[test]
fn zero_width_space_is_unshaped_soft_break_for_both_breakers() {
    for (name, result) in [
        ("greedy", layout_with_greedy("foo\u{200b}bar", 48.0)),
        ("lookahead", layout_with_lookahead("foo\u{200b}bar", 48.0)),
    ] {
        let control = result
            .clusters
            .iter()
            .find(|cluster| cluster.text == "\u{200b}")
            .unwrap();
        assert_eq!("", control.display_text, "{name}");
        assert_eq!(0.0, control.advance, "{name}");
        assert!(
            result
                .glyph_runs
                .iter()
                .flat_map(|run| &run.glyphs)
                .all(|glyph| glyph.cluster_range != control.range),
            "{name}"
        );
        assert_eq!(0, result.lines[0].range.start(), "{name}");
        assert_eq!(4, result.lines[0].range.end(), "{name}");
        assert_eq!(4, result.lines[1].range.start(), "{name}");
        assert_eq!(7, result.lines[1].range.end(), "{name}");
        assert_eq!(
            "ZeroWidthSpaceSoftBreakNoShape",
            result
                .debug
                .shaping_decisions
                .iter()
                .find(|decision| decision.range == control.range)
                .unwrap()
                .reason,
            "{name}",
        );
        assert_eq!(
            control.range, result.debug.zero_width_break_decisions[0].range,
            "{name}"
        );
    }
}

#[test]
fn leading_zero_width_space_does_not_create_empty_auto_wrapped_line() {
    for (name, result) in [
        ("greedy", layout_with_greedy("\u{200b}中", 8.0)),
        ("lookahead", layout_with_lookahead("\u{200b}中", 8.0)),
    ] {
        assert_eq!(1, result.lines.len(), "{name}");
        assert_eq!(0, result.lines[0].range.start(), "{name}");
        assert_eq!(2, result.lines[0].range.end(), "{name}");
    }
}
