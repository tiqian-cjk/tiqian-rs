use tiqian::core::geometry::TextRange;
use tiqian::core::text::Text;
use tiqian::core::text_model::TextStyle;
use tiqian::font::font_policy::{FontCandidate, FontDecision, FontRole};
use tiqian::shaping::text_shaper::{
    ExplainableStubTextShaper, ShapingInput, TextShaper,
    ShapingSource, UnimplementedTextShaper, PLATFORM_MULTI_FACE_STRING_DRAW_ISSUE,
    UNVERIFIED_DISPLAY_SUBSTITUTION_COVERAGE_ISSUE,
};

fn input(text: &str, role: FontRole, display_text: &str) -> ShapingInput {
    let range = TextRange::new(0, Text::from(text).utf16_len());
    ShapingInput::builder(
        Text::from(text),
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
    .display_text(Text::from(display_text))
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

#[test]
fn shaping_input_with_features_and_constants() {
    let input = ShapingInput::builder(
        Text::from("Test"),
        TextRange::new(0, 4),
        TextStyle::builder().font_size(16.0).build(),
        FontDecision {
            range: TextRange::new(0, 4),
            candidate: FontCandidate {
                key: "test-font".to_owned(),
                family: "test-font".to_owned(),
                role: FontRole::LatinText,
            },
            role: FontRole::LatinText,
            reason: "coverage-test".to_owned(),
        },
    )
    .display_text(Text::from("Test"))
    .open_type_features(vec!["fwid=1".to_owned(), "vert=1".to_owned()])
    .build();
    assert_eq!(vec!["fwid=1", "vert=1"], input.open_type_features);
    assert_eq!("Test", input.display_text);

    let result = ExplainableStubTextShaper.shape(&input);
    assert_eq!(4, result.decisions[0].glyph_count);
    assert_eq!(4, result.decisions[0].glyphs_without_ink_bounds);
    assert_eq!(
        "ExplainableStubTextShaper:nominal-em-advance",
        result.decisions[0].reason
    );
    assert_eq!("Stub", result.decisions[0].source);
    assert!(!UNVERIFIED_DISPLAY_SUBSTITUTION_COVERAGE_ISSUE.is_empty());
    assert!(!PLATFORM_MULTI_FACE_STRING_DRAW_ISSUE.is_empty());
}

#[test]
fn covers_all_shaping_source_enum_entries() {
    let sources = [
        ShapingSource::Stub,
        ShapingSource::JvmAwt,
        ShapingSource::AndroidPaint,
        ShapingSource::Skia,
        ShapingSource::HarfBuzz,
        ShapingSource::CoreText,
    ];
    assert_eq!(6, sources.len());
    assert_eq!(ShapingSource::Stub, sources[0]);
    assert_eq!(ShapingSource::JvmAwt, sources[1]);
    assert_eq!(ShapingSource::AndroidPaint, sources[2]);
    assert_eq!(ShapingSource::Skia, sources[3]);
    assert_eq!(ShapingSource::HarfBuzz, sources[4]);
    assert_eq!(ShapingSource::CoreText, sources[5]);
}

#[test]
fn unimplemented_text_shaper_throws_on_shape() {
    let error = std::panic::catch_unwind(|| {
        UnimplementedTextShaper.shape(&input("test", FontRole::LatinText, "test"))
    })
    .expect_err("unimplemented shaper must panic");
    let message = error
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| error.downcast_ref::<&str>().copied())
        .unwrap_or("non-string panic payload");
    assert!(message.contains("platform-specific"));
}

#[test]
fn explainable_stub_nominal_advance_branches() {
    let shaper = ExplainableStubTextShaper;
    assert_eq!(
        32.0,
        shaper
            .shape(&input("⸺", FontRole::CjkPunctuation, "⸺"))
            .clusters[0]
            .advance
    );
    assert_eq!(
        32.0,
        shaper
            .shape(&input("——", FontRole::CjkPunctuation, "⸺"))
            .clusters[0]
            .advance
    );
    assert_eq!(
        8.0,
        shaper
            .shape(&input(" ", FontRole::LatinText, " "))
            .clusters[0]
            .advance
    );
    assert_eq!(
        24.0,
        shaper
            .shape(&input("   ", FontRole::LatinText, "   "))
            .clusters[0]
            .advance
    );
    let empty = shaper.shape(&input("", FontRole::LatinText, ""));
    assert_eq!(0.0, empty.clusters[0].advance);
    assert_eq!(1, empty.glyph_runs[0].glyphs.len());
    assert_eq!(
        32.0,
        shaper
            .shape(&input(" a", FontRole::LatinText, " a"))
            .clusters[0]
            .advance
    );
    assert_eq!(
        32.0,
        shaper
            .shape(&input("a ", FontRole::LatinText, "a "))
            .clusters[0]
            .advance
    );
}

#[test]
fn surrogate_pair_handling_in_code_point_count() {
    let shaper = ExplainableStubTextShaper;
    let pair = shaper.shape(&input("😀", FontRole::LatinText, "😀"));
    assert_eq!(1, pair.decisions[0].glyph_count);
    assert_eq!(16.0, pair.clusters[0].advance);

    let multiple_pairs = shaper.shape(&input("😀𠀋", FontRole::LatinText, "😀𠀋"));
    assert_eq!(2, multiple_pairs.decisions[0].glyph_count);
    assert_eq!(32.0, multiple_pairs.clusters[0].advance);
}
