// 对应 Kotlin 源文件：engine/src/commonMain/kotlin/org/tiqian/layout/LayoutDebugAssembly.kt

use std::collections::HashMap;

use super::super::clreq::ClreqProfile::ClreqPunctuationGlyphSubstitutor;
use super::super::core::Geometry::TextRange;
use super::super::core::LayoutModel::{
    AutoSpaceDecisionInfo, BopomofoDecisionInfo, BreakOpportunityDecisionInfo, Cluster,
    ClusterGeometryDecisionInfo, ContextualKinsokuDecisionInfo, DecorationDecisionInfo,
    DecorationSegmentInfo, EmergencyTrackingEligibilityDecisionInfo, FontDecisionInfo,
    InlineBoxDecisionInfo, InlineObjectDecisionInfo, InlineObjectLineHeightDecisionInfo,
    InlineObjectPunctuationAttachmentDecisionInfo, JustificationAllocationInfo,
    JustificationDecisionInfo, KinsokuDecisionInfo, LayoutDebugInfo, LineBox,
    LineEdgeTrimDecisionInfo, LineLengthGridDecisionInfo, LineRepairAllocationInfo,
    LineRepairCandidateInfo, LineRepairDecisionInfo, LineSpacingDecisionInfo,
    MandatoryBreakDecisionInfo, MaxLinesDecisionInfo, MetricDecisionInfo, PunctuationDecisionInfo,
    RoleOverrideInfo, RubyDecisionInfo, RubyLineHeightDecisionInfo, ShapingDecisionInfo,
    SpacingDecisionInfo, ZeroWidthBreakDecisionInfo,
};
use super::super::core::Text::Text;
use super::super::font::FontPolicy::{FontDecision, FontRoleClassifier, FontRoleContext};
use super::Justifier::JustificationPlan;
use super::LineGeometryStage::ClusterMetricDecision;
use super::LineOptimization::{LineSolution, RepairCandidate, RepairOption};
use super::ProgressiveBreakDecisions::ProgressiveBreakOpportunity;
use super::PunctuationGeometryLedger::AttachedInlinePunctuationBoundaryResult;
use super::PunctuationModel::{PunctuationAtom, PunctuationSpacingCompressionResult};
use super::QuotePairAnalyzer::QuoteRoleDecision;

pub struct LayoutDebugStageInput<'a> {
    pub text: &'a Text,
    pub font_decisions: &'a [FontDecision],
    pub punctuation_glyph_substitutor: &'a ClreqPunctuationGlyphSubstitutor,
    pub substitution_rollbacks: &'a HashMap<TextRange, String>,
    pub shaping_decisions: &'a [ShapingDecisionInfo],
    pub metric_decisions: &'a [ClusterMetricDecision],
    pub punctuation_atoms: &'a [PunctuationAtom],
    pub geometry_decisions: &'a [ClusterGeometryDecisionInfo],
    pub spacing_plan: &'a PunctuationSpacingCompressionResult,
    pub attached_punctuation_boundary: &'a AttachedInlinePunctuationBoundaryResult,
    pub role_override_infos: &'a [RoleOverrideInfo],
    pub line_breaker_strategy_name: &'a str,
    pub laid_out_lines: &'a [LineBox],
    pub line_solution: &'a LineSolution,
    pub clusters: &'a [Cluster],
    pub justification_plans: &'a [Option<JustificationPlan>],
    pub auto_space_decisions: &'a [AutoSpaceDecisionInfo],
    pub edge_trim_decisions: &'a [LineEdgeTrimDecisionInfo],
    pub decoration_decisions: &'a [DecorationDecisionInfo],
    pub decoration_segments: &'a [DecorationSegmentInfo],
    pub ruby_decisions: &'a [RubyDecisionInfo],
    pub bopomofo_decisions: &'a [BopomofoDecisionInfo],
    pub mandatory_break_decisions: &'a [MandatoryBreakDecisionInfo],
    pub max_lines_decision: Option<MaxLinesDecisionInfo>,
    pub line_spacing_decision: Option<LineSpacingDecisionInfo>,
    pub ruby_line_height_decision: Option<RubyLineHeightDecisionInfo>,
    pub inline_object_line_height_decision: Option<InlineObjectLineHeightDecisionInfo>,
    pub kinsoku_decision: KinsokuDecisionInfo,
    pub contextual_kinsoku_decisions: &'a [ContextualKinsokuDecisionInfo],
    pub line_length_grid_decision: LineLengthGridDecisionInfo,
    pub first_line_indent_decision: super::super::core::LayoutModel::FirstLineIndentDecisionInfo,
    pub inline_box_decisions: &'a [InlineBoxDecisionInfo],
    pub inline_object_decisions: &'a [InlineObjectDecisionInfo],
    pub inline_object_punctuation_attachment_decisions:
        &'a [InlineObjectPunctuationAttachmentDecisionInfo],
    pub zero_width_break_decisions: &'a [ZeroWidthBreakDecisionInfo],
    pub break_opportunity_decisions: &'a [BreakOpportunityDecisionInfo],
    pub emergency_tracking_eligibility_decisions: &'a [EmergencyTrackingEligibilityDecisionInfo],
    pub progressive_break_opportunities: &'a HashMap<i32, ProgressiveBreakOpportunity>,
}

/// Materializes the structured decision stream without owning layout policy.
pub fn build_layout_debug_info(stage: LayoutDebugStageInput<'_>) -> LayoutDebugInfo {
    let font_decisions = stage
        .font_decisions
        .iter()
        .map(|decision| {
            let cluster_text = Text::from(stage.text.slice(decision.range));
            let substitution = stage
                .punctuation_glyph_substitutor
                .substitute(&cluster_text);
            let rollback_cause = stage
                .substitution_rollbacks
                .iter()
                .find(|(range, _)| is_inside(**range, decision.range))
                .map(|(_, cause)| cause);
            FontDecisionInfo {
                range: decision.range,
                source_text: cluster_text.clone(),
                display_text: if rollback_cause.is_some() {
                    cluster_text
                } else {
                    substitution.display_text
                },
                role: format!("{:?}", decision.role),
                font_key: decision.candidate.key.clone(),
                reason: decision.reason.clone(),
                substitution_reason: if let Some(cause) = rollback_cause {
                    format!("{}:{cause}", substitution.reason)
                } else {
                    substitution.reason
                },
            }
        })
        .collect();
    let metric_decisions = stage
        .metric_decisions
        .iter()
        .map(|decision| MetricDecisionInfo {
            range: decision.range,
            source_text: decision.source_text.clone(),
            role: format!("{:?}", decision.request.role),
            font_key: decision.request.font_key.clone(),
            raw_ascent: decision.raw_metrics.ascent,
            raw_descent: decision.raw_metrics.descent,
            raw_leading: decision.raw_metrics.leading,
            raw_source: format!("{:?}", decision.raw_metrics.source),
            layout_ascent: decision.layout_metrics.ascent,
            layout_descent: decision.layout_metrics.descent,
            baseline_class: format!("{:?}", decision.layout_metrics.baseline_class),
            metric_box: format!("{:?}", decision.layout_metrics.metric_box),
            layout_source: format!("{:?}", decision.layout_metrics.source),
            reason: decision.layout_metrics.reason.clone(),
        })
        .collect();
    let punctuation_decisions = stage
        .punctuation_atoms
        .iter()
        .map(|atom| {
            PunctuationDecisionInfo::builder(
                atom.range,
                atom.character,
                format!("{:?}", atom.punctuation_class),
                atom.advance,
                atom.body_width,
                atom.leading_glue.natural,
                atom.trailing_glue.natural,
                format!("{:?}", atom.anchor),
            )
            .ink_bounds(atom.ink_bounds)
            .geometry_source(atom.geometry_source.clone())
            .policy_body_floor(atom.policy_body_floor)
            .ink_width(atom.ink_width)
            .ink_center(atom.ink_center)
            .ink_containment_body_floor(atom.ink_containment_body_floor)
            .ink_containment_applied(atom.ink_containment_applied)
            .ink_bounds_fallback(atom.ink_bounds_fallback.clone())
            .halt_advance(atom.halt_advance)
            .halt_validation(atom.halt_validation.clone())
            .advance_expansion(atom.advance_expansion)
            .glyph_inline_shift(atom.glyph_inline_shift)
            .glyph_placement_reason(atom.glyph_placement_reason.clone())
            .leading_glue_initially_consumed(atom.leading_glue_initially_consumed)
            .trailing_glue_initially_consumed(atom.trailing_glue_initially_consumed)
            .build()
        })
        .collect();
    let spacing_decisions: Vec<_> = stage
        .spacing_plan
        .adjustments
        .iter()
        .map(|adjustment| SpacingDecisionInfo {
            range: adjustment.range,
            left_char: adjustment.left_char,
            right_char: adjustment.right_char,
            natural_inner_glue: adjustment.natural_inner_glue,
            adjusted_inner_glue: adjustment.adjusted_inner_glue,
            reduction: adjustment.reduction,
            reduction_target_range: adjustment.reduction_target_range,
            reason: adjustment.reason.clone(),
        })
        .chain(
            stage
                .attached_punctuation_boundary
                .decisions
                .iter()
                .cloned(),
        )
        .collect();
    let line_decisions = stage
        .laid_out_lines
        .iter()
        .zip(&stage.line_solution.lines)
        .enumerate()
        .map(|(line_index, (line, candidate))| {
            let mut notes = vec![
                format!("index:{line_index}"),
                format!("end:{:?}", line.end_reason),
                format!("natural:{}", line.natural_width),
                format!("adjusted:{}", line.adjusted_width),
                format!("visual:{}", line.visual_width),
            ];
            if let Some(opportunity) = stage
                .progressive_break_opportunities
                .get(&(candidate.cluster_range.last() + 1))
            {
                notes.push(format!("technical-break:{:?}", opportunity.tier));
            }
            if let Some(repair) = candidate.repair.as_ref() {
                notes.push(format!("repair-reason:{}", repair.reason()));
            }
            if let Some(fallback) = stage
                .justification_plans
                .get(line_index)
                .and_then(Option::as_ref)
                .and_then(|plan| plan.fallback_reason.as_ref())
            {
                notes.push(format!("justify-fallback:{fallback}"));
            }
            super::super::core::LayoutModel::LineDecisionInfo::builder(
                line.range,
                stage.line_breaker_strategy_name.to_owned(),
            )
            .repair(candidate.repair.as_ref().map(repair_name))
            .repair_penalty(candidate.repair.as_ref().map_or(0, RepairOption::penalty))
            .repair_decision(
                candidate
                    .repair
                    .as_ref()
                    .map(|repair| repair_to_decision_info(repair, stage.clusters)),
            )
            .repair_candidates(
                candidate
                    .repair_candidates
                    .iter()
                    .map(|candidate| repair_candidate_to_decision_info(candidate, stage.clusters))
                    .collect(),
            )
            .notes(notes)
            .build()
        })
        .collect();
    let justification_decisions = stage
        .justification_plans
        .iter()
        .zip(&stage.line_solution.lines)
        .filter_map(|(plan, candidate)| {
            plan.as_ref()
                .filter(|plan| !plan.allocations.is_empty() || plan.deficit_before > 0.0)
                .map(|plan| JustificationDecisionInfo {
                    line_range: candidate.source_range,
                    deficit_before: plan.deficit_before,
                    deficit_after: plan.unfilled_deficit,
                    allocations: plan
                        .allocations
                        .iter()
                        .map(|allocation| JustificationAllocationInfo {
                            cluster_range: stage.clusters[allocation.target_cluster_index as usize]
                                .range,
                            kind: format!("{:?}", allocation.kind),
                            priority: allocation.priority,
                            delta: allocation.delta,
                            reason: allocation.reason.clone(),
                        })
                        .collect(),
                })
        })
        .collect();
    LayoutDebugInfo::builder()
        .font_decisions(font_decisions)
        .shaping_decisions(stage.shaping_decisions.to_vec())
        .metric_decisions(metric_decisions)
        .punctuation_decisions(punctuation_decisions)
        .geometry_decisions(stage.geometry_decisions.to_vec())
        .spacing_decisions(spacing_decisions)
        .role_overrides(stage.role_override_infos.to_vec())
        .line_decisions(line_decisions)
        .justification_decisions(justification_decisions)
        .auto_space_decisions(stage.auto_space_decisions.to_vec())
        .line_edge_trim_decisions(stage.edge_trim_decisions.to_vec())
        .decoration_decisions(stage.decoration_decisions.to_vec())
        .decoration_segments(stage.decoration_segments.to_vec())
        .ruby_decisions(stage.ruby_decisions.to_vec())
        .bopomofo_decisions(stage.bopomofo_decisions.to_vec())
        .mandatory_break_decisions(stage.mandatory_break_decisions.to_vec())
        .max_lines_decision(stage.max_lines_decision)
        .line_spacing_decision(stage.line_spacing_decision)
        .ruby_line_height_decision(stage.ruby_line_height_decision)
        .inline_object_line_height_decision(stage.inline_object_line_height_decision)
        .kinsoku_decision(Some(stage.kinsoku_decision))
        .contextual_kinsoku_decisions(stage.contextual_kinsoku_decisions.to_vec())
        .line_length_grid_decision(Some(stage.line_length_grid_decision))
        .first_line_indent_decision(Some(stage.first_line_indent_decision))
        .inline_box_decisions(stage.inline_box_decisions.to_vec())
        .inline_object_decisions(stage.inline_object_decisions.to_vec())
        .inline_object_punctuation_attachment_decisions(
            stage
                .inline_object_punctuation_attachment_decisions
                .to_vec(),
        )
        .zero_width_break_decisions(stage.zero_width_break_decisions.to_vec())
        .break_opportunity_decisions(stage.break_opportunity_decisions.to_vec())
        .emergency_tracking_eligibility_decisions(
            stage.emergency_tracking_eligibility_decisions.to_vec(),
        )
        .build()
}

pub fn quote_role_decisions_to_role_override_infos(
    decisions: &[QuoteRoleDecision],
    text: &Text,
    base_classifier: &dyn FontRoleClassifier,
    context: &FontRoleContext,
) -> Vec<RoleOverrideInfo> {
    let mut decisions = decisions.to_vec();
    decisions.sort_by_key(|decision| decision.index);
    decisions
        .into_iter()
        .map(|decision| {
            let range = TextRange::new(decision.index, decision.index + 1);
            let source_text = Text::from(text.slice(range));
            let original_role = base_classifier.classify(text, range, context);
            RoleOverrideInfo {
                range,
                source_text,
                original_role: format!("{:?}", original_role),
                overridden_role: format!("{:?}", decision.role),
                source: decision.source,
                reason: decision.reason,
            }
        })
        .collect()
}

fn repair_candidate_to_decision_info(
    candidate: &RepairCandidate,
    clusters: &[Cluster],
) -> LineRepairCandidateInfo {
    LineRepairCandidateInfo::builder(
        candidate.kind.clone(),
        candidate.reason_code.clone(),
        clusters[candidate.offender_cluster_index as usize].range,
        candidate.penalty,
        candidate.accepted,
    )
    .rejection_reason(candidate.rejection_reason.clone())
    .target_cluster_index(candidate.target_cluster_index)
    .carried_cluster_index(candidate.carried_cluster_index)
    .shrink(candidate.shrink)
    .required_shrink(candidate.required_shrink)
    .available_capacity(candidate.available_capacity)
    .build()
}

fn repair_to_decision_info(repair: &RepairOption, clusters: &[Cluster]) -> LineRepairDecisionInfo {
    match repair {
        RepairOption::PushIn {
            penalty,
            reason,
            offender_cluster_index,
            allocations,
            total_shrink,
            total_available_capacity,
        } => LineRepairDecisionInfo::builder(
            "PushIn".to_owned(),
            reason.split(':').next().unwrap_or(reason).to_owned(),
            clusters[*offender_cluster_index as usize].range,
            *penalty,
        )
        .target_cluster_index(Some(*offender_cluster_index))
        .shrink(*total_shrink)
        .available_capacity(*total_available_capacity)
        .push_in_allocations(
            allocations
                .iter()
                .map(|allocation| LineRepairAllocationInfo {
                    cluster_range: clusters[allocation.cluster_index as usize].range,
                    shrink: allocation.shrink,
                    available_capacity: allocation.available_capacity,
                })
                .collect(),
        )
        .build(),
        RepairOption::CarryPrevious {
            penalty,
            offender_cluster_index,
            carried_cluster_index,
            ..
        } => LineRepairDecisionInfo::builder(
            "CarryPrevious".to_owned(),
            "ForbiddenAtLineStart".to_owned(),
            clusters[*offender_cluster_index as usize].range,
            *penalty,
        )
        .carried_cluster_index(Some(*carried_cluster_index))
        .build(),
        RepairOption::LeaveRagged {
            penalty,
            offender_cluster_index,
            ..
        } => LineRepairDecisionInfo::builder(
            "LeaveRagged".to_owned(),
            "ForbiddenAtLineStart".to_owned(),
            clusters[*offender_cluster_index as usize].range,
            *penalty,
        )
        .build(),
        RepairOption::Hang {
            penalty,
            offender_cluster_index,
            ..
        } => LineRepairDecisionInfo::builder(
            "Hang".to_owned(),
            "ForbiddenAtLineStart".to_owned(),
            clusters[*offender_cluster_index as usize].range,
            *penalty,
        )
        .build(),
        RepairOption::CarryNext {
            penalty,
            moved_cluster_index,
            ..
        } => LineRepairDecisionInfo::builder(
            "CarryNext".to_owned(),
            "ForbiddenAtLineEnd".to_owned(),
            clusters[*moved_cluster_index as usize].range,
            *penalty,
        )
        .carried_cluster_index(Some(*moved_cluster_index))
        .build(),
    }
}

fn repair_name(repair: &RepairOption) -> String {
    match repair {
        RepairOption::PushIn { .. } => "PushIn",
        RepairOption::Hang { .. } => "Hang",
        RepairOption::CarryPrevious { .. } => "CarryPrevious",
        RepairOption::CarryNext { .. } => "CarryNext",
        RepairOption::LeaveRagged { .. } => "LeaveRagged",
    }
    .to_owned()
}

fn is_inside(inner: TextRange, outer: TextRange) -> bool {
    inner.start() >= outer.start() && inner.end() <= outer.end()
}
