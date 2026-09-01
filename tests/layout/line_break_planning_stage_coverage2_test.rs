use tiqian::common::HashMap;
use tiqian::clreq::clreq_profile::PunctuationClass;
use tiqian::core::geometry::{LayoutConstraints, TextRange};
use tiqian::core::layout_model::{Cluster, EmergencyTrackingEligibilityDecisionInfo};
use tiqian::core::text::Text;
use tiqian::core::text_model::{
    INLINE_OBJECT_REPLACEMENT_CHAR, InlineObjectSpan, LayoutInput, ParagraphStyle,
    TiqianTextContent,
};
use tiqian::font::font_policy::{FontCandidate, FontDecision, FontRole};
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
        engine.fallback_resolver.as_ref(),
        engine.font_metrics_resolver.as_ref(),
        &engine.quote_pair_analyzer,
        engine.text_shaper.as_ref(),
        engine.hyphenator,
    );
    build_paragraph_layout_prep(
        &input,
        &annotation,
        &rejected,
        engine.text_shaper.as_ref(),
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
        engine.font_metrics_resolver.as_ref(),
        engine.font_metrics_normalizer.as_ref(),
        &engine.justifier,
        engine.line_breaker.as_ref(),
    ))
}

#[test]
#[should_panic(expected = "crosses font decision")]
fn test_cluster_crosses_font_decision_throws() {
    let engine = ExplainableStubParagraphLayoutEngine::default();
    let mut prep = base_prep(&engine, "abcdef");
    let bad_cluster = Cluster::new(TextRange::new(0, 5), Text::from("abcde"), "test".to_owned(), 50.0);
    let bad_decision = FontDecision {
        range: TextRange::new(0, 3),
        candidate: FontCandidate {
            key: "test".to_owned(),
            family: "test".to_owned(),
            role: FontRole::LatinText,
        },
        role: FontRole::LatinText,
        reason: "test".to_owned(),
    };
    prep.natural_clusters = vec![bad_cluster.clone()];
    prep.clusters = vec![bad_cluster];
    prep.font_decisions = vec![bad_decision];

    plan(&engine, &prep);
}

#[test]
fn test_font_decision_with_no_matching_clusters_uses_text_substring() {
    let engine = ExplainableStubParagraphLayoutEngine::default();
    let mut prep = base_prep(&engine, "abcdef");
    let decision = FontDecision {
        range: TextRange::new(4, 6),
        candidate: FontCandidate {
            key: "test".to_owned(),
            family: "test".to_owned(),
            role: FontRole::LatinText,
        },
        role: FontRole::LatinText,
        reason: "test".to_owned(),
    };
    let cluster = Cluster::new(TextRange::new(0, 2), Text::from("ab"), "test".to_owned(), 20.0);
    prep.natural_clusters = vec![cluster.clone()];
    prep.clusters = vec![cluster];
    prep.font_decisions = vec![decision];

    let result = plan(&engine, &prep);
    assert_eq!(1, result.metric_decisions.len());
    assert_eq!("ef", result.metric_decisions[0].request.face_selection_text.as_str());
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
            TextRange::new(0, 1),
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
        999,
        ProgressiveBreakOpportunity::new(ProgressiveBreakTier::Whitespace, TextRange::new(0, 3)),
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
            range: TextRange::new(100, 200),
            source_text: Text::from("unmapped"),
            reason: "reason".to_owned(),
        },
        EmergencyTrackingEligibilityDecisionInfo {
            range: TextRange::new(0, 4),
            source_text: Text::from("中文字符"),
            reason: "validReason".to_owned(),
        },
        EmergencyTrackingEligibilityDecisionInfo {
            range: TextRange::new(0, 4),
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
        Cluster::new(TextRange::new(0, 0), Text::from(""), "test".to_owned(), 0.0),
        Cluster::new(TextRange::new(0, 1), Text::from("a"), "test".to_owned(), 10.0),
        Cluster::new(TextRange::new(1, 1), Text::from(""), "test".to_owned(), 0.0),
        Cluster::new(TextRange::new(1, 2), Text::from("b"), "test".to_owned(), 10.0),
    ];
    prep.natural_clusters = clusters.clone();
    prep.clusters = clusters;
    prep.cluster_roles = vec![FontRole::LatinText; 4];
    prep.east_asian_spacing_edges = vec![prep.east_asian_spacing_edges[0]; 4];
    prep.natural_inline_attachments = vec![Default::default(); 4];
    prep.emergency_tracking_eligibility_decisions = vec![
        EmergencyTrackingEligibilityDecisionInfo {
            range: TextRange::new(0, 2),
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
        (TextRange::new(0, 1), PunctuationClass::Dash),
        (TextRange::new(2, 3), PunctuationClass::Connector),
    ]);

    assert!(!plan(&engine, &prep).line_solution.lines.is_empty());
}
