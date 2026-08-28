use tiqian::org::tiqian::core::Geometry::{LayoutConstraints, TextRange};
use tiqian::org::tiqian::core::TextModel::{
    LayoutInput, ParagraphStyle, TextSpan, TextStyle, TiqianTextContent,
};
use tiqian::org::tiqian::core::Units::Ic;
use tiqian::org::tiqian::layout::ParagraphLayoutEngine::{
    ExplainableStubParagraphLayoutEngine, ParagraphLayoutEngine,
};

fn layout(content: TiqianTextContent) -> tiqian::org::tiqian::core::LayoutModel::LayoutResult {
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
    let result = layout(TiqianTextContent::new("中A文".to_owned()));

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
    let content = TiqianTextContent::builder("中A文".to_owned())
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
fn cjk_mixed_sizes_align_by_ideographic_box_bottom() {
    let content = TiqianTextContent::builder("中小大".to_owned())
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
