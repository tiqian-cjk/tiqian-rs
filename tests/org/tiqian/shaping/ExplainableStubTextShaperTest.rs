use tiqian::org::tiqian::core::Geometry::TextRange;
use tiqian::org::tiqian::core::TextModel::TextStyle;
use tiqian::org::tiqian::font::FontPolicy::{FontCandidate, FontDecision, FontRole};
use tiqian::org::tiqian::shaping::TextShaper::{
    ExplainableStubTextShaper, ShapingInput, TextShaper,
};

fn input(text: &str, role: FontRole, display_text: &str) -> ShapingInput {
    let range = TextRange::new(0, text.encode_utf16().count() as i32);
    ShapingInput::builder(
        text.to_owned(),
        range,
        TextStyle::builder().font_size(16.0).build(),
        FontDecision {
            range,
            candidate: FontCandidate {
                key: "test-font".to_owned(),
                family: "test-font".to_owned(),
                role,
            },
            role,
            reason: "test".to_owned(),
        },
    )
    .display_text(display_text.to_owned())
    .build()
}

#[test]
fn shapes_single_cjk_cluster_with_one_em_advance() {
    let result = ExplainableStubTextShaper.shape(&input("中", FontRole::CjkText, "中"));

    assert_eq!(1, result.clusters.len());
    assert_eq!("中", result.clusters[0].text);
    assert_eq!("中", result.clusters[0].display_text);
    assert_eq!(16.0, result.clusters[0].advance);
    assert_eq!(1, result.glyph_runs[0].glyphs.len());
    assert_eq!("Stub", result.decisions[0].source);
}

#[test]
fn keeps_latin_run_as_single_shaped_cluster_with_nominal_glyphs() {
    let result = ExplainableStubTextShaper.shape(&input("Hello", FontRole::LatinText, "Hello"));

    assert_eq!(1, result.clusters.len());
    assert_eq!("Hello", result.clusters[0].text);
    assert_eq!(80.0, result.clusters[0].advance);
    assert_eq!(5, result.glyph_runs[0].glyphs.len());
    assert_eq!(5, result.decisions[0].glyph_count);
}

#[test]
fn shapes_clreq_dash_substitution_as_two_em_display_cluster() {
    let result = ExplainableStubTextShaper.shape(&input("——", FontRole::CjkPunctuation, "⸺"));

    assert_eq!("——", result.clusters[0].text);
    assert_eq!("⸺", result.clusters[0].display_text);
    assert_eq!(32.0, result.clusters[0].advance);
    assert_eq!(1, result.glyph_runs[0].glyphs.len());
    assert_eq!(32.0, result.glyph_runs[0].glyphs[0].advance);
}
