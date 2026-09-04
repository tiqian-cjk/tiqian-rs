use tiqian::core::geometry::{text_range, LayoutConstraints, TextRange};
use tiqian::core::text::Text;
use tiqian::core::text_model::{LayoutInput, ParagraphStyle, TiqianTextContent};
use tiqian::core::units::Ic;
use tiqian::layout::paragraph_layout_engine::{
    ExplainableStubParagraphLayoutEngine, ParagraphLayoutEngine,
};

fn layout(text: &str, suppressed: Vec<TextRange>) -> tiqian::core::layout_model::LayoutResult {
    ExplainableStubParagraphLayoutEngine::default().layout(
        LayoutInput::builder(
            TiqianTextContent::builder(Text::from(text))
                .auto_space_suppressed_ranges(suppressed)
                .build(),
            LayoutConstraints::with_defaults(640.0),
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
fn verbatim_internal_boundaries_are_suppressed_while_outer_edges_keep_autospace() {
    let text = "跑print你好print跑";
    let control = layout(text, Vec::new());
    assert_eq!(
        4,
        control
            .debug
            .auto_space_decisions
            .iter()
            .filter(|decision| decision.reason == "TextAutoSpaceInsert:east-asian-spacing-W-N")
            .count()
    );

    let result = layout(text, vec![text_range(1, 13)]);
    assert_eq!(
        2,
        result
            .debug
            .auto_space_decisions
            .iter()
            .filter(|decision| decision.reason == "TextAutoSpaceInsert:east-asian-spacing-W-N")
            .count()
    );
    assert_eq!(
        2,
        result
            .debug
            .auto_space_decisions
            .iter()
            .filter(|decision| decision.reason
                == "VerbatimRangeAutoSpace:east-asian-spacing-W-N-suppressed")
            .count()
    );
}

#[test]
fn authored_space_inside_verbatim_range_is_not_normalized() {
    let text = "跑a 你b跑";
    let control = layout(text, Vec::new());
    assert_eq!(
        1,
        control
            .debug
            .auto_space_decisions
            .iter()
            .filter(
                |decision| decision.reason == "TextAutoSpaceReplace:east-asian-spacing-W-space-N"
            )
            .count()
    );

    let result = layout(text, vec![text_range(1, 5)]);
    assert_eq!(
        0,
        result
            .debug
            .auto_space_decisions
            .iter()
            .filter(
                |decision| decision.reason == "TextAutoSpaceReplace:east-asian-spacing-W-space-N"
            )
            .count()
    );
}
