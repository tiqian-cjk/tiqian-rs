use crate::support::DeterministicStubFontBackend;
use tiqian::core::font_face::{FontFaceId, FontVariationInstance};
use tiqian::core::geometry::{scalar_offset, text_range};
use tiqian::core::layout_model::{Cluster, Glyph, GlyphRun};
use tiqian::core::text::Text;
use tiqian::core::text_model::TextStyle;
use tiqian::font::font_metrics::FontMetricsRequest;
use tiqian::font::font_policy::FontRole;
use tiqian::shaping::font_backend::{
    FontBackend, FontBackendRequest, FontBackendShapingResult, FontCandidateAttempt,
};
use tiqian::shaping::replayable_font_backend::ReplayableFontCatalog;
use tiqian::shaping::text_shaper::ShapingResult;

fn request(text: &str, display_text: &str, role: FontRole) -> FontBackendRequest {
    let text = Text::from(text);
    FontBackendRequest::builder(
        text.clone(),
        text_range(0, text.scalar_len().value()),
        TextStyle::builder().font_size(20.0).build(),
        role,
    )
    .display_text(Text::from(display_text))
    .build()
}

fn face(resource_id: &str) -> FontFaceId {
    FontFaceId::new(resource_id.to_owned(), 0, FontVariationInstance::default())
}

#[test]
fn stub_backend_shapes_display_text_with_the_role_selected_controlled_face() {
    let backend = DeterministicStubFontBackend::default();
    for (role, expected_face) in [
        (FontRole::CjkText, "cjk-primary"),
        (FontRole::CjkPunctuation, "cjk-primary"),
        (FontRole::LatinText, "latin-primary"),
        (FontRole::Symbol, "symbol-fallback"),
        (FontRole::Emoji, "symbol-fallback"),
        (FontRole::Unknown, "symbol-fallback"),
    ] {
        let shaped = backend.shape(&request("——", "⸺", role));
        assert_eq!(expected_face, shaped.face.resource_id(), "{role:?}");
        assert_eq!(Text::from("——"), shaped.shaping.clusters[0].text);
        assert_eq!(Text::from("⸺"), shaped.shaping.clusters[0].display_text);
        assert_eq!(1, shaped.shaping.glyph_runs[0].glyphs.len());
        assert_eq!(
            Some(&shaped.face),
            shaped.shaping.glyph_runs[0].glyphs[0]
                .render_font_face
                .as_ref()
        );
        assert_eq!(40.0, shaped.shaping.glyph_runs[0].advance);
        assert_eq!(0, shaped.selected_attempt().unwrap().missing_glyphs);
    }
}

#[test]
fn stub_backend_keeps_a_replayable_placeholder_for_empty_display_text() {
    let backend = DeterministicStubFontBackend::default();
    let shaped = backend.shape(&request("甲", "", FontRole::CjkText));

    assert_eq!(1, shaped.shaping.glyph_runs[0].glyphs.len());
    assert_eq!(1, shaped.shaping.decisions[0].glyph_count);
    assert_eq!(1, shaped.shaping.decisions[0].glyphs_without_ink_bounds);
    assert_eq!(20.0, shaped.shaping.glyph_runs[0].advance);
}

#[test]
fn stub_backend_exposes_catalog_and_metrics_for_the_same_controlled_face() {
    let backend = DeterministicStubFontBackend::default();
    assert_eq!(3, backend.faces().len());
    assert_eq!(
        "DeterministicStubFontBackend",
        backend.capability_report().backend
    );
    assert!(
        backend
            .capability_report()
            .can_replay_from_controlled_bytes()
    );
    assert!(backend.face(&face("cjk-primary")).is_some());
    assert!(backend.face(&face("missing")).is_none());

    let cjk = backend.metrics(&FontMetricsRequest::new(
        face("cjk-primary"),
        20.0,
        FontRole::CjkText,
        "zh-Hans".to_owned(),
    ));
    let latin = backend.metrics(&FontMetricsRequest::new(
        face("latin-primary"),
        20.0,
        FontRole::LatinText,
        "en".to_owned(),
    ));
    assert!((cjk.typo_ascent.unwrap() - 17.6).abs() < 1e-5);
    assert!((cjk.typo_descent.unwrap() - 2.4).abs() < 1e-5);
    assert_eq!(None, latin.typo_ascent);
    assert_eq!(16.0, latin.ascent);
}

#[test]
fn font_backend_result_uses_the_first_complete_candidate_and_preserves_all_missing_evidence() {
    let primary = face("primary");
    let fallback = face("fallback");
    let shaping = ShapingResult::new(
        vec![Cluster::new(
            text_range(0, 1),
            Text::from("A"),
            fallback.clone(),
            10.0,
        )],
        vec![GlyphRun::new(
            text_range(0, 1),
            fallback.clone(),
            vec![Glyph::builder(1, text_range(0, 1), 10.0).build()],
            10.0,
        )],
    );
    let fallback_result = FontBackendShapingResult::new(
        fallback.clone(),
        shaping.clone(),
        vec![
            FontCandidateAttempt::new("primary".to_owned(), primary.clone(), 1),
            FontCandidateAttempt::new("fallback".to_owned(), fallback.clone(), 0),
        ],
    );
    assert_eq!(
        "fallback",
        fallback_result.selected_attempt().unwrap().candidate_key
    );
    assert_eq!(
        fallback,
        fallback_result
            .resolution(text_range(0, 1), FontRole::LatinText)
            .selected_attempt()
            .unwrap()
            .face
    );

    let all_missing = FontBackendShapingResult::new(
        primary.clone(),
        shaping,
        vec![
            FontCandidateAttempt::new("primary".to_owned(), primary.clone(), 1),
            FontCandidateAttempt::new("fallback".to_owned(), fallback, 2),
        ],
    );
    assert_eq!(
        "primary",
        all_missing.selected_attempt().unwrap().candidate_key
    );
    assert_eq!(2, all_missing.attempts.len());
    assert!(
        all_missing
            .attempts
            .iter()
            .all(FontCandidateAttempt::has_missing_glyphs)
    );
    assert_eq!(
        scalar_offset(0),
        all_missing
            .resolution(text_range(0, 1), FontRole::LatinText)
            .range
            .start()
    );
}

#[test]
fn font_backend_result_allows_missing_selected_evidence() {
    let primary = face("primary");
    let fallback = face("fallback");
    let shaping = ShapingResult::new(
        vec![Cluster::new(
            text_range(0, 1),
            Text::from("A"),
            primary.clone(),
            10.0,
        )],
        vec![GlyphRun::new(
            text_range(0, 1),
            primary.clone(),
            vec![Glyph::builder(1, text_range(0, 1), 10.0).build()],
            10.0,
        )],
    );

    let empty_evidence =
        FontBackendShapingResult::new(primary.clone(), shaping.clone(), Vec::new());
    assert_eq!(None, empty_evidence.selected_attempt());
    assert_eq!(
        None,
        empty_evidence
            .resolution(text_range(0, 1), FontRole::LatinText)
            .selected_attempt(),
    );

    let inconsistent_evidence = FontBackendShapingResult::new(
        primary,
        shaping,
        vec![FontCandidateAttempt::new(
            "fallback".to_owned(),
            fallback,
            0,
        )],
    );
    assert_eq!(None, inconsistent_evidence.selected_attempt());
}
