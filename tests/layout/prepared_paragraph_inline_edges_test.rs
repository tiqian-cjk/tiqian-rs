use tiqian::core::geometry::{text_range, LayoutConstraints};
use tiqian::core::text::Text;
use tiqian::core::text_model::{InlineBoxSpan, LayoutInput, ParagraphStyle, TiqianTextContent};
use tiqian::core::units::Ic;
use tiqian::api::ParagraphLayoutEngineBuilder;
use crate::support::DeterministicStubFontBackend;
use tiqian::layout::prepared_paragraph::to_prepared_paragraph_json;

#[test]
fn end_only_inline_box_emits_edge_without_inline_start_field() {
    let result = ParagraphLayoutEngineBuilder::new(Box::new(DeterministicStubFontBackend::default())).build().layout(
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
    let result = ParagraphLayoutEngineBuilder::new(Box::new(DeterministicStubFontBackend::default())).build().layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from("中文正文")),
            LayoutConstraints::with_defaults(320.0),
        )
        .paragraph_style(ParagraphStyle::builder().first_line_indent(Some(Ic::ZERO)).build())
        .build(),
    );

    assert!(!to_prepared_paragraph_json(&result, true).contains("\"inlineEdges\":"));
}

#[test]
fn non_finite_inline_box_edges_do_not_leak_into_render_plan() {
    let result = ParagraphLayoutEngineBuilder::new(Box::new(DeterministicStubFontBackend::default())).build().layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from("中文正文")),
            LayoutConstraints::with_defaults(320.0),
        )
        .paragraph_style(ParagraphStyle::builder().first_line_indent(Some(Ic::ZERO)).build())
        .inline_boxes(vec![InlineBoxSpan::with_edges(text_range(0, 2), f32::NAN, f32::INFINITY)])
        .build(),
    );

    let json = to_prepared_paragraph_json(&result, true);
    assert!(!json.contains("NaN"), "{json}");
    assert!(!json.contains("Infinity"), "{json}");
}
