use tiqian::clreq::clreq_profile::{
    AdjustmentStylePolicy, ClreqProfile, ClreqProfileResolver, LineAdjustmentStrategy,
};
use tiqian::core::geometry::LayoutConstraints;
use tiqian::core::text::Text;
use tiqian::core::text_model::{
    LastLineAlignment, LayoutInput, LineLengthGrid, ParagraphStyle, TiqianTextContent,
};
use tiqian::core::units::Ic;
use tiqian::layout::paragraph_layout_engine::{
    ExplainableStubParagraphLayoutEngine, ParagraphLayoutEngine,
};
use tiqian::linebreak::hyphenation::NoHyphenator;

struct PushOutOnlyProfile;

impl ClreqProfileResolver for PushOutOnlyProfile {
    fn resolve(&self, _: &tiqian::core::text_model::LayoutProfileId) -> ClreqProfile {
        let mut profile = ClreqProfile::mainland_horizontal();
        profile.adjustment.line_adjustment = LineAdjustmentStrategy::PushOutOnly;
        profile
    }
}

struct FixedSinoWesternGapProfile;

impl ClreqProfileResolver for FixedSinoWesternGapProfile {
    fn resolve(&self, _: &tiqian::core::text_model::LayoutProfileId) -> ClreqProfile {
        let mut profile = ClreqProfile::mainland_horizontal();
        profile.adjustment = AdjustmentStylePolicy::builder()
            .allow_sino_western_gap_adjustment(false)
            .build();
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
) -> tiqian::core::layout_model::LayoutResult {
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

#[test]
fn sino_western_gap_knob_disables_stretch_and_shrink() {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.clreq_profile_resolver = Box::new(FixedSinoWesternGapProfile);
    let result = engine.layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from("中文Hello文中文中文中文中")),
            LayoutConstraints::with_defaults(160.0),
        )
        .paragraph_style(ParagraphStyle::builder().first_line_indent(Some(Ic::ZERO)).build())
        .build(),
    );

    assert!(!result.debug.justification_decisions.is_empty());
    assert!(result
        .debug
        .justification_decisions
        .iter()
        .flat_map(|decision| &decision.allocations)
        .all(|allocation| allocation.kind != "CjkLatinSpace"));
}

#[test]
fn half_em_word_spaces_do_not_stretch_under_justification() {
    let result = layout(
        "AB CD EF中文中文中",
        160.0,
        ParagraphStyle::builder().first_line_indent(Some(Ic::ZERO)).build(),
    );

    assert!(result.lines.len() >= 2);
    let decision = &result.debug.justification_decisions[0];
    assert_eq!(0.0, decision.deficit_after);
    assert!(decision.allocations.iter().all(|allocation| allocation.kind != "WordSpace"));
    assert!(!decision.allocations.is_empty());
    assert_eq!(160.0, result.lines[0].visual_width);
}

#[test]
fn justify_fills_saturated_line_with_uncapped_even_share() {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.hyphenator = &NoHyphenator;
    let result = engine.layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from("中文中文Network中文")),
            LayoutConstraints::with_defaults(160.0),
        )
        .paragraph_style(ParagraphStyle::builder().first_line_indent(Some(Ic::ZERO)).build())
        .build(),
    );

    let decision = result
        .debug
        .justification_decisions
        .iter()
        .find(|decision| decision.line_range.start() == 0)
        .unwrap();
    assert_eq!(0.0, decision.deficit_after);
    assert_eq!(160.0, result.lines[0].visual_width);
    let deltas: Vec<_> = decision
        .allocations
        .iter()
        .filter(|allocation| allocation.kind == "CjkInterChar")
        .map(|allocation| allocation.delta)
        .collect();
    assert_eq!(3, deltas.len());
    assert!(deltas.iter().all(|delta| (*delta - 32.0).abs() < 0.01));
}
