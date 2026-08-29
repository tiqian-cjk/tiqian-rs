use tiqian::org::tiqian::clreq::ClreqProfile::{
    ClreqProfile, ClreqProfileResolver, LineAdjustmentStrategy,
};
use tiqian::org::tiqian::core::Geometry::LayoutConstraints;
use tiqian::org::tiqian::core::Text::Text;
use tiqian::org::tiqian::core::TextModel::{
    LastLineAlignment, LayoutInput, LineLengthGrid, ParagraphStyle, TiqianTextContent,
};
use tiqian::org::tiqian::core::Units::Ic;
use tiqian::org::tiqian::layout::ParagraphLayoutEngine::{
    ExplainableStubParagraphLayoutEngine, ParagraphLayoutEngine,
};

struct PushOutOnlyProfile;

impl ClreqProfileResolver for PushOutOnlyProfile {
    fn resolve(&self, _: &tiqian::org::tiqian::core::TextModel::LayoutProfileId) -> ClreqProfile {
        let mut profile = ClreqProfile::mainland_horizontal();
        profile.adjustment.line_adjustment = LineAdjustmentStrategy::PushOutOnly;
        profile
    }
}

fn engine() -> ExplainableStubParagraphLayoutEngine {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.clreq_profile_resolver = Box::new(PushOutOnlyProfile);
    engine
}

fn layout(
    text: &str,
    max_width: f32,
    style: ParagraphStyle,
) -> tiqian::org::tiqian::core::LayoutModel::LayoutResult {
    engine().layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from(text)),
            LayoutConstraints::with_defaults(max_width),
        )
        .paragraph_style(style)
        .build(),
    )
}

fn exact_measure_style() -> ParagraphStyle {
    ParagraphStyle::builder()
        .first_line_indent(Some(Ic::ZERO))
        .line_length_grid(LineLengthGrid::with_enabled(false))
        .build()
}

#[test]
fn connector_boundaries_remain_closed_during_justification() {
    let result = layout(
        "中～文中Example",
        80.0,
        ParagraphStyle::builder()
            .first_line_indent(Some(Ic::ZERO))
            .build(),
    );

    assert!(result.lines.len() >= 2);
    let decision = &result.debug.justification_decisions[0];
    assert_eq!(0.0, decision.deficit_after);
    let inter_char: Vec<_> = decision
        .allocations
        .iter()
        .filter(|allocation| allocation.kind == "CjkInterChar")
        .collect();
    assert_eq!(
        vec![2],
        inter_char
            .iter()
            .map(|allocation| allocation.cluster_range.start())
            .collect::<Vec<_>>()
    );
    assert_eq!(16.0, inter_char[0].delta);
}

#[test]
fn inseparable_number_and_unit_boundary_remains_closed_during_justification() {
    let text = "中文50℃中文中文中文Example";
    let result = layout(text, 128.0, exact_measure_style());
    let number = result
        .clusters
        .iter()
        .find(|cluster| cluster.range.start() <= 2 && cluster.range.end() >= 4)
        .unwrap();
    let unit = result
        .clusters
        .iter()
        .find(|cluster| cluster.range.start() <= 4 && cluster.range.end() >= 5)
        .unwrap();
    assert_ne!(number.range, unit.range);
    let decision = result
        .debug
        .justification_decisions
        .iter()
        .find(|decision| {
            number.range.start() >= decision.line_range.start()
                && unit.range.end() <= decision.line_range.end()
                && !decision.allocations.is_empty()
        })
        .expect("expected a justified line containing 50℃");

    assert!(decision.allocations.iter().all(|allocation| {
        allocation.cluster_range != number.range
            || (allocation.kind != "CjkLatinSpace" && allocation.kind != "CjkInterChar")
    }));
    assert_eq!(0.0, decision.deficit_after);
}

#[test]
fn last_line_is_never_justified() {
    let result = layout(
        "中文中",
        80.0,
        ParagraphStyle::builder()
            .first_line_indent(Some(Ic::ZERO))
            .build(),
    );

    assert_eq!(1, result.lines.len());
    let line = &result.lines[0];
    assert_eq!(48.0, line.adjusted_width);
    assert_eq!(48.0, line.visual_width);
    assert!(result.debug.justification_decisions.is_empty());
}

#[test]
fn mandatory_and_paragraph_end_lines_take_last_line_alignment() {
    let style = ParagraphStyle::builder()
        .first_line_indent(Some(Ic::ZERO))
        .last_line_alignment(LastLineAlignment::Center)
        .line_length_grid(LineLengthGrid::with_enabled(false))
        .build();
    let result = layout("中文中\n中文中文中文中", 100.0, style);

    assert_eq!(3, result.lines.len());
    assert_eq!(26.0, result.lines[0].indent);
    assert_eq!(0.0, result.lines[1].indent);
    assert_eq!(42.0, result.lines[2].indent);
}
