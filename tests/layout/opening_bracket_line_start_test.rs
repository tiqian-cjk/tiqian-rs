use tiqian::core::geometry::LayoutConstraints;
use tiqian::core::text::Text;
use tiqian::core::text_model::{LayoutInput, ParagraphStyle, TiqianTextContent};
use tiqian::core::units::Ic;
use tiqian::layout::paragraph_layout_engine::{
    ExplainableStubParagraphLayoutEngine, ParagraphLayoutEngine,
};

#[test]
fn opening_bracket_at_line_start_uses_half_width_leading_trim() {
    let text = "这是第一行测试文字这是第一行测试\n（Shaping & Font Metrics）这是第二行文字\n（GPOS / GSUB 特性表查询）这是第三行文字";
    let input = LayoutInput::builder(
        TiqianTextContent::new(Text::from(text)),
        LayoutConstraints::with_defaults(672.0),
    )
    .paragraph_style(
        ParagraphStyle::builder()
            .first_line_indent(Some(Ic::ZERO))
            .build(),
    )
    .build();
    let result = ExplainableStubParagraphLayoutEngine::default().layout(input);

    assert_eq!(3, result.lines.len());
    for line in result.lines.iter().skip(1) {
        let first = &result.clusters[line.cluster_range.first() as usize];
        assert_eq!("（", first.text);
        assert!((first.advance - 8.0).abs() < 0.01);
    }
    let start_trims: Vec<_> = result
        .debug
        .line_edge_trim_decisions
        .iter()
        .filter(|decision| decision.reason == "LineStartHalfWidthPunctuation")
        .collect();
    assert_eq!(2, start_trims.len());
    assert!(
        start_trims
            .iter()
            .all(|decision| decision.side == "leading"
                && (decision.trim_amount - 8.0).abs() < 0.01)
    );
}
