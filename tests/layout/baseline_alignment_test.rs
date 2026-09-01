use tiqian::core::geometry::{LayoutConstraints, TextRange};
use tiqian::core::text::Text;
use tiqian::core::text_model::{
    LayoutInput, ParagraphStyle, TextSpan, TextStyle, TiqianTextContent,
};
use tiqian::core::units::Ic;
use tiqian::layout::paragraph_layout_engine::{
    ExplainableStubParagraphLayoutEngine, ParagraphLayoutEngine,
};

fn layout(content: TiqianTextContent) -> tiqian::core::layout_model::LayoutResult {
    ExplainableStubParagraphLayoutEngine::default().layout(
        LayoutInput::builder(content, LayoutConstraints::with_defaults(400.0))
            .paragraph_style(
                ParagraphStyle::builder()
                    .first_line_indent(Some(Ic::ZERO))
                    .build(),
            )
            .build(),
    )
}

#[test]
fn latin_inside_cjk_uses_shared_roman_baseline() {
    let result = layout(TiqianTextContent::new(Text::from("中A文")));

    assert_eq!(
        0.0,
        result
            .clusters
            .iter()
            .find(|cluster| cluster.text == "A")
            .unwrap()
            .baseline_shift
    );
}

#[test]
fn explicit_baseline_shift_reaches_latin_cluster() {
    let content = TiqianTextContent::builder(Text::from("中A文"))
        .spans(vec![TextSpan {
            range: TextRange::new(1, 2),
            style: TextStyle::builder().baseline_shift(-6.0).build(),
        }])
        .build();
    let result = layout(content);

    assert!(
        (result
            .clusters
            .iter()
            .find(|cluster| cluster.text == "A")
            .unwrap()
            .baseline_shift
            + 6.0)
            .abs()
            < 0.001
    );
}

#[test]
fn cjk_punctuation_provides_ideographic_reference_without_han_body() {
    let result = layout(TiqianTextContent::new(Text::from("MacBook。")));

    assert_eq!(
        0.0,
        result
            .clusters
            .iter()
            .find(|cluster| cluster.text == "。")
            .unwrap()
            .baseline_shift
    );
}

#[test]
fn cjk_mixed_sizes_align_by_ideographic_box_bottom() {
    let content = TiqianTextContent::builder(Text::from("中小大"))
        .spans(vec![
            TextSpan {
                range: TextRange::new(1, 2),
                style: TextStyle::builder().font_size(12.0).build(),
            },
            TextSpan {
                range: TextRange::new(2, 3),
                style: TextStyle::builder().font_size(20.0).build(),
            },
        ])
        .build();
    let result = layout(content);

    assert_eq!(
        0.0,
        result
            .clusters
            .iter()
            .find(|cluster| cluster.text == "中")
            .unwrap()
            .baseline_shift
    );
    assert!(
        (result
            .clusters
            .iter()
            .find(|cluster| cluster.text == "小")
            .unwrap()
            .baseline_shift
            - 0.48)
            .abs()
            < 0.01
    );
    assert!(
        (result
            .clusters
            .iter()
            .find(|cluster| cluster.text == "大")
            .unwrap()
            .baseline_shift
            + 0.48)
            .abs()
            < 0.01
    );
}
