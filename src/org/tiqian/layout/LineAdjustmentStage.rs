// 对应 Kotlin 源文件：engine/src/commonMain/kotlin/org/tiqian/layout/LineAdjustmentStage.kt

use std::collections::{HashMap, HashSet};

use super::super::clreq::ClreqProfile::{LineEndPunctuationStyle, PunctuationClass};
use super::super::core::Geometry::{Size, TextRange};
use super::super::core::LayoutModel::{
    Cluster, ContextualKinsokuDecisionInfo, Glyph, GlyphRun, LayoutResult,
    LineEdgeTrimDecisionInfo, LineEndReason,
};
use super::super::font::FontMetrics::BaselineClass;
use super::super::font::FontPolicy::FallbackResolver;
use super::super::shaping::TextShaper::TextShaper;
use super::AnnotationGeometryStage::{AnnotationGeometryRequest, resolve_annotation_geometry};
use super::Justifier::{JustificationPlan, JustificationRequest, Justifier};
use super::LayoutDebugAssembly::{LayoutDebugStageInput, build_layout_debug_info};
use super::LineBreakPlanningStage::{LineBreakPlanningStageResult, ParagraphLayoutPrep};
use super::LineGeometryStage::{
    build_line_boxes, renderable_glyph_run_clusters, resolve_line_vertical_geometry,
};
use super::LineOptimization::RepairOption;
use super::ParagraphShapingStage::map_to_cluster_range;
use super::ProgressiveBreakDecisions::{ProgressiveBreakTier, ShrinkChannel};

/// Kotlin `finishParagraphLayout` 的 Rust stage outcome。
///
/// FIXME(strict-mirror): Kotlin 通过 `ExplainableStubParagraphLayoutEngine` extension receiver
/// 直接调用主入口的 `layoutWithRejectedTechnicalTiers`。Rust 将 stage 划为显式 request/outcome；
/// `Retry` 的重放控制流必须且仅能由 `ParagraphLayoutEngine.rs` 执行，不能迁入本文件。
#[derive(Clone, Debug, PartialEq)]
pub enum LineAdjustmentStageOutcome {
    Finished(LayoutResult),
    Retry {
        rejected_technical_tiers_by_span: HashMap<TextRange, HashSet<ProgressiveBreakTier>>,
    },
}

pub struct LineAdjustmentRequest<'a> {
    pub prep: &'a ParagraphLayoutPrep,
    pub plan: &'a LineBreakPlanningStageResult,
    pub justifier: &'a Justifier,
    pub line_breaker_strategy_name: &'a str,
    pub fallback_resolver: &'a dyn FallbackResolver,
    pub text_shaper: &'a dyn TextShaper,
}

pub fn finish_paragraph_layout(request: LineAdjustmentRequest<'_>) -> LineAdjustmentStageOutcome {
    let prep = request.prep;
    let plan = request.plan;
    let line_solution = &plan.line_solution;
    let applied_hanging_clusters: HashSet<_> = line_solution
        .lines
        .iter()
        .flat_map(|line| line.hanging_cluster_indices.iter().copied())
        .collect();
    let impossible_measure_contextual_hang_clusters: HashSet<_> = plan
        .ascii_point_mark_kinsoku
        .impossible_measure_hang_eligible_clusters
        .iter()
        .chain(
            &plan
                .inline_object_kinsoku
                .impossible_measure_hang_eligible_clusters,
        )
        .copied()
        .collect();
    let contextual_kinsoku_decisions = plan
        .ascii_point_mark_kinsoku
        .decisions
        .iter()
        .chain(&plan.inline_object_kinsoku.decisions)
        .chain(&plan.unicode_punctuation_boundaries.decisions)
        .fold(Vec::new(), |mut decisions, decision| {
            if !decisions
                .iter()
                .any(|existing: &ContextualKinsokuDecisionInfo| {
                    existing.range == decision.range
                        && existing.forbidden_position == decision.forbidden_position
                })
            {
                let fallback = if impossible_measure_contextual_hang_clusters
                    .contains(&decision.cluster_index)
                    && applied_hanging_clusters.contains(&decision.cluster_index)
                {
                    Some(
                        if decision.reason == "AttachedAsciiPointMarkKinsoku" {
                            "AttachedAsciiPointMarkImpossibleMeasureHang"
                        } else {
                            "InlineObjectAttachedMarkImpossibleMeasureHang"
                        }
                        .to_owned(),
                    )
                } else {
                    None
                };
                decisions.push(
                    ContextualKinsokuDecisionInfo::with_impossible_measure_fallback(
                        decision.range,
                        decision.source_text.clone(),
                        decision.cluster_index,
                        decision.forbidden_position.clone(),
                        decision.reason.clone(),
                        fallback,
                    ),
                );
            }
            decisions
        });

    let (mut push_in_trailing, mut push_in_leading, mut push_in_raw_trims) =
        (HashMap::new(), HashMap::new(), HashMap::new());
    for allocation in line_solution
        .lines
        .iter()
        .filter_map(|line| match &line.repair {
            Some(RepairOption::PushIn { allocations, .. }) => Some(allocations),
            _ => None,
        })
        .flatten()
    {
        match allocation.channel {
            ShrinkChannel::TrailingGlue => add_amount(
                &mut push_in_trailing,
                allocation.cluster_index,
                allocation.shrink,
            ),
            ShrinkChannel::LeadingGlue => add_amount(
                &mut push_in_leading,
                allocation.cluster_index,
                allocation.shrink,
            ),
            ShrinkChannel::LeadingAndTrailingGlue => {
                add_amount(
                    &mut push_in_leading,
                    allocation.cluster_index,
                    allocation.shrink / 2.0,
                );
                add_amount(
                    &mut push_in_trailing,
                    allocation.cluster_index,
                    allocation.shrink / 2.0,
                );
            }
            ShrinkChannel::RawAdvance => add_amount(
                &mut push_in_raw_trims,
                allocation.cluster_index,
                allocation.shrink,
            ),
        }
    }
    let line_hyphen_advance_at = |line_index: usize| -> f32 {
        if prep.hyphen_offsets.is_empty()
            || line_index >= line_solution.lines.len().saturating_sub(1)
        {
            return 0.0;
        }
        let next = &line_solution.lines[line_index + 1];
        if next.cluster_range.is_empty() {
            return 0.0;
        }
        if prep.hyphen_offsets.contains(
            &prep.natural_clusters[next.cluster_range.first() as usize]
                .range
                .start(),
        ) {
            prep.hyphen_advance
        } else {
            0.0
        }
    };
    for (line_index, line) in line_solution.lines.iter().enumerate() {
        if line.cluster_range.is_empty() {
            continue;
        }
        let hyphen = line_hyphen_advance_at(line_index);
        if hyphen <= 0.0 {
            continue;
        }
        let line_limit = if line.cluster_range.first() == 0 {
            prep.measure - plan.first_line_indent
        } else {
            prep.measure - plan.block_indent
        };
        let content: f32 = line
            .cluster_range
            .into_iter()
            .map(|index| prep.clusters[index as usize].advance)
            .sum();
        let mut shortfall = content + hyphen - line_limit;
        if shortfall <= TECHNICAL_STRETCH_EPSILON_PX {
            continue;
        }
        let mut opportunities: Vec<_> = prep
            .shrink_opportunities
            .iter()
            .filter(|opportunity| {
                line.cluster_range.contains(opportunity.cluster_index) && !opportunity.line_end_only
            })
            .collect();
        opportunities.sort_by_key(|opportunity| opportunity.tier);
        for opportunity in opportunities {
            if shortfall <= TECHNICAL_STRETCH_EPSILON_PX {
                break;
            }
            let used = match opportunity.channel {
                ShrinkChannel::TrailingGlue => push_in_trailing
                    .get(&opportunity.cluster_index)
                    .copied()
                    .unwrap_or(0.0),
                ShrinkChannel::LeadingGlue => push_in_leading
                    .get(&opportunity.cluster_index)
                    .copied()
                    .unwrap_or(0.0),
                ShrinkChannel::LeadingAndTrailingGlue => {
                    push_in_trailing
                        .get(&opportunity.cluster_index)
                        .copied()
                        .unwrap_or(0.0)
                        + push_in_leading
                            .get(&opportunity.cluster_index)
                            .copied()
                            .unwrap_or(0.0)
                }
                ShrinkChannel::RawAdvance => push_in_raw_trims
                    .get(&opportunity.cluster_index)
                    .copied()
                    .unwrap_or(0.0),
            };
            let taken = shortfall.min((opportunity.capacity - used).max(0.0));
            if taken <= 0.0 {
                continue;
            }
            match opportunity.channel {
                ShrinkChannel::TrailingGlue => {
                    add_amount(&mut push_in_trailing, opportunity.cluster_index, taken)
                }
                ShrinkChannel::LeadingGlue => {
                    add_amount(&mut push_in_leading, opportunity.cluster_index, taken)
                }
                ShrinkChannel::LeadingAndTrailingGlue => {
                    add_amount(&mut push_in_leading, opportunity.cluster_index, taken / 2.0);
                    add_amount(
                        &mut push_in_trailing,
                        opportunity.cluster_index,
                        taken / 2.0,
                    );
                }
                ShrinkChannel::RawAdvance => {
                    add_amount(&mut push_in_raw_trims, opportunity.cluster_index, taken)
                }
            }
            shortfall -= taken;
        }
    }

    let push_in_geometry = prep
        .base_geometry
        .consume_trailing_by_cluster(&push_in_trailing)
        .consume_leading_by_cluster(&push_in_leading);
    let edge_trim_result = push_in_geometry.consume_line_edge_glue(
        &line_solution.lines,
        prep.adjustment_style.line_end_punctuation == LineEndPunctuationStyle::ForceHalfWidth,
    );
    let (auto_space_edge_trims, auto_space_edge_decisions) =
        resolve_auto_space_edge_trims(prep, plan, &push_in_raw_trims);
    let mut raw_trims = auto_space_edge_trims;
    for (index, amount) in push_in_raw_trims {
        add_amount(&mut raw_trims, index, amount);
    }
    let trimmed_geometry = edge_trim_result.geometry.with_raw_edge_trims(&raw_trims);
    let trimmed_clusters = trimmed_geometry.resolve_clusters();
    let mut edge_trim_decisions = edge_trim_result.decisions;
    edge_trim_decisions.extend(auto_space_edge_decisions);

    let justification_plans: Vec<_> = line_solution
        .lines
        .iter()
        .enumerate()
        .map(|(line_index, line)| {
            if line_index == line_solution.lines.len() - 1
                || line.cluster_range.is_empty()
                || line.end_reason != LineEndReason::AutoWrap
            {
                return None;
            }
            let selected_technical_break = plan
                .progressive_break_opportunities
                .get(&(line.cluster_range.last() + 1));
            let preferred_tracking_span = selected_technical_break
                .filter(|break_opportunity| {
                    break_opportunity.tier == ProgressiveBreakTier::Emergency
                })
                .map(|break_opportunity| break_opportunity.span_range);
            let preferred_emergency_tracking_boundaries =
                preferred_tracking_span.map_or_else(HashMap::new, |span| {
                    plan.emergency_tracking_boundary_after_clusters
                        .iter()
                        .filter_map(|(left, reason)| {
                            let right = left + 1;
                            (prep.natural_clusters[*left as usize].range.start() >= span.start()
                                && prep.natural_clusters[right as usize].range.end() <= span.end())
                            .then(|| (*left, reason.clone()))
                        })
                        .collect()
                });
            let mut justification = JustificationRequest::new(
                &trimmed_clusters,
                &prep.cluster_roles,
                &prep.east_asian_spacing_edges,
                line.in_measure_cluster_range(),
                (if line.cluster_range.first() == 0 {
                    prep.measure - plan.first_line_indent
                } else {
                    prep.measure - plan.block_indent
                }) - line_hyphen_advance_at(line_index),
                prep.font_size,
                prep.clreq_profile.auto_space.gap_em,
                prep.clreq_profile.auto_space.stretch_max_em,
            );
            justification.allow_sino_western_gap_stretch =
                prep.adjustment_style.allow_sino_western_gap_adjustment;
            justification.no_stretch_boundary_clusters = plan.no_stretch_boundary_clusters.clone();
            justification.no_stretch_boundary_after_clusters =
                plan.no_stretch_boundary_after_clusters.clone();
            justification.western_bracket_cjk_inter_char_boundary_after_clusters = plan
                .western_bracket_cjk_inter_char_boundary_after_clusters
                .clone();
            justification.attached_inline_physical_boundary_after_clusters = plan
                .attached_inline_physical_boundary_after_clusters
                .clone();
            justification.attached_inline_virtual_boundary_after_clusters =
                plan.attached_inline_virtual_boundary_after_clusters.clone();
            justification.attached_inline_virtual_sino_western_boundary_after_clusters = plan
                .attached_inline_virtual_sino_western_boundary_after_clusters
                .clone();
            justification.uniform_inline_object_boundary_after_clusters =
                prep.uniform_inline_object_boundary_after_clusters.clone();
            justification.preferred_inline_object_boundary_after_clusters =
                prep.preferred_inline_object_boundary_after_clusters.clone();
            justification.technical_boundary_after_clusters =
                plan.technical_boundary_after_clusters.clone();
            justification.emergency_tracking_boundary_after_clusters =
                plan.emergency_tracking_boundary_after_clusters.clone();
            justification.preferred_emergency_tracking_boundary_after_clusters =
                preferred_emergency_tracking_boundaries;
            Some(request.justifier.justify(justification))
        })
        .collect();

    let mut newly_rejected_technical_tiers: HashMap<TextRange, HashSet<ProgressiveBreakTier>> =
        HashMap::new();
    for (line_index, line) in line_solution.lines.iter().enumerate() {
        if line.end_reason != LineEndReason::AutoWrap || line.cluster_range.is_empty() {
            continue;
        }
        let Some(selected_break) = plan
            .progressive_break_opportunities
            .get(&(line.cluster_range.last() + 1))
            .filter(|break_opportunity| break_opportunity.tier != ProgressiveBreakTier::Emergency)
        else {
            continue;
        };
        if prep
            .rejected_technical_tiers_by_span
            .get(&selected_break.span_range)
            .is_some_and(|tiers| tiers.contains(&selected_break.tier))
        {
            continue;
        }
        let Some(current_line_plan) = justification_plans[line_index].as_ref() else {
            continue;
        };
        let body_tracking = current_line_plan.allocations.iter().any(|allocation| {
            matches!(
                allocation.kind,
                super::PunctuationModel::GlueKind::CjkInterChar
                    | super::PunctuationModel::GlueKind::EmergencyGraphemeTracking
            ) && allocation.delta
                > CURRENT_LINE_TECHNICAL_BODY_STRETCH_LIMIT_EM * prep.font_size
                    + TECHNICAL_STRETCH_EPSILON_PX
        });
        if body_tracking {
            newly_rejected_technical_tiers
                .entry(selected_break.span_range)
                .or_default()
                .insert(selected_break.tier);
        }
    }
    if !newly_rejected_technical_tiers.is_empty() {
        let mut rejected = prep.rejected_technical_tiers_by_span.clone();
        for (span, tiers) in newly_rejected_technical_tiers {
            rejected.entry(span).or_default().extend(tiers);
        }
        return LineAdjustmentStageOutcome::Retry {
            rejected_technical_tiers_by_span: rejected,
        };
    }

    let mut justify_delta_by_cluster = HashMap::new();
    for allocation in justification_plans
        .iter()
        .flatten()
        .flat_map(|plan| &plan.allocations)
    {
        add_amount(
            &mut justify_delta_by_cluster,
            allocation.target_cluster_index,
            allocation.delta,
        );
    }
    let final_geometry = trimmed_geometry.add_justification_deltas(&justify_delta_by_cluster);
    let final_clusters: Vec<_> = final_geometry
        .resolve_clusters()
        .into_iter()
        .map(|mut cluster| {
            if let Some(metric) = plan.metric_decision_by_range.get(&cluster.range) {
                let metric_shift = if metric.layout_metrics.baseline_class == BaselineClass::Roman {
                    0.0
                } else {
                    plan.base_box_descent - metric.layout_metrics.descent
                };
                let shift = cluster.baseline_shift
                    + metric_shift
                    + (prep.style_at)(cluster.range.start()).baseline_shift;
                if shift <= -0.01 || shift >= 0.01 {
                    cluster.baseline_shift = shift;
                }
            }
            cluster
        })
        .collect();
    let geometry_decisions = final_geometry.to_decision_info();
    let glyph_runs = build_glyph_runs(prep, &final_clusters);
    let vertical_geometry = resolve_line_vertical_geometry(
        &prep.input,
        prep.font_size,
        &prep.pinyin_spans,
        &prep.natural_clusters,
        line_solution,
        &prep.ruby_font_geometry_by_span,
        plan.existing_interline_space,
        plan.base_line_metrics,
        plan.base_face_height,
        plan.ruby_extent,
        &prep.inline_object_by_cluster_index,
        plan.base_ascent,
        plan.base_descent,
    );
    let line_boxes = build_line_boxes(
        &prep.input,
        line_solution,
        &trimmed_clusters,
        &final_clusters,
        plan.first_line_indent,
        plan.block_indent,
        prep.measure,
        prep.grid_body_offset,
        &vertical_geometry.line_baseline,
        &vertical_geometry.line_top,
        &vertical_geometry.line_bottom,
        &|index| line_hyphen_advance_at(index as usize),
        &prep.hyphen_glyphs,
        &justification_plans,
    );
    let annotation = resolve_annotation_geometry(AnnotationGeometryRequest {
        input: &prep.input,
        font_size: prep.font_size,
        inline_object_by_cluster_index: &prep.inline_object_by_cluster_index,
        line_solution,
        clreq_profile: &prep.clreq_profile,
        geometry_decisions: &geometry_decisions,
        auto_space_decisions: &prep.auto_space_decisions,
        visible_line_ranges: &line_boxes.visible_line_ranges,
        lines: &line_boxes.visible_lines,
        final_clusters: &final_clusters,
        cluster_roles: &prep.cluster_roles,
        justify_delta_by_cluster: &justify_delta_by_cluster,
        ruby_and_bopomofo_spread: &prep.ruby_and_bopomofo_spread,
        metric_decisions: &plan.metric_decisions,
        pinyin_spans: &prep.pinyin_spans,
        natural_clusters: &prep.natural_clusters,
        ruby_font_geometry_by_span: &prep.ruby_font_geometry_by_span,
        ruby_stack_gap: prep.ruby_stack_gap,
        base_ascent: plan.base_ascent,
        ruby_font_size: prep.ruby_font_size,
        ruby_font_weight: prep.ruby_font_weight,
        base_descent: plan.base_descent,
        bopomofo_font_weight_at: prep.bopomofo_font_weight_at.as_ref(),
        fallback_resolver: request.fallback_resolver,
        text_shaper: request.text_shaper,
    });
    let lines = line_boxes.visible_lines;
    let widest_line = lines
        .iter()
        .map(|line| line.indent + line.visual_width + line.hyphen_advance)
        .fold(0.0, f32::max);
    let total_height = lines.last().map(|line| line.bottom).unwrap_or_else(|| {
        if prep.text.is_empty() {
            0.0
        } else {
            plan.base_line_metrics.height
        }
    });
    let debug = build_layout_debug_info(LayoutDebugStageInput {
        text: &prep.text,
        font_decisions: &prep.font_decisions,
        punctuation_glyph_substitutor: &prep.punctuation_glyph_substitutor,
        substitution_rollbacks: &prep.substitution_rollbacks,
        shaping_decisions: &prep.shaping_decisions,
        metric_decisions: &plan.metric_decisions,
        punctuation_atoms: &prep.punctuation_atoms,
        geometry_decisions: &geometry_decisions,
        spacing_plan: &prep.spacing_plan,
        attached_punctuation_boundary: &prep.attached_punctuation_boundary,
        role_override_infos: &prep.role_override_infos,
        line_breaker_strategy_name: request.line_breaker_strategy_name,
        laid_out_lines: &line_boxes.laid_out_lines,
        line_solution,
        clusters: &prep.clusters,
        justification_plans: &justification_plans,
        auto_space_decisions: &prep.auto_space_decisions,
        edge_trim_decisions: &edge_trim_decisions,
        decoration_decisions: &annotation.decoration_decisions,
        decoration_segments: &annotation.decoration_segments,
        ruby_decisions: &annotation.ruby_decisions,
        bopomofo_decisions: &annotation.bopomofo_decisions,
        mandatory_break_decisions: &prep.mandatory_break_decisions,
        max_lines_decision: line_boxes.max_lines_decision,
        line_spacing_decision: plan.line_spacing_decision.clone(),
        ruby_line_height_decision: vertical_geometry.ruby_line_height_decision,
        inline_object_line_height_decision: vertical_geometry.inline_object_line_height_decision,
        kinsoku_decision: plan.kinsoku_decision.clone(),
        contextual_kinsoku_decisions: &contextual_kinsoku_decisions,
        line_length_grid_decision: prep.line_length_grid_decision.clone(),
        first_line_indent_decision: plan.first_line_indent_decision.clone(),
        inline_box_decisions: &prep.inline_box_result.decisions,
        inline_object_decisions: &annotation.inline_object_decisions,
        inline_object_punctuation_attachment_decisions: &prep
            .inline_object_punctuation_attachment_decisions,
        zero_width_break_decisions: &prep.zero_width_break_decisions,
        break_opportunity_decisions: &prep.break_opportunity_decisions,
        emergency_tracking_eligibility_decisions: &prep.emergency_tracking_eligibility_decisions,
        progressive_break_opportunities: &plan.progressive_break_opportunities,
    });
    LineAdjustmentStageOutcome::Finished(LayoutResult::with_debug(
        prep.input.clone(),
        Size {
            width: widest_line.min(prep.input.constraints.max_width()),
            height: total_height,
        },
        final_clusters,
        glyph_runs,
        lines,
        debug,
    ))
}

fn resolve_auto_space_edge_trims(
    prep: &ParagraphLayoutPrep,
    plan: &LineBreakPlanningStageResult,
    push_in_raw_trims: &HashMap<i32, f32>,
) -> (HashMap<i32, f32>, Vec<LineEdgeTrimDecisionInfo>) {
    let auto_space_gap = prep.clreq_profile.auto_space.gap_em * prep.font_size;
    let mut trims = HashMap::new();
    let mut decisions = Vec::new();
    for line in &plan.line_solution.lines {
        if line.cluster_range.is_empty() {
            continue;
        }
        for (cluster_index, side) in [
            (line.cluster_range.last(), "trailing"),
            (line.cluster_range.first(), "leading"),
        ] {
            if let Some(decision) = prep.auto_space_decisions.iter().find(|decision| {
                decision.cluster_range == prep.natural_clusters[cluster_index as usize].range
                    && decision.side == side
            }) {
                add_amount(&mut trims, cluster_index, auto_space_gap);
                decisions.push(LineEdgeTrimDecisionInfo {
                    line_range: line.source_range,
                    cluster_range: decision.cluster_range,
                    side: side.to_owned(),
                    trim_amount: auto_space_gap,
                    consumed_before: 0.0,
                    natural_glue: auto_space_gap,
                    reason: "TextAutoSpaceLineEdgeTrim".to_owned(),
                });
            }
            let cluster = &prep.natural_clusters[cluster_index as usize];
            if is_space_run(cluster)
                && !prep
                    .inline_object_separator_space_trims
                    .contains_key(&cluster_index)
                && cluster.advance > 0.0
            {
                add_amount(&mut trims, cluster_index, cluster.advance);
                decisions.push(LineEdgeTrimDecisionInfo {
                    line_range: line.source_range,
                    cluster_range: cluster.range,
                    side: side.to_owned(),
                    trim_amount: cluster.advance,
                    consumed_before: 0.0,
                    natural_glue: cluster.advance,
                    reason: "LineEdgeWordSpaceCollapse".to_owned(),
                });
            }
        }
        let trailing_index = line.cluster_range.last();
        if let Some(amount) = prep
            .attached_punctuation_trailing_glue_by_cluster
            .get(&trailing_index)
            .filter(|amount| **amount > 0.0)
        {
            add_amount(&mut trims, trailing_index, *amount);
            decisions.push(LineEdgeTrimDecisionInfo {
                line_range: line.source_range,
                cluster_range: prep.natural_clusters[trailing_index as usize].range,
                side: "trailing".to_owned(),
                trim_amount: *amount,
                consumed_before: 0.0,
                natural_glue: *amount,
                reason: "AttachedInlineVirtualBoundaryLineEndTrim".to_owned(),
            });
        }
        if line.end_reason == LineEndReason::AutoWrap {
            let trailing_index = line.cluster_range.last();
            let discardable = prep
                .inline_object_by_cluster_index
                .get(&trailing_index)
                .map_or(0.0, |object| {
                    object.trailing_boundary.line_end_discardable_advance
                });
            let consumed_before = push_in_raw_trims
                .get(&trailing_index)
                .copied()
                .unwrap_or(0.0)
                .min(discardable);
            let remaining = (discardable - consumed_before).max(0.0);
            if remaining > 0.0 {
                add_amount(&mut trims, trailing_index, remaining);
                decisions.push(LineEdgeTrimDecisionInfo {
                    line_range: line.source_range,
                    cluster_range: prep.natural_clusters[trailing_index as usize].range,
                    side: "trailing".to_owned(),
                    trim_amount: remaining,
                    consumed_before,
                    natural_glue: discardable,
                    reason: "InlineObjectLineEndDiscardableGlue".to_owned(),
                });
            }
        }
    }
    (trims, decisions)
}

fn build_glyph_runs(prep: &ParagraphLayoutPrep, final_clusters: &[Cluster]) -> Vec<GlyphRun> {
    renderable_glyph_run_clusters(final_clusters, &prep.open_type_features_by_cluster_range)
        .into_iter()
        .map(|clusters| {
            let open_type_features = prep
                .open_type_features_by_cluster_range
                .get(&clusters[0].range)
                .cloned()
                .unwrap_or_default();
            let glyphs = clusters
                .iter()
                .enumerate()
                .flat_map(|(fallback_id, cluster)| {
                    prep.shaped_glyphs_by_cluster_range
                        .get(&cluster.range)
                        .map(|glyphs| {
                            center_dash_ink(
                                map_to_cluster_range(glyphs, cluster),
                                cluster,
                                prep.atom_class_by_range.get(&cluster.range),
                            )
                        })
                        .unwrap_or_else(|| {
                            vec![
                                Glyph::builder(fallback_id as u32, cluster.range, cluster.advance)
                                    .build(),
                            ]
                        })
                })
                .collect();
            GlyphRun::with_open_type_features(
                TextRange::new(
                    clusters.first().unwrap().range.start(),
                    clusters.last().unwrap().range.end(),
                ),
                clusters[0].font_key.clone(),
                glyphs,
                clusters.iter().map(|cluster| cluster.advance).sum(),
                open_type_features,
            )
        })
        .collect()
}

fn center_dash_ink(
    glyphs: Vec<Glyph>,
    cluster: &Cluster,
    punctuation_class: Option<&PunctuationClass>,
) -> Vec<Glyph> {
    if punctuation_class != Some(&PunctuationClass::Dash) || glyphs.len() != 1 {
        return glyphs;
    }
    let mut glyph = glyphs.into_iter().next().unwrap();
    let Some(ink) = glyph.bounds else {
        return vec![glyph];
    };
    let inset = (cluster.advance - (ink.right - ink.left)) / 2.0 - ink.left;
    if inset > 0.5 {
        glyph.x += inset;
    }
    vec![glyph]
}

fn add_amount(target: &mut HashMap<i32, f32>, index: i32, amount: f32) {
    *target.entry(index).or_insert(0.0) += amount;
}
fn is_space_run(cluster: &Cluster) -> bool {
    !cluster.text.is_empty() && cluster.text.chars().all(|character| character == ' ')
}

const CURRENT_LINE_TECHNICAL_BODY_STRETCH_LIMIT_EM: f32 = 0.0;
const TECHNICAL_STRETCH_EPSILON_PX: f32 = 0.001;
