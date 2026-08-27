use tiqian::org::tiqian::core::Geometry::LayoutConstraints;
use tiqian::org::tiqian::core::TextModel::{LayoutInput, ParagraphStyle, TiqianTextContent};
use tiqian::org::tiqian::core::Units::Ic;
use tiqian::org::tiqian::layout::LineBreaker::{GreedyLineBreaker, LookaheadLineBreaker};
use tiqian::org::tiqian::layout::ParagraphLayoutEngine::{
    ExplainableStubParagraphLayoutEngine, ParagraphLayoutEngine,
};
use tiqian::org::tiqian::linebreak::Hyphenation::NoHyphenator;

fn layout_with_greedy(text: &str) -> tiqian::org::tiqian::core::LayoutModel::LayoutResult {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.line_breaker = Box::new(GreedyLineBreaker::default());
    engine.hyphenator = &NoHyphenator;
    engine.layout(input(text))
}

fn layout_with_lookahead(text: &str) -> tiqian::org::tiqian::core::LayoutModel::LayoutResult {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.line_breaker = Box::new(LookaheadLineBreaker::default());
    engine.hyphenator = &NoHyphenator;
    engine.layout(input(text))
}

fn input(text: &str) -> LayoutInput {
    LayoutInput::builder(
        TiqianTextContent::new(text.to_owned()),
        LayoutConstraints::with_defaults(64.0),
    )
    .paragraph_style(ParagraphStyle::builder().first_line_indent(Some(Ic::ZERO)).build())
    .build()
}

#[test]
fn cjk_attached_ascii_point_mark_is_separate_from_following_latin_run() {
    for (name, result) in [
        ("greedy", layout_with_greedy("中文,anyway继续")),
        ("lookahead", layout_with_lookahead("中文,anyway继续")),
    ] {
        assert!(result.clusters.iter().any(|cluster| cluster.text == ","), "{name}: {:?}", result.clusters);
        assert!(result.debug.font_decisions.iter().any(|decision| decision.source_text == "anyway"), "{name}: {:?}", result.debug.font_decisions);
        assert!(result.clusters.iter().all(|cluster| cluster.text != ",anyway"), "{name}: {:?}", result.clusters);
    }
}

#[test]
fn latin_tokens_keep_existing_internal_ascii_punctuation_segmentation() {
    let result = ExplainableStubParagraphLayoutEngine::default().layout(
        LayoutInput::builder(
            TiqianTextContent::new("foo,bar 1,234 50% \"quoted\"".to_owned()),
            LayoutConstraints::with_defaults(1000.0),
        )
        .paragraph_style(ParagraphStyle::builder().first_line_indent(Some(Ic::ZERO)).build())
        .build(),
    );
    let texts = result.clusters.iter().map(|cluster| cluster.text.as_str()).collect::<Vec<_>>();

    for token in ["foo,bar", "1,234", "50%", "\"quoted\""] {
        assert!(texts.contains(&token), "missing {token}: {texts:?}");
    }
}
