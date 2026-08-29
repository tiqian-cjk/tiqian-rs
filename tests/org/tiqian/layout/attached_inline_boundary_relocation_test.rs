use tiqian::core::geometry::{LayoutConstraints, TextRange};
use tiqian::core::text::Text;
use tiqian::core::text_model::{
    InlineAttachment, LayoutInput, ParagraphStyle, TextSpan, TextStyle, TiqianTextContent,
};
use tiqian::core::units::Ic;
use tiqian::layout::line_breaker::{GreedyLineBreaker, LookaheadLineBreaker};
use tiqian::layout::paragraph_dp_line_breaker::ParagraphDpLineBreaker;
use tiqian::layout::paragraph_layout_engine::{
    ExplainableStubParagraphLayoutEngine, ParagraphLayoutEngine,
};
use tiqian::layout::unicode_punctuation_boundary_resolver::resolve_attached_inline_virtual_boundaries;

fn attached_span(range: TextRange) -> TextSpan {
    TextSpan {
        range,
        style: TextStyle::builder()
            .inline_attachment(InlineAttachment::Previous)
            .build(),
    }
}

fn layout_reference(text: &str) -> tiqian::core::layout_model::LayoutResult {
    let byte_start = text.find("[1]").unwrap();
    let start = text[..byte_start].encode_utf16().count() as i32;
    ExplainableStubParagraphLayoutEngine::default().layout(
        LayoutInput::builder(
            TiqianTextContent::builder(Text::from(text))
                .spans(vec![attached_span(TextRange::new(start, start + 3))])
                .build(),
            LayoutConstraints::with_defaults(320.0),
        )
        .paragraph_style(
            ParagraphStyle::builder()
                .first_line_indent(Some(Ic::ZERO))
                .build(),
        )
        .build(),
    )
}

#[test]
fn attached_run_exposes_only_its_prose_neighbors() {
    let result = resolve_attached_inline_virtual_boundaries(&[
        InlineAttachment::None,
        InlineAttachment::None,
        InlineAttachment::Previous,
        InlineAttachment::Previous,
        InlineAttachment::Previous,
        InlineAttachment::None,
    ]);

    assert_eq!(1, result.len());
    assert_eq!(1, result[0].previous_cluster_index);
    assert_eq!((2, 4), result[0].attached_cluster_range);
    assert_eq!(Some(5), result[0].next_cluster_index);
}

#[test]
fn virtual_punctuation_boundary_compresses_only_when_followed_by_punctuation() {
    let compressed = layout_reference("正文：“内容。”[1]，后文");
    let decision = compressed
        .debug
        .spacing_decisions
        .iter()
        .find(|decision| {
            decision
                .reason
                .as_str()
                .starts_with("AttachedInlineVirtualPunctuationBoundary")
        })
        .unwrap();
    assert_eq!(
        "AttachedInlineVirtualPunctuationBoundary:adjacent-punctuation",
        decision.reason
    );
    assert!(decision.natural_inner_glue > 0.0);
    assert_eq!(0.0, decision.adjusted_inner_glue);

    let natural = layout_reference("正文：“内容。”[1]后文");
    let decision = natural
        .debug
        .spacing_decisions
        .iter()
        .find(|decision| {
            decision
                .reason
                .as_str()
                .starts_with("AttachedInlineVirtualPunctuationBoundary")
        })
        .unwrap();
    assert_eq!(
        "AttachedInlineVirtualPunctuationBoundary:natural",
        decision.reason
    );
    assert_eq!(decision.natural_inner_glue, decision.adjusted_inner_glue);
}

#[test]
fn attached_reference_never_starts_wrapped_line_for_supported_breakers() {
    let text = "甲乙1丙";
    let range = TextRange::new(2, 3);
    for (name, breaker) in [
        (
            "greedy",
            Box::new(GreedyLineBreaker::default())
                as Box<dyn tiqian::layout::line_breaker::LineBreaker>,
        ),
        (
            "lookahead",
            Box::new(LookaheadLineBreaker::default())
                as Box<dyn tiqian::layout::line_breaker::LineBreaker>,
        ),
        (
            "paragraph-dp",
            Box::new(ParagraphDpLineBreaker::default())
                as Box<dyn tiqian::layout::line_breaker::LineBreaker>,
        ),
    ] {
        let mut engine = ExplainableStubParagraphLayoutEngine::default();
        engine.line_breaker = breaker;
        let result = engine.layout(
            LayoutInput::builder(
                TiqianTextContent::builder(Text::from(text))
                    .spans(vec![attached_span(range)])
                    .build(),
                LayoutConstraints::with_defaults(32.0),
            )
            .paragraph_style(
                ParagraphStyle::builder()
                    .first_line_indent(Some(Ic::ZERO))
                    .build(),
            )
            .build(),
        );
        assert!(result.lines.len() > 1, "{name}");
        assert!(
            result
                .lines
                .iter()
                .all(|line| line.range.start() != range.start()),
            "{name}: {:?}",
            result.lines
        );
        assert!(
            result
                .lines
                .iter()
                .any(|line| line.range.start() < range.start() && line.range.end() >= range.end()),
            "{name}: {:?}",
            result.lines
        );
    }
}
