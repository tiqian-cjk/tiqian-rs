use tiqian::org::tiqian::clreq::ClreqProfile::{
    ClreqProfile, ClreqProfileResolver, LineAdjustmentStrategy,
};
use tiqian::org::tiqian::core::Geometry::LayoutConstraints;
use tiqian::org::tiqian::core::TextModel::{
    LayoutInput, LineLengthGrid, ParagraphStyle, TiqianTextContent,
};
use tiqian::org::tiqian::core::Units::Ic;
use tiqian::org::tiqian::layout::ParagraphLayoutEngine::{
    ExplainableStubParagraphLayoutEngine, ParagraphLayoutEngine,
};
use tiqian::org::tiqian::linebreak::EnglishHyphenation::english_hyphenation;
use tiqian::org::tiqian::linebreak::Hyphenation::{Hyphenator, NoHyphenator};

struct PushOutOnlyProfile;

impl ClreqProfileResolver for PushOutOnlyProfile {
    fn resolve(&self, _: &tiqian::org::tiqian::core::TextModel::LayoutProfileId) -> ClreqProfile {
        let mut profile = ClreqProfile::mainland_horizontal();
        profile.adjustment.line_adjustment = LineAdjustmentStrategy::PushOutOnly;
        profile
    }
}

fn layout_with(
    hyphenator: &'static dyn Hyphenator,
    text: &str,
    max_width: f32,
) -> tiqian::org::tiqian::core::LayoutModel::LayoutResult {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.hyphenator = hyphenator;
    engine.layout(
        LayoutInput::builder(
            TiqianTextContent::new(text.to_owned()),
            LayoutConstraints::with_defaults(max_width),
        )
        .paragraph_style(
            ParagraphStyle::builder()
                .first_line_indent(Some(Ic::ZERO))
                .line_length_grid(LineLengthGrid::with_enabled(false))
                .build(),
        )
        .build(),
    )
}

#[test]
fn fitting_word_hyphenates_only_when_a_hyphenator_is_injected() {
    let no_hyphen = layout_with(&NoHyphenator, "中文中 coffee", 112.0);
    let hyphenated = layout_with(english_hyphenation::en_us(), "中文中 coffee", 112.0);

    assert!(
        no_hyphen
            .clusters
            .iter()
            .any(|cluster| cluster.text == "coffee")
    );
    assert!(
        no_hyphen
            .lines
            .iter()
            .all(|line| line.hyphen_advance == 0.0)
    );
    assert!(
        hyphenated
            .clusters
            .iter()
            .all(|cluster| cluster.text != "coffee")
    );
    assert!(
        hyphenated
            .clusters
            .iter()
            .any(|cluster| cluster.text == "cof")
    );
    assert!(
        hyphenated
            .clusters
            .iter()
            .any(|cluster| cluster.text == "fee")
    );
    assert!(
        hyphenated
            .lines
            .iter()
            .any(|line| line.hyphen_advance > 0.0)
    );
}

#[test]
fn hyphenated_syllable_clusters_match_the_injected_hyphenator() {
    let text = "中文internationalization中文";
    let result = layout_with(english_hyphenation::en_us(), text, 160.0);
    let word = "internationalization";
    let rebuilt = result
        .clusters
        .iter()
        .filter(|cluster| {
            !cluster.text.is_empty()
                && cluster
                    .text
                    .chars()
                    .all(|character| character.is_ascii_alphabetic())
        })
        .map(|cluster| cluster.text.as_str())
        .collect::<Vec<_>>()
        .join("-");
    let offsets = english_hyphenation::en_us().hyphenate(word);
    let mut expected = String::new();
    let mut previous = 0;
    for offset in offsets {
        expected.push_str(&word[previous..offset as usize]);
        expected.push('-');
        previous = offset as usize;
    }
    expected.push_str(&word[previous..]);

    assert_eq!(expected, rebuilt);
}

#[test]
fn reserved_hyphen_squeezes_comma_glue_before_hanging_residual() {
    let result = layout_with(
        english_hyphenation::en_us(),
        "中文，internationalization",
        128.0,
    );
    let comma = result
        .clusters
        .iter()
        .find(|cluster| cluster.text == "，")
        .unwrap();

    assert!(
        comma.advance < 16.0,
        "comma glue not compressed for the hyphen: {}",
        comma.advance
    );
}

#[test]
fn hyphen_is_reserved_inside_measure() {
    let result = layout_with(
        english_hyphenation::en_us(),
        "请运行 internationalization 命令",
        160.0,
    );
    let line = result
        .lines
        .iter()
        .find(|line| line.hyphen_advance > 0.0)
        .unwrap();

    assert!(line.indent + line.visual_width + line.hyphen_advance <= 160.01);
}

#[test]
fn tight_cjk_stretch_avoids_hyphenation_with_push_out_only() {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.clreq_profile_resolver = Box::new(PushOutOnlyProfile);
    let result = engine.layout(
        LayoutInput::builder(
            TiqianTextContent::new("中文中文中文中文 coffee".to_owned()),
            LayoutConstraints::with_defaults(180.0),
        )
        .paragraph_style(
            ParagraphStyle::builder()
                .first_line_indent(Some(Ic::ZERO))
                .line_length_grid(LineLengthGrid::with_enabled(false))
                .build(),
        )
        .build(),
    );

    assert!(result.lines.iter().all(|line| line.hyphen_advance == 0.0));
}
