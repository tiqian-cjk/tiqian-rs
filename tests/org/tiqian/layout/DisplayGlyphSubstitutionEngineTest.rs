use tiqian::common::HashSet;

use tiqian::clreq::ClreqProfile::{CjkPunctuationGlyphPolicy, ClreqProfile, ClreqProfileResolver};
use tiqian::core::Geometry::LayoutConstraints;
use tiqian::core::Text::Text;
use tiqian::core::TextModel::{LayoutInput, ParagraphStyle, TiqianTextContent};
use tiqian::core::Units::Ic;
use tiqian::layout::ParagraphLayoutEngine::{
    ExplainableStubParagraphLayoutEngine, ParagraphLayoutEngine,
};

struct PreserveInputProfile;

impl ClreqProfileResolver for PreserveInputProfile {
    fn resolve(&self, _: &tiqian::core::TextModel::LayoutProfileId) -> ClreqProfile {
        let mut profile = ClreqProfile::mainland_horizontal();
        profile.punctuation_glyph_policy = CjkPunctuationGlyphPolicy::PreserveInput;
        profile
    }
}

struct SplitDashProfile;

impl ClreqProfileResolver for SplitDashProfile {
    fn resolve(&self, _: &tiqian::core::TextModel::LayoutProfileId) -> ClreqProfile {
        let mut profile = ClreqProfile::mainland_horizontal();
        profile.punctuation_glyph_policy = CjkPunctuationGlyphPolicy::PreserveInput;
        profile.coalesce_repeatable_punctuation = HashSet::new();
        profile
    }
}

fn layout(
    text: &str,
    profile: Option<Box<dyn ClreqProfileResolver>>,
) -> tiqian::core::LayoutModel::LayoutResult {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    if let Some(profile) = profile {
        engine.clreq_profile_resolver = profile;
    }
    engine.layout(
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
fn recommended_display_glyphs_preserve_source_text() {
    let result = layout("……——・／", None);
    let ellipsis = result
        .clusters
        .iter()
        .find(|cluster| cluster.text == "……")
        .unwrap();
    let dash = result
        .clusters
        .iter()
        .find(|cluster| cluster.text == "——")
        .unwrap();
    let interpunct = result
        .clusters
        .iter()
        .find(|cluster| cluster.text == "・")
        .unwrap();
    let solidus = result
        .clusters
        .iter()
        .find(|cluster| cluster.text == "／")
        .unwrap();

    assert_eq!("⋯⋯", ellipsis.display_text);
    assert_eq!("⸺", dash.display_text);
    assert_eq!("·", interpunct.display_text);
    assert_eq!("／", solidus.display_text);
    assert!(
        result
            .clusters
            .iter()
            .all(|cluster| cluster.font_key == "cjk-primary")
    );
}

#[test]
fn profile_can_preserve_input_display_glyphs() {
    let result = layout("……——", Some(Box::new(PreserveInputProfile)));

    assert_eq!(
        "……",
        result
            .clusters
            .iter()
            .find(|cluster| cluster.text == "……")
            .unwrap()
            .display_text
            .as_str()
    );
    assert_eq!(
        "——",
        result
            .clusters
            .iter()
            .find(|cluster| cluster.text == "——")
            .unwrap()
            .display_text
            .as_str()
    );
}

#[test]
fn profile_coalesce_set_controls_repeated_dash_clustering() {
    let result = layout("——", Some(Box::new(SplitDashProfile)));

    assert_eq!(2, result.clusters.len());
    assert_eq!("—", result.clusters[0].text);
    assert_eq!("—", result.clusters[1].text);
}

#[test]
fn recommended_two_em_dash_uses_two_em_advance() {
    let result = layout("⸺", None);

    assert_eq!(1, result.clusters.len());
    assert_eq!(32.0, result.clusters[0].advance);
    assert_eq!(32.0, result.size.width);
}

#[test]
fn stub_shaper_records_named_profile_fallback_without_ink_bounds() {
    let result = layout("中文，世界。", None);

    assert!(!result.debug.punctuation_decisions.is_empty());
    for punctuation in &result.debug.punctuation_decisions {
        assert_eq!(
            "ProfileGlueFallbackWithoutFontGeometry",
            punctuation.geometry_source
        );
        assert_eq!(
            Some("shaper-no-ink-bounds".to_owned()),
            punctuation.ink_bounds_fallback
        );
        assert_eq!(0.0, punctuation.leading_glue_natural);
        assert_eq!(8.0, punctuation.trailing_glue_natural);
    }
}
