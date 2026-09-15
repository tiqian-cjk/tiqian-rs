use tiqian::common::HashMap;
use tiqian::clreq::clreq_profile::PunctuationClass;
use tiqian::core::font_face::FontFaceId;
use tiqian::core::geometry::{scalar_offset, text_range, LayoutConstraints};
use tiqian::core::layout_model::{Cluster, EmergencyTrackingEligibilityDecisionInfo};
use tiqian::core::text::Text;
use tiqian::core::text_model::{
    INLINE_OBJECT_REPLACEMENT_CHAR, InlineObjectSpan, LayoutInput, ParagraphStyle,
    TiqianTextContent,
};
use tiqian::font::font_policy::FontRole;
use tiqian::layout::line_break_planning_stage::{
    plan_paragraph_lines, LineBreakPlanningRequest, ParagraphLayoutPrep,
};
use tiqian::layout::paragraph_layout_engine::ExplainableStubParagraphLayoutEngine;
use tiqian::layout::progressive_break_decisions::{
    ProgressiveBreakOpportunity, ProgressiveBreakTier,
};
use tiqian::layout::width_independent_annotation_cache::{
    build_paragraph_layout_prep, prepare_width_independent_annotation,
};
use tiqian::shaping::font_backend::{FontCandidateAttempt, FontResolution};

fn base_prep(engine: &ExplainableStubParagraphLayoutEngine, text: &str) -> ParagraphLayoutPrep {
    let input = LayoutInput::builder(
        TiqianTextContent::new(Text::from(text)),
        LayoutConstraints::with_defaults(200.0),
    )
    .paragraph_style(ParagraphStyle::default())
    .build();
    let rejected = HashMap::new();
    let annotation = prepare_width_independent_annotation(
        &input,
        &rejected,
        engine.clreq_profile_resolver.as_ref(),
        engine.font_role_classifier.as_ref(),
        engine.font_backend.as_ref(),
        &engine.quote_pair_analyzer,
        engine.hyphenator,
    );
    build_paragraph_layout_prep(
        &input,
        &annotation,
        &rejected,
        engine.font_backend.as_ref(),
        engine.hyphenator,
        &engine.punctuation_atom_builder,
        &engine.punctuation_spacing_compressor,
    )
}

fn plan(
    engine: &ExplainableStubParagraphLayoutEngine,
    prep: &ParagraphLayoutPrep,
) -> tiqian::layout::line_break_planning_stage::LineBreakPlanningStageResult {
    plan_paragraph_lines(LineBreakPlanningRequest::new(
        prep,
        engine.font_backend.as_ref(),
        engine.font_metrics_normalizer.as_ref(),
        &engine.justifier,
        engine.line_breaker.as_ref(),
    ))
}

#[test]
fn test_font_resolution_metrics_use_resolved_face() {
    let engine = ExplainableStubParagraphLayoutEngine::default();
    let mut prep = base_prep(&engine, "abcdef");
    let bad_cluster = Cluster::new(text_range(0, 5), Text::from("abcde"), FontFaceId::with_resource_id("test"), 50.0);
    let bad_resolution = FontResolution::new(
        text_range(0, 3),
        FontRole::LatinText,
        FontFaceId::with_resource_id("test"),
        vec![FontCandidateAttempt::new(
            "test".to_owned(),
            FontFaceId::with_resource_id("test"),
            0,
        )],
    );
    prep.natural_clusters = vec![bad_cluster.clone()];
    prep.clusters = vec![bad_cluster];
    prep.font_resolutions = HashMap::from([(bad_resolution.range, bad_resolution)]);

    let result = plan(&engine, &prep);
    assert_eq!(1, result.metric_decisions.len());
    assert_eq!(text_range(0, 3), result.metric_decisions[0].range);
    assert_eq!(FontFaceId::with_resource_id("test"), result.metric_decisions[0].request.face);
    assert_eq!(FontRole::LatinText, result.metric_decisions[0].request.role);
}

#[test]
fn test_font_decision_with_no_matching_clusters_uses_text_substring() {
    let engine = ExplainableStubParagraphLayoutEngine::default();
    let mut prep = base_prep(&engine, "abcdef");
    let resolution = FontResolution::new(
        text_range(4, 6),
        FontRole::LatinText,
        FontFaceId::with_resource_id("test"),
        vec![FontCandidateAttempt::new(
            "test".to_owned(),
            FontFaceId::with_resource_id("test"),
            0,
        )],
    );
    let cluster = Cluster::new(text_range(0, 2), Text::from("ab"), FontFaceId::with_resource_id("test"), 20.0);
    prep.natural_clusters = vec![cluster.clone()];
    prep.clusters = vec![cluster];
    prep.font_resolutions = HashMap::from([(resolution.range, resolution)]);

    let result = plan(&engine, &prep);
    assert_eq!(1, result.metric_decisions.len());
    assert_eq!(FontFaceId::with_resource_id("test"), result.metric_decisions[0].request.face);
    assert_eq!(text_range(4, 6), result.metric_decisions[0].range);
}

#[test]
fn test_ascii_point_mark_kinsoku_line_start() {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    let result = tiqian::layout::paragraph_layout_engine::ParagraphLayoutEngine::layout(
        &mut engine,
        LayoutInput::builder(
            TiqianTextContent::new(Text::from("hello, world")),
            LayoutConstraints::with_defaults(50.0),
        )
        .build(),
    );
    assert!(!result.lines.is_empty());
}

#[test]
fn test_inline_object_kinsoku_line_start() {
    let text = format!("{INLINE_OBJECT_REPLACEMENT_CHAR}hello");
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    let result = tiqian::layout::paragraph_layout_engine::ParagraphLayoutEngine::layout(
        &mut engine,
        LayoutInput::builder(
            TiqianTextContent::new(Text::from(text)),
            LayoutConstraints::with_defaults(50.0),
        )
        .inline_objects(vec![InlineObjectSpan::with_fixed_boundaries(
            text_range(0, 1),
            16.0,
            8.0,
            8.0,
        )])
        .build(),
    );
    assert!(!result.lines.is_empty());
}

#[test]
fn test_progressive_break_offsets_unmapped_cluster_index() {
    let engine = ExplainableStubParagraphLayoutEngine::default();
    let mut prep = base_prep(&engine, "abc");
    prep.progressive_break_offsets = HashMap::from([(
        scalar_offset(999),
        ProgressiveBreakOpportunity::new(ProgressiveBreakTier::Whitespace, text_range(0, 3)),
    )]);

    let result = plan(&engine, &prep);
    assert!(result.progressive_break_opportunities.is_empty());
}

#[test]
fn test_emergency_tracking_eligibility_decisions_branches() {
    let engine = ExplainableStubParagraphLayoutEngine::default();
    let mut prep = base_prep(&engine, "中文字符");
    prep.emergency_tracking_eligibility_decisions = vec![
        EmergencyTrackingEligibilityDecisionInfo {
            range: text_range(100, 200),
            source_text: Text::from("unmapped"),
            reason: "reason".to_owned(),
        },
        EmergencyTrackingEligibilityDecisionInfo {
            range: text_range(0, 4),
            source_text: Text::from("中文字符"),
            reason: "validReason".to_owned(),
        },
        EmergencyTrackingEligibilityDecisionInfo {
            range: text_range(0, 4),
            source_text: Text::from("中文字符"),
            reason: "duplicateReason".to_owned(),
        },
    ];

    assert!(!plan(&engine, &prep).line_solution.lines.is_empty());
}

#[test]
fn test_emergency_tracking_boundary_whitespace_and_empty() {
    let engine = ExplainableStubParagraphLayoutEngine::default();
    let mut prep = base_prep(&engine, "ab");
    let clusters = vec![
        Cluster::new(text_range(0, 0), Text::from(""), FontFaceId::with_resource_id("test"), 0.0),
        Cluster::new(text_range(0, 1), Text::from("a"), FontFaceId::with_resource_id("test"), 10.0),
        Cluster::new(text_range(1, 1), Text::from(""), FontFaceId::with_resource_id("test"), 0.0),
        Cluster::new(text_range(1, 2), Text::from("b"), FontFaceId::with_resource_id("test"), 10.0),
    ];
    prep.natural_clusters = clusters.clone();
    prep.clusters = clusters;
    prep.cluster_roles = vec![FontRole::LatinText; 4];
    prep.east_asian_spacing_edges = vec![prep.east_asian_spacing_edges[0]; 4];
    prep.natural_inline_attachments = vec![Default::default(); 4];
    prep.emergency_tracking_eligibility_decisions = vec![
        EmergencyTrackingEligibilityDecisionInfo {
            range: text_range(0, 2),
            source_text: Text::from("ab"),
            reason: "reason".to_owned(),
        },
    ];

    assert!(!plan(&engine, &prep).line_solution.lines.is_empty());
}

#[test]
fn test_adjustable_inline_boundary_right_clusters_no_stretch_boundaries() {
    let engine = ExplainableStubParagraphLayoutEngine::default();
    let mut prep = base_prep(&engine, "中文字符排版");
    prep.uniform_inline_object_boundary_after_clusters = [0, 1, 3].into_iter().collect();
    prep.atom_class_by_range = HashMap::from([
        (text_range(0, 1), PunctuationClass::Dash),
        (text_range(2, 3), PunctuationClass::Connector),
    ]);

    assert!(!plan(&engine, &prep).line_solution.lines.is_empty());
}
