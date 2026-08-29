use tiqian::org::tiqian::clreq::ClreqProfile::{
    ClreqProfile, ClreqProfileResolver, KinsokuLevel, KinsokuMode,
};
use tiqian::org::tiqian::core::Geometry::LayoutConstraints;
use tiqian::org::tiqian::core::Text::Text;
use tiqian::org::tiqian::core::TextModel::{
    LayoutInput, LineLengthGrid, ParagraphStyle, TiqianTextContent,
};
use tiqian::org::tiqian::core::Units::Ic;
use tiqian::org::tiqian::layout::ParagraphLayoutEngine::{
    ExplainableStubParagraphLayoutEngine, ParagraphLayoutEngine,
};
use tiqian::org::tiqian::linebreak::Hyphenation::NoHyphenator;

struct FixedBasicProfile;

impl ClreqProfileResolver for FixedBasicProfile {
    fn resolve(&self, _: &tiqian::org::tiqian::core::TextModel::LayoutProfileId) -> ClreqProfile {
        let mut profile = ClreqProfile::mainland_horizontal();
        profile.kinsoku_mode = KinsokuMode::fixed(KinsokuLevel::Basic);
        profile
    }
}

fn engine() -> ExplainableStubParagraphLayoutEngine {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.clreq_profile_resolver = Box::new(FixedBasicProfile);
    engine.hyphenator = &NoHyphenator;
    engine
}

fn layout(
    text: &str,
    max_width: f32,
    grid: bool,
) -> tiqian::org::tiqian::core::LayoutModel::LayoutResult {
    engine().layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from(text)),
            LayoutConstraints::with_defaults(max_width),
        )
        .paragraph_style(
            ParagraphStyle::builder()
                .first_line_indent(Some(Ic::ZERO))
                .line_length_grid(LineLengthGrid::with_enabled(grid))
                .build(),
        )
        .build(),
    )
}

#[test]
fn kinsoku_carries_previous_cluster_when_forbidden_punctuation_would_start_line() {
    let result = layout("中文中文。", 64.0, true);

    assert_eq!(2, result.lines.len());
    assert_eq!(0, result.lines[0].range.start());
    assert_eq!(3, result.lines[0].range.end());
    assert_eq!(3, result.lines[1].range.start());
    assert_eq!(5, result.lines[1].range.end());
    assert_eq!(48.0, result.lines[0].adjusted_width);
    assert_eq!(24.0, result.lines[1].adjusted_width);
    assert_eq!(None, result.debug.line_decisions[0].repair);
    assert_eq!(
        Some("CarryPrevious".to_owned()),
        result.debug.line_decisions[1].repair.clone()
    );
    let repair = result.debug.line_decisions[1]
        .repair_decision
        .as_ref()
        .unwrap();
    assert_eq!("CarryPrevious", repair.kind);
    assert_eq!("ForbiddenAtLineStart", repair.reason_code);
    assert_eq!(Some(3), repair.carried_cluster_index);
}

#[test]
fn kinsoku_pushes_line_start_punctuation_in_when_glue_can_shrink() {
    let result = layout("中文中。", 60.0, false);

    assert_eq!(1, result.lines.len());
    let line = &result.lines[0];
    assert_eq!(0, line.range.start());
    assert_eq!(4, line.range.end());
    assert_eq!(64.0, line.natural_width);
    assert_eq!(56.0, line.adjusted_width);
    assert_eq!(
        Some("PushIn".to_owned()),
        result.debug.line_decisions[0].repair.clone()
    );
    let repair = result.debug.line_decisions[0]
        .repair_decision
        .as_ref()
        .unwrap();
    assert_eq!("PushIn", repair.kind);
    assert_eq!("ForbiddenAtLineStart", repair.reason_code);
    assert_eq!(4.0, repair.shrink);
    assert_eq!(8.0, repair.available_capacity);
}

#[test]
fn numeric_suffix_symbol_remains_on_one_line() {
    let text = "销量增长了50%呢";
    let result = layout(text, 120.0, false);
    let line_texts: Vec<_> = result
        .lines
        .iter()
        .map(|line| {
            text.chars()
                .skip(line.range.start() as usize)
                .take((line.range.end() - line.range.start()) as usize)
                .collect::<String>()
        })
        .collect();

    assert!(
        line_texts.iter().any(|line| line.contains("50%")),
        "50% must stay together: {line_texts:?}"
    );
    assert!(
        line_texts.iter().all(|line| !line.ends_with("50")),
        "no line may end mid-number: {line_texts:?}"
    );
}

#[test]
fn line_end_kinsoku_moves_dangling_opener_to_next_line() {
    let result = layout("中中中（中中）中", 64.0, true);

    for line in &result.lines {
        let last = result
            .clusters
            .iter()
            .rev()
            .find(|cluster| cluster.range.end() <= line.range.end())
            .unwrap();
        assert_ne!("（", last.text, "line must not end on an opening bracket");
    }
    assert!(
        result
            .debug
            .line_decisions
            .iter()
            .any(|decision| decision.repair.as_deref() == Some("CarryNext"))
    );
}
