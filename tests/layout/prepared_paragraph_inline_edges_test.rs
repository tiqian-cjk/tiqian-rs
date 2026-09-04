use tiqian::core::geometry::{text_range, LayoutConstraints};
use tiqian::core::text::Text;
use tiqian::core::text_model::{InlineBoxSpan, LayoutInput, ParagraphStyle, TiqianTextContent};
use tiqian::core::units::Ic;
use tiqian::layout::paragraph_layout_engine::{
    ExplainableStubParagraphLayoutEngine, ParagraphLayoutEngine,
};
use tiqian::layout::prepared_paragraph::to_prepared_paragraph_json;

#[test]
fn end_only_inline_box_emits_edge_without_inline_start_field() {
    let result = ExplainableStubParagraphLayoutEngine::default().layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from("中文正文")),
            LayoutConstraints::with_defaults(320.0),
        )
        .paragraph_style(ParagraphStyle::builder().first_line_indent(Some(Ic::ZERO)).build())
        .inline_boxes(vec![InlineBoxSpan::with_edges(text_range(0, 2), 0.0, 4.0)])
        .build(),
    );

    let json = to_prepared_paragraph_json(&result, true);
    let edges_at = json.find("\"inlineEdges\":[").expect("inlineEdges array missing");
    let entry = &json[edges_at..];
    assert!(entry.contains("\"offset\":2"), "{entry}");
    assert!(entry.contains("\"inlineEnd\":4"), "{entry}");
    assert!(!entry.contains("\"inlineStart\":"), "{entry}");
}

#[test]
fn content_without_inline_boxes_omits_inline_edges_array() {
    let result = ExplainableStubParagraphLayoutEngine::default().layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from("中文正文")),
            LayoutConstraints::with_defaults(320.0),
        )
        .paragraph_style(ParagraphStyle::builder().first_line_indent(Some(Ic::ZERO)).build())
        .build(),
    );

    assert!(!to_prepared_paragraph_json(&result, true).contains("\"inlineEdges\":"));
}
