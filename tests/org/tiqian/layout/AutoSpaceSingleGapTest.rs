use tiqian::org::tiqian::clreq::ClreqProfile::{
    AutoSpaceMode, AutoSpacePolicy, ClreqProfile, ClreqProfileResolver,
};
use tiqian::org::tiqian::core::Geometry::LayoutConstraints;
use tiqian::org::tiqian::core::Text::Text;
use tiqian::org::tiqian::core::TextModel::{LayoutInput, ParagraphStyle, TiqianTextContent};
use tiqian::org::tiqian::core::Units::Ic;
use tiqian::org::tiqian::layout::ParagraphLayoutEngine::{
    ExplainableStubParagraphLayoutEngine, ParagraphLayoutEngine,
};

struct LetterOnlyAutoSpace;

impl ClreqProfileResolver for LetterOnlyAutoSpace {
    fn resolve(&self, _: &tiqian::org::tiqian::core::TextModel::LayoutProfileId) -> ClreqProfile {
        let mut profile = ClreqProfile::mainland_horizontal();
        profile.auto_space = AutoSpacePolicy {
            cjk_latin: AutoSpaceMode::Insert,
            cjk_digit: AutoSpaceMode::Disabled,
            ..AutoSpacePolicy::default()
        };
        profile
    }
}

fn layout(text: &str) -> tiqian::org::tiqian::core::LayoutModel::LayoutResult {
    ExplainableStubParagraphLayoutEngine::default().layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from(text)),
            LayoutConstraints::with_defaults(320.0),
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
fn authored_space_runs_collapse_to_one_autospace_gap() {
    let one = layout("中文 CJK 段落");
    let two = layout("中文  CJK 段落");
    let three = layout("中文   CJK段落");

    assert!(
        one.clusters
            .iter()
            .filter(|cluster| cluster.text == " ")
            .all(|cluster| cluster.advance == 2.0)
    );
    assert_eq!(
        2.0,
        two.clusters
            .iter()
            .find(|cluster| cluster.text == "  ")
            .unwrap()
            .advance
    );
    assert_eq!(
        2.0,
        three
            .clusters
            .iter()
            .find(|cluster| cluster.text == "   ")
            .unwrap()
            .advance
    );
    assert_eq!(
        50.0,
        three
            .clusters
            .iter()
            .find(|cluster| cluster.text == "CJK")
            .unwrap()
            .advance
    );
}

#[test]
fn absent_authored_space_inserts_one_gap_at_each_cjk_latin_edge() {
    let result = layout("中文CJK段落");
    let latin = result
        .clusters
        .iter()
        .find(|cluster| cluster.text == "CJK")
        .unwrap();

    assert_eq!(52.0, latin.advance);
    assert_eq!(2, result.debug.auto_space_decisions.len());
    assert!(
        result
            .debug
            .auto_space_decisions
            .iter()
            .all(|decision| decision.mode == "Insert")
    );
    assert!(
        result
            .debug
            .auto_space_decisions
            .iter()
            .all(|decision| decision.characters_affected == 0)
    );
    assert!(
        result
            .debug
            .auto_space_decisions
            .iter()
            .all(|decision| decision.total_reduction == -2.0)
    );
}

#[test]
fn letter_and_digit_boundaries_follow_separate_profile_modes() {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.clreq_profile_resolver = Box::new(LetterOnlyAutoSpace);
    let result = engine.layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from("甲A乙9丙")),
            LayoutConstraints::with_defaults(320.0),
        )
        .paragraph_style(
            ParagraphStyle::builder()
                .first_line_indent(Some(Ic::ZERO))
                .build(),
        )
        .build(),
    );
    let a_range = result
        .clusters
        .iter()
        .find(|cluster| cluster.text == "A")
        .unwrap()
        .range;
    let nine_range = result
        .clusters
        .iter()
        .find(|cluster| cluster.text == "9")
        .unwrap()
        .range;

    assert!(!result.debug.auto_space_decisions.is_empty());
    assert!(
        result
            .debug
            .auto_space_decisions
            .iter()
            .all(|decision| decision.cluster_range == a_range)
    );
    assert!(
        result
            .debug
            .auto_space_decisions
            .iter()
            .all(|decision| decision.cluster_range != nine_range)
    );
}
