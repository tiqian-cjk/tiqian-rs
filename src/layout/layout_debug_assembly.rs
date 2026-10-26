// 对应 Kotlin 源文件：engine/src/commonMain/kotlin/org/tiqian/layout/LayoutDebugAssembly.kt

use alloc::{borrow::ToOwned, format, string::String, vec::Vec};
use crate::common::HashMap;

use super::super::clreq::clreq_profile::{
    CjkPunctuationGlyphSubstitution, ClreqPunctuationGlyphSubstitutor,
};
use super::super::core::geometry::TextRange;
use super::super::core::layout_model::{
    AutoSpaceDecisionInfo, BopomofoGeometry, BreakOpportunityDecisionInfo, Cluster,
    ClusterGeometryDecisionInfo, ContextualKinsokuDecisionInfo, DecorationDecisionInfo,
    DecorationSegmentGeometry, EmergencyTrackingEligibilityDecisionInfo, FontDecisionInfo,
    InlineBoxDecisionInfo, InlineObjectDecisionInfo, InlineObjectLineHeightDecisionInfo,
    InlineObjectPunctuationAttachmentDecisionInfo, JustificationAllocationInfo,
    JustificationDecisionInfo, KinsokuDecisionInfo, LayoutDebugInfo, LineBox,
    LineEdgeTrimDecisionInfo, LineLengthGridDecisionInfo, LineRepairAllocationInfo,
    LineRepairCandidateInfo, LineRepairDecisionInfo, LineSpacingDecisionInfo,
    MandatoryBreakDecisionInfo, MaxLinesDecisionInfo, MetricDecisionInfo, PunctuationDecisionInfo,
    RoleOverrideInfo, RubyGeometry, RubyLineHeightDecisionInfo, ShapingDecisionInfo,
    SpacingDecisionInfo, ZeroWidthBreakDecisionInfo,
};
use super::super::core::text::Text;
use super::super::font::font_policy::{FontRoleClassifier, FontRoleContext};
use super::super::shaping::font_backend::FontResolution;
use super::justifier::JustificationPlan;
use super::line_geometry_stage::ClusterMetricDecision;
use super::line_optimization::{LineSolution, RepairCandidate, RepairKind, RepairOption};
use super::progressive_break_decisions::ProgressiveBreakOpportunity;
use super::punctuation_geometry_ledger::AttachedInlinePunctuationBoundaryResult;
use super::punctuation_model::{PunctuationAtom, PunctuationSpacingCompressionResult};
use super::quote_pair_analyzer::QuoteRoleDecision;

pub struct LayoutDebugStageInput<'a> {
    pub text: &'a Text,
    pub font_resolutions: &'a HashMap<TextRange, FontResolution>,
    pub substitution_rollbacks: &'a HashMap<TextRange, String>,
    /// shaping 与 debug 共用的标点显示替换器；debug 按整段范围重新解析显示文本。
    pub punctuation_glyph_substitutor: &'a ClreqPunctuationGlyphSubstitutor,
    /// shaping 实际采用的显示替换，按 segment range 记录。
    pub substitutions: &'a HashMap<TextRange, CjkPunctuationGlyphSubstitution>,
    pub shaping_decisions: Vec<ShapingDecisionInfo>,
    pub metric_decisions: &'a [ClusterMetricDecision],
    pub punctuation_atoms: &'a [PunctuationAtom],
    pub geometry_decisions: Vec<ClusterGeometryDecisionInfo>,
    pub spacing_plan: &'a PunctuationSpacingCompressionResult,
    pub attached_punctuation_boundary: &'a AttachedInlinePunctuationBoundaryResult,
    pub role_override_infos: &'a [RoleOverrideInfo],
    pub line_breaker_strategy_name: &'static str,
    pub laid_out_lines: &'a [LineBox],
    pub line_solution: &'a LineSolution,
    pub clusters: &'a [Cluster],
    pub justification_plans: &'a [Option<JustificationPlan>],
    pub auto_space_decisions: &'a [AutoSpaceDecisionInfo],
    pub edge_trim_decisions: Vec<LineEdgeTrimDecisionInfo>,
    pub decoration_decisions: Vec<DecorationDecisionInfo>,
    pub decoration_segments: Vec<DecorationSegmentGeometry>,
    pub ruby_decisions: Vec<RubyGeometry>,
    pub bopomofo_decisions: Vec<BopomofoGeometry>,
    pub mandatory_break_decisions: &'a [MandatoryBreakDecisionInfo],
    pub max_lines_decision: Option<MaxLinesDecisionInfo>,
    pub line_spacing_decision: Option<LineSpacingDecisionInfo>,
    pub ruby_line_height_decision: Option<RubyLineHeightDecisionInfo>,
    pub inline_object_line_height_decision: Option<InlineObjectLineHeightDecisionInfo>,
    pub kinsoku_decision: KinsokuDecisionInfo,
    pub contextual_kinsoku_decisions: Vec<ContextualKinsokuDecisionInfo>,
    pub line_length_grid_decision: LineLengthGridDecisionInfo,
    pub first_line_indent_decision: super::super::core::layout_model::FirstLineIndentDecisionInfo,
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
    let mut resolutions: Vec<_> = stage.font_resolutions.values().collect();
    resolutions.sort_by_key(|resolution| (resolution.range.start(), resolution.range.end()));
    let font_decisions = resolutions
        .into_iter()
        .filter_map(|resolution| {
            let Some(selected_attempt) = resolution.selected_attempt() else {
                log::warn!(
                    "font resolution lacks selected candidate evidence; omitting debug decision"
                );
                return None;
            };
            let cluster_text = stage.text.slice_text(resolution.range);
            // debug 记录按 font resolution 的整段范围给出显示文本与原因。
            // shaping 的 substitutions 以 segment 为 key，粒度更细，不能直接当整段结果用；
            // 因此这里按整段重新走一次 role 门控的显示文本解析，
            // 与测量、绘制使用的是同一条规则。
            let substituted = stage
                .punctuation_glyph_substitutor
                .display_text_for_role_with_candidate(&cluster_text, resolution.role);
            let candidate = stage
                .punctuation_glyph_substitutor
                .policy_display_text(&cluster_text);
            let rollback_cause = stage
                .substitution_rollbacks
                .iter()
                .find(|(range, _)| is_inside(**range, resolution.range))
                .map(|(_, cause)| cause);
            let (display_text, substitution_reason) = match rollback_cause {
                Some(cause) => (
                    cluster_text.clone(),
                    stage
                        .punctuation_glyph_substitutor
                        .reason_for_role(&cluster_text, resolution.role, &candidate)
                        .render(|| code_point_labels(&cluster_text), String::new)
                        + ":"
                        + cause,
                ),
                None => (
                    substituted.clone(),
                    stage
                        .punctuation_glyph_substitutor
                        .reason_for_role(&cluster_text, resolution.role, &candidate)
                        .render(
                            || code_point_labels(&cluster_text),
                            || code_point_labels(&substituted),
                        ),
                ),
            };
            Some(FontDecisionInfo {
                range: resolution.range,
                source_text: cluster_text,
                display_text,
                role: resolution.role,
                candidate_key: selected_attempt.candidate_key.clone(),
                resolved_face: Some(resolution.face.clone()),
                reason: "FontBackendCompleteShapingSelection",
                substitution_reason,
            })
        })
        .collect();
    let metric_decisions = stage
        .metric_decisions
        .iter()
        .map(|decision| MetricDecisionInfo {
            range: decision.range,
            source_text: decision.source_text.clone(),
            role: decision.request.role,
            font_face: decision.request.face.clone(),
            raw_ascent: decision.raw_metrics.ascent,
            raw_descent: decision.raw_metrics.descent,
            raw_leading: decision.raw_metrics.leading,
            raw_source: decision.raw_metrics.source,
            layout_ascent: decision.layout_metrics.ascent,
            layout_descent: decision.layout_metrics.descent,
            baseline_class: decision.layout_metrics.baseline_class,
            metric_box: decision.layout_metrics.metric_box,
            layout_source: decision.layout_metrics.source,
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
                atom.punctuation_class,
                atom.advance,
                atom.body_width,
                atom.leading_glue.natural,
                atom.trailing_glue.natural,
                atom.anchor,
            )
            .ink_bounds(atom.ink_bounds)
            .geometry_source(atom.geometry_source)
            .geometry_forced_half_width(atom.geometry_forced_half_width)
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
            reason: adjustment.reason,
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
            // 行号、end reason 与各项宽度都能从 `LineBox` 直接读到，不再重复写入 notes。
            let technical_break = stage
                .progressive_break_opportunities
                .get(&(candidate.cluster_range.last() + 1))
                .map(|opportunity| opportunity.tier);
            let justify_fallback = stage
                .justification_plans
                .get(line_index)
                .and_then(Option::as_ref)
                .and_then(|plan| plan.fallback_reason.clone());
            super::super::core::layout_model::LineDecisionInfo::builder(
                line.range,
                stage.line_breaker_strategy_name,
            )
            .repair(candidate.repair.as_ref().map(|repair| repair.kind().name().to_owned()))
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
            .technical_break(technical_break)
            .justify_fallback(justify_fallback)
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
                            kind: allocation.kind,
                            priority: allocation.priority,
                            delta: allocation.delta,
                            reason: allocation.reason,
                        })
                        .collect(),
                })
        })
        .collect();
    LayoutDebugInfo::builder()
        .font_decisions(font_decisions)
        .shaping_decisions(stage.shaping_decisions)
        .metric_decisions(metric_decisions)
        .punctuation_decisions(punctuation_decisions)
        .geometry_decisions(stage.geometry_decisions)
        .spacing_decisions(spacing_decisions)
        .role_overrides(stage.role_override_infos.to_vec())
        .line_decisions(line_decisions)
        .justification_decisions(justification_decisions)
        .auto_space_decisions(stage.auto_space_decisions.to_vec())
        .line_edge_trim_decisions(stage.edge_trim_decisions)
        .decoration_decisions(stage.decoration_decisions)
        .decoration_segments(stage.decoration_segments)
        .ruby_decisions(stage.ruby_decisions)
        .bopomofo_decisions(stage.bopomofo_decisions)
        .mandatory_break_decisions(stage.mandatory_break_decisions.to_vec())
        .max_lines_decision(stage.max_lines_decision)
        .line_spacing_decision(stage.line_spacing_decision)
        .ruby_line_height_decision(stage.ruby_line_height_decision)
        .inline_object_line_height_decision(stage.inline_object_line_height_decision)
        .kinsoku_decision(Some(stage.kinsoku_decision))
        .contextual_kinsoku_decisions(stage.contextual_kinsoku_decisions)
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
            let source_text = text.slice_text(range);
            let original_role = base_classifier.classify(text, range, context);
            RoleOverrideInfo {
                range,
                source_text,
                original_role,
                overridden_role: decision.role,
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
        candidate.kind,
        candidate.reason_code,
        clusters[candidate.offender_cluster_index as usize].range,
        candidate.penalty,
        candidate.accepted,
    )
    .rejection_reason(candidate.rejection_reason)
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
            reason_code,
            offender_cluster_index,
            allocations,
            total_shrink,
            total_available_capacity,
            ..
        } => LineRepairDecisionInfo::builder(
            RepairKind::PushIn,
            *reason_code,
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
            reason_code,
            offender_cluster_index,
            carried_cluster_index,
            ..
        } => LineRepairDecisionInfo::builder(
            RepairKind::CarryPrevious,
            *reason_code,
            clusters[*offender_cluster_index as usize].range,
            *penalty,
        )
        .carried_cluster_index(Some(*carried_cluster_index))
        .build(),
        RepairOption::LeaveRagged {
            penalty,
            reason_code,
            offender_cluster_index,
            ..
        } => LineRepairDecisionInfo::builder(
            RepairKind::LeaveRagged,
            *reason_code,
            clusters[*offender_cluster_index as usize].range,
            *penalty,
        )
        .build(),
        RepairOption::Hang {
            penalty,
            reason_code,
            offender_cluster_index,
            ..
        } => LineRepairDecisionInfo::builder(
            RepairKind::Hang,
            *reason_code,
            clusters[*offender_cluster_index as usize].range,
            *penalty,
        )
        .build(),
        RepairOption::CarryNext {
            penalty,
            reason_code,
            moved_cluster_index,
            ..
        } => LineRepairDecisionInfo::builder(
            RepairKind::CarryNext,
            *reason_code,
            clusters[*moved_cluster_index as usize].range,
            *penalty,
        )
        .carried_cluster_index(Some(*moved_cluster_index))
        .build(),
    }
}

fn is_inside(inner: TextRange, outer: TextRange) -> bool {
    inner.start() >= outer.start() && inner.end() <= outer.end()
}

/// 与 `ClreqPunctuationGlyphSubstitutor` 内部使用的码点标签格式一致。
fn code_point_labels(text: &Text) -> String {
    text.chars()
        .map(|character| format!("U+{:04X}", character as u32))
        .collect::<Vec<_>>()
        .join("+")
}
