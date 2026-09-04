use tiqian::clreq::clreq_profile::{
    AutoSpaceMode, AutoSpacePolicy, ClreqProfile, ClreqProfileResolver,
};
use tiqian::core::geometry::{text_range, LayoutConstraints};
use tiqian::core::text::Text;
use tiqian::core::text_model::{
    InlineAttachment, LayoutInput, ParagraphStyle, TextSpan, TextStyle, TiqianTextContent,
};
use tiqian::core::units::Ic;
use tiqian::layout::paragraph_layout_engine::{
    ExplainableStubParagraphLayoutEngine, ParagraphLayoutEngine,
};

struct LetterOnlyAutoSpace;

impl ClreqProfileResolver for LetterOnlyAutoSpace {
    fn resolve(&self, _: &tiqian::core::text_model::LayoutProfileId) -> ClreqProfile {
        let mut profile = ClreqProfile::mainland_horizontal();
        profile.auto_space = AutoSpacePolicy {
            cjk_latin: AutoSpaceMode::Insert,
            cjk_digit: AutoSpaceMode::Disabled,
            ..AutoSpacePolicy::default()
        };
        profile
    }
}

struct DisabledAutoSpace;

impl ClreqProfileResolver for DisabledAutoSpace {
    fn resolve(&self, _: &tiqian::core::text_model::LayoutProfileId) -> ClreqProfile {
        let mut profile = ClreqProfile::mainland_horizontal();
        profile.auto_space = AutoSpacePolicy::disabled();
        profile
    }
}

fn layout(text: &str) -> tiqian::core::layout_model::LayoutResult {
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
fn one_typed_space_becomes_one_autospace_gap() {
    let result = layout("中文 CJK 段落");
    assert!(
        result.clusters
            .iter()
            .filter(|cluster| cluster.text == " ")
            .all(|cluster| cluster.advance == 2.0)
    );
    assert_eq!(48.0, result.clusters.iter().find(|cluster| cluster.text == "CJK").unwrap().advance);
}

#[test]
fn auto_space_replaces_typed_space_at_cjk_latin_boundary() {
    let result = layout("中文 CJK 段落");
    let spaces: Vec<_> = result.clusters.iter().filter(|cluster| cluster.text == " ").collect();

    assert_eq!(2, spaces.len());
    assert!(spaces.iter().all(|space| space.advance == 2.0));
    assert_eq!(2, result.debug.auto_space_decisions.len());
    assert!(result.debug.auto_space_decisions.iter().all(|decision| {
        decision.mode == "Replace" && decision.side == "gap" && decision.total_reduction == 6.0
    }));
}

#[test]
fn auto_space_does_not_shrink_spaces_between_latin_words() {
    let result = layout("Hello world");

    assert_eq!(3, result.clusters.len());
    assert_eq!(8.0, result.clusters.iter().find(|cluster| cluster.text == " ").unwrap().advance);
    assert!(result.debug.auto_space_decisions.is_empty());
}

#[test]
fn auto_space_disabled_keeps_typed_spaces_at_half_em() {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.clreq_profile_resolver = Box::new(DisabledAutoSpace);
    let result = engine.layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from("中文 CJK 段落")),
            LayoutConstraints::with_defaults(320.0),
        )
        .paragraph_style(ParagraphStyle::builder().first_line_indent(Some(Ic::ZERO)).build())
        .build(),
    );
    let spaces: Vec<_> = result.clusters.iter().filter(|cluster| cluster.text == " ").collect();

    assert_eq!(2, spaces.len());
    assert!(spaces.iter().all(|space| space.advance == 8.0));
    assert!(result.debug.auto_space_decisions.is_empty());
}

#[test]
fn two_typed_spaces_at_boundary_still_collapse_to_one_gap() {
    let result = layout("中文  CJK 段落");
    assert_eq!(
        2.0,
        result.clusters
            .iter()
            .find(|cluster| cluster.text == "  ")
            .unwrap()
            .advance
    );
    assert_eq!(2.0, result.clusters.iter().find(|cluster| cluster.text == " ").unwrap().advance);
}

#[test]
fn three_typed_spaces_still_one_gap() {
    let result = layout("中文   CJK段落");
    assert_eq!(
        2.0,
        result.clusters
            .iter()
            .find(|cluster| cluster.text == "   ")
            .unwrap()
            .advance
    );
    assert_eq!(
        50.0,
        result.clusters
            .iter()
            .find(|cluster| cluster.text == "CJK")
            .unwrap()
            .advance
    );
}

fn layout_with_attached_reference(text: &str) -> tiqian::core::layout_model::LayoutResult {
    ExplainableStubParagraphLayoutEngine::default().layout(
        LayoutInput::builder(
            TiqianTextContent::builder(Text::from(text))
                .spans(vec![TextSpan {
                    range: text_range(2, 3),
                    style: TextStyle::builder().inline_attachment(InlineAttachment::Previous).build(),
                }])
                .build(),
            LayoutConstraints::with_defaults(320.0),
        )
        .paragraph_style(ParagraphStyle::builder().first_line_indent(Some(Ic::ZERO)).build())
        .build(),
    )
}

#[test]
fn attached_reference_between_cjk_text_does_not_invent_an_autospace_gap() {
    assert!(layout_with_attached_reference("正文1后文").debug.auto_space_decisions.is_empty());
}

#[test]
fn attached_reference_before_latin_text_gets_the_virtual_cjk_latin_gap() {
    let decision = &layout_with_attached_reference("正文1ABC").debug.auto_space_decisions[0];
    assert_eq!("trailing", decision.side);
    assert_eq!("InlineAttachment.Previous", decision.boundary_role);
    assert_eq!("AttachedInlineVirtualAutoSpace:east-asian-spacing-W-N", decision.reason);
}

#[test]
fn attached_reference_at_paragraph_end_has_no_autospace_gap() {
    assert!(layout_with_attached_reference("正文1").debug.auto_space_decisions.is_empty());
}

#[test]
fn unicode_east_asian_spacing_covers_narrow_scripts_without_script_whitelists() {
    for sample in ["α", "я", "ա"] {
        let result = layout(&format!("中{sample}文"));
        let narrow = result.clusters.iter().find(|cluster| cluster.text == sample).unwrap();
        assert_eq!(20.0, narrow.advance, "sample={sample}");
        assert_eq!(2, result.debug.auto_space_decisions.len(), "sample={sample}");
        assert!(result.debug.auto_space_decisions.iter().all(|decision| decision.cluster_range == narrow.range));
        assert!(result.debug.auto_space_decisions.iter().all(|decision| decision.reason == "TextAutoSpaceInsert:east-asian-spacing-W-N"));
    }
}

#[test]
fn conditional_punctuation_follows_chinese_language_resolution() {
    let result = layout("中%文");
    assert_eq!(2, result.debug.auto_space_decisions.len());
    assert!(result.debug.auto_space_decisions.iter().all(|decision| decision.boundary_role == "EastAsianSpacing.Wide"));
}

#[test]
fn autospace_does_not_fire_between_latin_and_cjk_punctuation() {
    assert!(layout("Tiqian ）说明").debug.auto_space_decisions.is_empty());
}

#[test]
fn autospace_does_not_fire_before_slash_led_latin_technical_run() {
    let result = layout("恐跨/TERFism。如果");
    let technical = result.clusters.iter().find(|cluster| cluster.text == "/TERFism").unwrap();
    assert!(result.debug.auto_space_decisions.iter().all(|decision| decision.cluster_range != technical.range || decision.side != "leading"));
}

#[test]
fn autospace_still_fires_between_latin_and_cjk_text_even_with_punctuation_nearby() {
    let decisions = &layout("中文 shaping 之后").debug.auto_space_decisions;
    assert_eq!(2, decisions.len());
    assert!(decisions.iter().all(|decision| decision.boundary_role == "EastAsianSpacing.Wide"));
    assert!(decisions.iter().all(|decision| decision.side == "gap"));
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
