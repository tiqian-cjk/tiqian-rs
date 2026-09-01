use tiqian::common::HashSet;
use tiqian::clreq::clreq_profile::{CjkPunctuationGlyphPolicy, ClreqProfile, ClreqProfileResolver};
use tiqian::core::geometry::{LayoutConstraints, Rect, TextRange};
use tiqian::core::layout_model::{Cluster, Glyph, GlyphRun, ShapingDecisionInfo};
use tiqian::core::text::Text;
use tiqian::core::text_model::{
    LayoutInput, LineLengthGrid, ParagraphStyle, TextSpan, TextStyle, TiqianTextContent,
};
use tiqian::core::units::Ic;
use tiqian::layout::line_breaker::LookaheadLineBreaker;
use tiqian::layout::paragraph_layout_engine::{
    ExplainableStubParagraphLayoutEngine, ParagraphLayoutEngine,
};
use tiqian::shaping::text_shaper::{
    ExplainableStubTextShaper, ShapingInput, ShapingResult, TextShaper,
    UNVERIFIED_DISPLAY_SUBSTITUTION_COVERAGE_ISSUE,
};

struct PreserveInputProfile;

impl ClreqProfileResolver for PreserveInputProfile {
    fn resolve(&self, _: &tiqian::core::text_model::LayoutProfileId) -> ClreqProfile {
        let mut profile = ClreqProfile::mainland_horizontal();
        profile.punctuation_glyph_policy = CjkPunctuationGlyphPolicy::PreserveInput;
        profile
    }
}

struct SplitDashProfile;

impl ClreqProfileResolver for SplitDashProfile {
    fn resolve(&self, _: &tiqian::core::text_model::LayoutProfileId) -> ClreqProfile {
        let mut profile = ClreqProfile::mainland_horizontal();
        profile.punctuation_glyph_policy = CjkPunctuationGlyphPolicy::PreserveInput;
        profile.coalesce_repeatable_punctuation = HashSet::new();
        profile
    }
}

fn input(text: &str) -> LayoutInput {
    LayoutInput::builder(
        TiqianTextContent::new(Text::from(text)),
        LayoutConstraints::with_defaults(320.0),
    )
    .paragraph_style(
        ParagraphStyle::builder()
            .first_line_indent(Some(Ic::ZERO))
            .build(),
    )
    .build()
}

#[test]
fn preserves_source_text_when_using_clreq_recommended_display_glyphs() {
    let result = ExplainableStubParagraphLayoutEngine::default().layout(input("……——・／"));
    for (source, display) in [("……", "⋯⋯"), ("——", "⸺"), ("・", "·"), ("／", "／")] {
        let cluster = result.clusters.iter().find(|cluster| cluster.text == source).unwrap();
        assert_eq!(source, cluster.text);
        assert_eq!(display, cluster.display_text);
        assert_eq!("cjk-primary", cluster.font_key);
    }
}

#[test]
fn honors_profile_punctuation_glyph_policy() {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.clreq_profile_resolver = Box::new(PreserveInputProfile);
    let result = engine.layout(input("……——"));
    assert_eq!("……", result.clusters.iter().find(|cluster| cluster.text == "……").unwrap().display_text);
    assert_eq!("——", result.clusters.iter().find(|cluster| cluster.text == "——").unwrap().display_text);
}

#[test]
fn coalesce_set_is_driven_by_profile() {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.clreq_profile_resolver = Box::new(SplitDashProfile);
    assert_eq!(
        vec!["—", "—"],
        engine.layout(input("——")).clusters.iter().map(|cluster| cluster.text.as_str()).collect::<Vec<_>>(),
    );
    assert_eq!(
        vec!["A", "—", "—", "B"],
        engine.layout(input("A——B")).clusters.iter().map(|cluster| cluster.text.as_str()).collect::<Vec<_>>(),
    );
}

#[test]
fn uses_two_em_advance_for_recommended_dash_codepoint() {
    let result = ExplainableStubParagraphLayoutEngine::default().layout(input("⸺"));
    assert_eq!(1, result.clusters.len());
    assert_eq!(32.0, result.clusters[0].advance);
    assert_eq!(32.0, result.size.width);
}

struct FeatureBoundaryTextShaper;

impl TextShaper for FeatureBoundaryTextShaper {
    fn shape(&self, input: &ShapingInput) -> ShapingResult {
        let source = input.text.slice_text(input.range);
        if input.display_text != "A’B" {
            return ExplainableStubTextShaper.shape(input);
        }
        let clusters: Vec<_> = (input.range.start()..input.range.end())
            .map(|index| {
                let range = TextRange::new(index, index + 1);
                Cluster::with_display_text(
                    range,
                    input.text.slice_text(range),
                    input.display_text.slice_text(TextRange::new(index - input.range.start(), index - input.range.start() + 1)),
                    input.font_decision.candidate.key.clone(),
                    16.0,
                )
            })
            .collect();
        let glyph_runs = clusters
            .iter()
            .enumerate()
            .map(|(glyph_id, cluster)| {
                let features = (cluster.text == "’").then(|| vec!["pwid".to_owned(), "palt".to_owned()]).unwrap_or_default();
                GlyphRun::with_open_type_features(
                    cluster.range,
                    cluster.font_key.clone(),
                    vec![Glyph::builder(glyph_id as u32, cluster.range, cluster.advance).build()],
                    cluster.advance,
                    features,
                )
            })
            .collect();
        assert_eq!("A’B", source);
        ShapingResult::new(clusters, glyph_runs)
    }
}

#[test]
fn preserves_open_type_features_as_final_glyph_run_boundaries() {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.text_shaper = Box::new(FeatureBoundaryTextShaper);
    let result = engine.layout(input("A’B"));
    assert_eq!(
        vec![TextRange::new(0, 1), TextRange::new(1, 2), TextRange::new(2, 3)],
        result.glyph_runs.iter().map(|run| run.range).collect::<Vec<_>>(),
    );
    assert_eq!(
        vec![Vec::<String>::new(), vec!["pwid".to_owned(), "palt".to_owned()], Vec::new()],
        result.glyph_runs.iter().map(|run| run.open_type_features.clone()).collect::<Vec<_>>(),
    );
}

struct NoBoundsTextShaper;

impl TextShaper for NoBoundsTextShaper {
    fn shape(&self, input: &ShapingInput) -> ShapingResult {
        let source = input.text.slice_text(input.range);
        ShapingResult::new(
            vec![Cluster::with_display_text(
                input.range,
                source,
                input.display_text.clone(),
                input.font_decision.candidate.key.clone(),
                16.0,
            )],
            vec![GlyphRun::new(
                input.range,
                input.font_decision.candidate.key.clone(),
                vec![Glyph::builder(0, input.range, 16.0).build()],
                16.0,
            )],
        )
    }
}

#[test]
fn shaping_without_bounds_produces_named_profile_fallback() {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.text_shaper = Box::new(NoBoundsTextShaper);
    let result = engine.layout(input("。"));
    assert_eq!(1, result.debug.punctuation_decisions.len());
    let punctuation = &result.debug.punctuation_decisions[0];
    assert_eq!("ProfileGlueFallbackWithoutFontGeometry", punctuation.geometry_source);
    assert_eq!(Some("shaper-no-ink-bounds".to_owned()), punctuation.ink_bounds_fallback);
    assert_eq!(8.0, punctuation.body_width);
    assert_eq!(0.0, punctuation.leading_glue_natural);
    assert_eq!(8.0, punctuation.trailing_glue_natural);
}

struct MissingGlyphTextShaper;

impl TextShaper for MissingGlyphTextShaper {
    fn shape(&self, input: &ShapingInput) -> ShapingResult {
        let mut result = ExplainableStubTextShaper.shape(input);
        if input.display_text.contains('⸺') {
            result.decisions = result
                .decisions
                .into_iter()
                .map(|decision| ShapingDecisionInfo { missing_glyphs: 1, ..decision })
                .collect();
        }
        result
    }
}

#[test]
fn substitution_rolls_back_to_source_text_when_font_lacks_the_glyph() {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.text_shaper = Box::new(MissingGlyphTextShaper);
    let result = engine.layout(input("中——文"));
    assert_eq!(
        "——",
        result.clusters.iter().find(|cluster| cluster.text == "——").unwrap().display_text,
    );
    let decision = result.debug.font_decisions.iter().find(|decision| decision.source_text == "——").unwrap();
    assert_eq!("——", decision.display_text);
    assert!(decision.substitution_reason.ends_with("SubstitutionRollbackOnMissingGlyph"));
}

struct UnverifiedCoverageTextShaper;

impl TextShaper for UnverifiedCoverageTextShaper {
    fn shape(&self, input: &ShapingInput) -> ShapingResult {
        let mut result = ExplainableStubTextShaper.shape(input);
        if input.display_text.contains('⋯') {
            result.decisions = result
                .decisions
                .into_iter()
                .map(|decision| ShapingDecisionInfo {
                    capability_issue: Some(UNVERIFIED_DISPLAY_SUBSTITUTION_COVERAGE_ISSUE.to_owned()),
                    ..decision
                })
                .collect();
        }
        result
    }
}

#[test]
fn ellipsis_substitution_rolls_back_when_coverage_cannot_be_verified() {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.text_shaper = Box::new(UnverifiedCoverageTextShaper);
    let result = engine.layout(input("中……文"));
    assert_eq!(
        "……",
        result.clusters.iter().find(|cluster| cluster.text == "……").unwrap().display_text,
    );
    assert!(result
        .debug
        .font_decisions
        .iter()
        .find(|decision| decision.source_text == "……")
        .unwrap()
        .substitution_reason
        .ends_with("SubstitutionRollbackOnUnverifiedGlyphCoverage"));
}

struct UnderfilledDashInkTextShaper;

impl TextShaper for UnderfilledDashInkTextShaper {
    fn shape(&self, input: &ShapingInput) -> ShapingResult {
        let mut result = ExplainableStubTextShaper.shape(input);
        if input.display_text.contains('⸺') {
            for run in &mut result.glyph_runs {
                for glyph in &mut run.glyphs {
                    glyph.advance = 32.0;
                    glyph.bounds = Some(tiqian::core::geometry::Rect {
                        left: 1.0,
                        top: -10.0,
                        right: 26.0,
                        bottom: -8.0,
                    });
                }
            }
        }
        result
    }
}

#[test]
fn dash_substitution_rolls_back_when_ink_does_not_fill_the_two_em_advance() {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.text_shaper = Box::new(UnderfilledDashInkTextShaper);
    let result = engine.layout(input("中——文"));
    assert_eq!(
        "——",
        result.clusters.iter().find(|cluster| cluster.text == "——").unwrap().display_text,
    );
    assert!(result
        .debug
        .font_decisions
        .iter()
        .find(|decision| decision.source_text == "——")
        .unwrap()
        .substitution_reason
        .ends_with("DashSubstitutionInkCoverageRollback"));
}

struct OneEmFallbackDashTextShaper;

impl TextShaper for OneEmFallbackDashTextShaper {
    fn shape(&self, input: &ShapingInput) -> ShapingResult {
        let mut result = ExplainableStubTextShaper.shape(input);
        if input.display_text.contains('⸺') {
            for cluster in &mut result.clusters {
                cluster.advance = 16.0;
            }
            for run in &mut result.glyph_runs {
                run.advance = 16.0;
                for glyph in &mut run.glyphs {
                    glyph.advance = 16.0;
                    glyph.bounds = Some(Rect {
                        left: 0.5,
                        top: -9.0,
                        right: 15.7,
                        bottom: -7.0,
                    });
                }
            }
            for decision in &mut result.decisions {
                decision.advance = 16.0;
            }
        }
        result
    }
}

#[test]
fn dash_substitution_rolls_back_when_fallback_reports_a_full_one_em_glyph() {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.text_shaper = Box::new(OneEmFallbackDashTextShaper);
    let result = engine.layout(input("中——文"));
    assert_eq!(
        "——",
        result.clusters.iter().find(|cluster| cluster.text == "——").unwrap().display_text,
    );
    assert!(result
        .debug
        .font_decisions
        .iter()
        .find(|decision| decision.source_text == "——")
        .unwrap()
        .substitution_reason
        .ends_with("DashSubstitutionInkCoverageRollback"));
}

struct DashSpanSizeTextShaper;

impl TextShaper for DashSpanSizeTextShaper {
    fn shape(&self, input: &ShapingInput) -> ShapingResult {
        let mut result = ExplainableStubTextShaper.shape(input);
        if input.display_text.contains('⸺') {
            for cluster in &mut result.clusters {
                cluster.advance = 32.0;
            }
            for run in &mut result.glyph_runs {
                run.advance = 32.0;
                for glyph in &mut run.glyphs {
                    glyph.advance = 32.0;
                    glyph.bounds = Some(Rect {
                        left: 1.0,
                        top: -18.0,
                        right: 31.0,
                        bottom: -14.0,
                    });
                }
            }
            for decision in &mut result.decisions {
                decision.advance = 32.0;
            }
        }
        result
    }
}

#[test]
fn dash_coverage_target_uses_the_dash_span_font_size() {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.text_shaper = Box::new(DashSpanSizeTextShaper);
    let result = engine.layout(
        LayoutInput::builder(
            TiqianTextContent::builder(Text::from("中——文"))
                .spans(vec![TextSpan {
                    range: TextRange::new(1, 3),
                    style: TextStyle::builder().font_size(32.0).build(),
                }])
                .build(),
            LayoutConstraints::with_defaults(320.0),
        )
        .paragraph_style(
            ParagraphStyle::builder()
                .first_line_indent(Some(Ic::ZERO))
                .build(),
        )
        .build(),
    );
    assert_eq!(
        "——",
        result.clusters.iter().find(|cluster| cluster.text == "——").unwrap().display_text,
    );
}

struct CenteredDashInkTextShaper;

impl TextShaper for CenteredDashInkTextShaper {
    fn shape(&self, input: &ShapingInput) -> ShapingResult {
        let mut result = ExplainableStubTextShaper.shape(input);
        if input.display_text.contains('⸺') {
            for run in &mut result.glyph_runs {
                for glyph in &mut run.glyphs {
                    glyph.advance = 32.0;
                    glyph.bounds = Some(Rect {
                        left: 0.5,
                        top: -10.0,
                        right: 28.0,
                        bottom: -8.0,
                    });
                }
            }
        }
        result
    }
}

#[test]
fn dash_ink_centers_within_the_two_em_body_when_the_font_rule_underfills() {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.text_shaper = Box::new(CenteredDashInkTextShaper);
    let result = engine.layout(input("中——文"));
    let dash = result.clusters.iter().find(|cluster| cluster.text == "——").unwrap();
    assert_eq!("⸺", dash.display_text);
    let glyph = result
        .glyph_runs
        .iter()
        .flat_map(|run| &run.glyphs)
        .find(|glyph| glyph.cluster_range == dash.range)
        .unwrap();
    assert!((glyph.x - 1.75).abs() <= 0.01, "glyph.x={}", glyph.x);
}

struct FullDashInkTextShaper;

impl TextShaper for FullDashInkTextShaper {
    fn shape(&self, input: &ShapingInput) -> ShapingResult {
        let mut result = ExplainableStubTextShaper.shape(input);
        if input.display_text.contains('⸺') {
            for run in &mut result.glyph_runs {
                for glyph in &mut run.glyphs {
                    glyph.advance = 32.0;
                    glyph.bounds = Some(Rect {
                        left: 1.0,
                        top: -10.0,
                        right: 31.0,
                        bottom: -8.0,
                    });
                }
            }
        }
        result
    }
}

#[test]
fn dash_substitution_is_kept_when_ink_fills_the_two_em_advance() {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.text_shaper = Box::new(FullDashInkTextShaper);
    let result = engine.layout(input("中——文"));
    assert_eq!(
        "⸺",
        result.clusters.iter().find(|cluster| cluster.text == "——").unwrap().display_text,
    );
}

#[test]
fn substitution_is_kept_when_font_covers_the_glyph() {
    let result = ExplainableStubParagraphLayoutEngine::default().layout(input("中——文"));
    assert_eq!(
        "⸺",
        result.clusters.iter().find(|cluster| cluster.text == "——").unwrap().display_text,
    );
}

struct AmbiguousGlyphClusterTextShaper;

impl TextShaper for AmbiguousGlyphClusterTextShaper {
    fn shape(&self, input: &ShapingInput) -> ShapingResult {
        ShapingResult::new(
            vec![Cluster::with_display_text(
                input.range,
                input.text.slice_text(input.range),
                input.display_text.clone(),
                input.font_decision.candidate.key.clone(),
                32.0,
            )],
            vec![GlyphRun::new(
                input.range,
                input.font_decision.candidate.key.clone(),
                vec![Glyph::builder(0, input.range, 32.0)
                    .bounds(Some(Rect {
                        left: 2.0,
                        top: -10.0,
                        right: 30.0,
                        bottom: -6.0,
                    }))
                    .build()],
                32.0,
            )],
        )
    }
}

#[test]
fn ambiguous_glyph_cluster_mapping_falls_back_to_policy_with_recorded_reason() {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.text_shaper = Box::new(AmbiguousGlyphClusterTextShaper);
    let result = engine.layout(input("……"));
    assert_eq!(2, result.debug.punctuation_decisions.len());
    for punctuation in &result.debug.punctuation_decisions {
        assert_eq!("ProfileGlueFallbackWithoutFontGeometry", punctuation.geometry_source);
        assert_eq!(
            Some("glyph-cluster-mapping-ambiguous".to_owned()),
            punctuation.ink_bounds_fallback,
        );
    }
}

struct CharacterLocalInkTextShaper;

impl TextShaper for CharacterLocalInkTextShaper {
    fn shape(&self, input: &ShapingInput) -> ShapingResult {
        if input.display_text != "⋯⋯" {
            return ExplainableStubTextShaper.shape(input);
        }
        ShapingResult::new(
            vec![Cluster::with_display_text(
                input.range,
                input.text.slice_text(input.range),
                input.display_text.clone(),
                input.font_decision.candidate.key.clone(),
                32.0,
            )],
            vec![GlyphRun::new(
                input.range,
                input.font_decision.candidate.key.clone(),
                vec![
                    Glyph::builder(1, input.range, 16.0)
                        .x(0.0)
                        .bounds(Some(Rect {
                            left: 1.5,
                            top: -7.0,
                            right: 14.5,
                            bottom: -5.0,
                        }))
                        .build(),
                    Glyph::builder(2, input.range, 16.0)
                        .x(16.0)
                        .bounds(Some(Rect {
                            left: 1.5,
                            top: -7.0,
                            right: 14.5,
                            bottom: -5.0,
                        }))
                        .build(),
                ],
                32.0,
            )],
        )
    }
}

#[test]
fn multi_character_punctuation_uses_character_local_ink_bounds() {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.text_shaper = Box::new(CharacterLocalInkTextShaper);
    let result = engine.layout(input("……"));
    assert_eq!(2, result.debug.punctuation_decisions.len());
    assert_eq!(
        vec![Some(8.0), Some(8.0)],
        result
            .debug
            .punctuation_decisions
            .iter()
            .map(|decision| decision.ink_center)
            .collect::<Vec<_>>(),
    );
    assert_eq!(
        vec![16.0, 16.0],
        result
            .debug
            .punctuation_decisions
            .iter()
            .map(|decision| decision.advance)
            .collect::<Vec<_>>(),
    );
}

#[test]
fn rolled_back_dash_still_keeps_its_boundaries_closed_under_justification() {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.line_breaker = Box::new(LookaheadLineBreaker::default());
    engine.text_shaper = Box::new(UnderfilledDashInkTextShaper);
    let text = "在所谓中文语境下——不如说中文中文中文中文";
    let hit = (13..=30).find_map(|cells| {
        let result = engine.layout(
            LayoutInput::builder(
                TiqianTextContent::new(Text::from(text)),
                LayoutConstraints::with_defaults(cells as f32 * 16.0 + 7.0),
            )
            .paragraph_style(
                ParagraphStyle::builder()
                    .first_line_indent(Some(Ic::ZERO))
                    .line_length_grid(LineLengthGrid::with_enabled(false))
                    .build(),
            )
            .build(),
        );
        let dash = result.clusters.iter().find(|cluster| cluster.text == "——")?;
        let decision = result.debug.justification_decisions.iter().find(|decision| {
            dash.range.start() >= decision.line_range.start()
                && dash.range.end() <= decision.line_range.end()
                && !decision.allocations.is_empty()
        })?;
        Some((dash.clone(), decision.clone()))
    }).expect("no width produced a justified line containing the dash");

    let (dash, decision) = hit;
    assert_eq!("——", dash.display_text);
    assert!(decision.allocations.iter().all(|allocation| {
        allocation.kind != "CjkInterChar" || allocation.cluster_range != dash.range
    }), "allocations={:?}", decision.allocations);
}
