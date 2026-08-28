use tiqian::org::tiqian::core::Geometry::LayoutConstraints;
use tiqian::org::tiqian::core::TextModel::{LayoutInput, ParagraphStyle, TiqianTextContent};
use tiqian::org::tiqian::core::Units::Ic;
use tiqian::org::tiqian::layout::ParagraphLayoutEngine::{
    ExplainableStubParagraphLayoutEngine, ParagraphLayoutEngine,
};

const FIXTURES: [&str; 5] = [
    "中文，中文。",
    "他说：“你好，世界。”！！",
    "中（中文）文中文中文中",
    "有人说：「先有咖啡馆，后有启蒙运动」。每座城市、每条街巷、每个清晨都有人在等一杯 espresso……这并不是巧合。",
    "读报、辩论、下棋、写作——城市生活忽然多出一个公共客厅。",
];

#[test]
fn punctuation_resolved_advance_never_falls_below_body_width() {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    for text in FIXTURES {
        for max_width in [48.0, 64.0, 80.0, 100.0, 160.0, 320.0] {
            let result = engine.layout(
                LayoutInput::builder(
                    TiqianTextContent::new(text.to_owned()),
                    LayoutConstraints::with_defaults(max_width),
                )
                .paragraph_style(
                    ParagraphStyle::builder()
                        .first_line_indent(Some(Ic::ZERO))
                        .build(),
                )
                .build(),
            );
            for geometry in &result.debug.geometry_decisions {
                assert!(
                    geometry.resolved_advance >= geometry.body_width - 0.001,
                    "body floor violated for {:?} ({:?}) at max_width={max_width}: resolved={} < body={}",
                    geometry.source_text,
                    geometry.range,
                    geometry.resolved_advance,
                    geometry.body_width,
                );
            }
        }
    }
}
