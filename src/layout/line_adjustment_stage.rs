// 对应 Kotlin 源文件：engine/src/commonMain/kotlin/org/tiqian/layout/LineAdjustmentStage.kt

use crate::common::{HashMap, HashSet};

use super::super::clreq::clreq_profile::{LineEndPunctuationStyle, PunctuationClass};
use super::super::core::geometry::{Size, TextRange};
use super::super::core::layout_model::{
    Cluster, ContextualKinsokuDecisionInfo, Glyph, GlyphRun, ImpossibleMeasureFallback,
    LayoutResult, LineEdgeTrimDecisionInfo, LineEdgeTrimReason, LineEdgeTrimSide, LineEndReason,
};
use super::super::font::font_metrics::BaselineClass;
use super::super::shaping::font_backend::FontBackend;
use super::annotation_geometry_stage::{AnnotationGeometryRequest, resolve_annotation_geometry};
use super::justifier::{JustificationRequest, Justifier};
use super::layout_debug_assembly::{LayoutDebugStageInput, build_layout_debug_info};
use super::line_break_planning_stage::{LineBreakPlanningStageResult, ParagraphLayoutPrep};
use super::line_geometry_stage::{
    build_line_boxes, renderable_glyph_run_clusters, resolve_line_vertical_geometry,
};
use super::line_optimization::RepairOption;
use super::paragraph_shaping_stage::map_to_cluster_range;
use super::progressive_break_decisions::{ProgressiveBreakTier, ShrinkChannel};
use super::white_space::LineWhiteSpaceAction;

/// Kotlin `finishParagraphLayout` 的 Rust stage outcome。
///
/// FIXME(strict-mirror): Kotlin 通过 `ParagraphLayoutEngine` extension receiver
/// 直接调用主入口的 `layoutWithRejectedTechnicalTiers`。Rust 将 stage 划为显式 request/outcome；
/// `Retry` 的重放控制流必须且仅能由 `ParagraphLayoutEngine.rs` 执行，不能迁入本文件。
#[derive(Clone, Debug, PartialEq)]
pub enum LineAdjustmentStageOutcome {
    Finished(Box<LayoutResult>),
    Retry {
        rejected_technical_tiers_by_span: HashMap<TextRange, HashSet<ProgressiveBreakTier>>,
    },
}

pub struct LineAdjustmentRequest<'a> {
    pub prep: ParagraphLayoutPrep,
    pub plan: &'a LineBreakPlanningStageResult,
    pub justifier: &'a Justifier,
    pub line_breaker_strategy_name: &'static str,
    pub font_backend: &'a dyn FontBackend,
}

pub fn finish_paragraph_layout(request: LineAdjustmentRequest<'_>) -> LineAdjustmentStageOutcome {
    let prep = &request.prep;
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
                    Some(if decision.reason.is_attached_ascii_point_mark() {
                        ImpossibleMeasureFallback::AttachedAsciiPointMarkImpossibleMeasureHang
                    } else {
                        ImpossibleMeasureFallback::InlineObjectAttachedMarkImpossibleMeasureHang
                    })
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
    let mut trimmed_clusters = trimmed_geometry.resolve_clusters();
    for line in &line_solution.lines {
        for adjustment in &line.white_space_adjustments {
            match adjustment.action {
                LineWhiteSpaceAction::ResolveTab
                | LineWhiteSpaceAction::RemoveStart
                | LineWhiteSpaceAction::RemoveEnd => {
                    trimmed_clusters[adjustment.cluster_index as usize].advance = adjustment.occupied_advance;
                }
                _ => {}
            }
        }
        if let Some(measurement) = &line.measurement {
            measurement.resolve_final_tabs(line.cluster_range, &mut trimmed_clusters);
        }
    }
    let mut edge_trim_decisions = edge_trim_result.decisions;
    edge_trim_decisions.extend(auto_space_edge_decisions);

    let mut justification_plans: Vec<_> = line_solution
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
            let range = line.measurement.as_ref().map_or(line.in_measure_cluster_range(), |measurement| {
                measurement.alignment_range(line.in_measure_cluster_range())
            });
            let justification = JustificationRequest {
                adjusted_clusters: &trimmed_clusters,
                cluster_roles: &prep.cluster_roles,
                east_asian_spacing_edges: &prep.east_asian_spacing_edges,
                line_cluster_range: range,
                max_width: (if line.cluster_range.first() == 0 {
                    prep.measure - plan.first_line_indent
                } else {
                    prep.measure - plan.block_indent
                }) - line_hyphen_advance_at(line_index),
                font_size: prep.font_size,
                skip: false,
                skip_reason: None,
                allow_sino_western_gap_stretch: prep
                    .adjustment_style
                    .allow_sino_western_gap_adjustment,
                cjk_latin_space_base_em: prep.clreq_profile.auto_space.gap_em,
                cjk_latin_space_max_em: prep.clreq_profile.auto_space.stretch_max_em,
                no_stretch_boundary_clusters: &plan.no_stretch_boundary_clusters,
                no_stretch_boundary_after_clusters: &plan.no_stretch_boundary_after_clusters,
                western_bracket_cjk_inter_char_boundary_after_clusters: &plan
                    .western_bracket_cjk_inter_char_boundary_after_clusters,
                attached_inline_physical_boundary_after_clusters: &plan
                    .attached_inline_physical_boundary_after_clusters,
                attached_inline_virtual_boundary_after_clusters: &plan
                    .attached_inline_virtual_boundary_after_clusters,
                attached_inline_virtual_sino_western_boundary_after_clusters: &plan
                    .attached_inline_virtual_sino_western_boundary_after_clusters,
                uniform_inline_object_boundary_after_clusters: &prep
                    .uniform_inline_object_boundary_after_clusters,
                preferred_inline_object_boundary_after_clusters: &prep
                    .preferred_inline_object_boundary_after_clusters,
                technical_boundary_after_clusters: &plan.technical_boundary_after_clusters,
                emergency_tracking_boundary_after_clusters: &plan
                    .emergency_tracking_boundary_after_clusters,
                preferred_emergency_tracking_boundary_after_clusters:
                    &preferred_emergency_tracking_boundaries,
            };
            let mut result = request.justifier.justify(justification.clone());
            if let Some(measurement) = &line.measurement
                && measurement.has_tabs(line.cluster_range)
                && !result.allocations.is_empty()
            {
                // Tab 会吸收其前方拉伸；只在含 Tab 的最终行上求解预算，不改机会或容量。
                let target = justification.max_width;
                let original_deficit = result.deficit_before;
                let stages = request.justifier.prepare_stages(justification.clone());
                let mut low = 0.0;
                let mut high = original_deficit + measurement.justification_tab_budget(line.cluster_range);
                let mut best_budget = 0.0;
                let mut deltas = vec![0.0; (range.last() - range.first() + 1).max(0) as usize];
                let mut final_width = |budget| {
                    stages.fill_deltas(budget, &mut deltas);
                    measurement.justified_width(range, &trimmed_clusters, &deltas)
                };
                let mut best_width = final_width(low);
                let upper_width = final_width(high);
                if upper_width <= target {
                    best_width = upper_width;
                    low = high;
                    best_budget = low;
                } else {
                    // 固定次数限制浮点求解成本；无 Tab 的常见路径不进入此循环。
                    for _ in 0..32 {
                        let middle = (low + high) * 0.5;
                        if middle == low || middle == high { break; }
                        let width = final_width(middle);
                        if width <= target {
                            low = middle;
                            best_budget = middle;
                            best_width = width;
                        } else { high = middle; }
                    }
                }
                // 在 Tab 吸收形成的平台上取首次达到最佳宽度的预算，避免无效耗尽容量。
                let mut left = 0.0;
                let mut right = low;
                for _ in 0..32 {
                    let middle = (left + right) * 0.5;
                    if middle == left || middle == right { break; }
                    let width = final_width(middle);
                    if width >= best_width && width <= target {
                        right = middle;
                        best_budget = middle;
                    } else { left = middle; }
                }
                result = stages.plan(best_budget);
                result.deficit_before = original_deficit;
                result.unfilled_deficit = (target - best_width).max(0.0);
            }
            Some(result)
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
                super::punctuation_model::GlueKind::CjkInterChar
                    | super::punctuation_model::GlueKind::EmergencyGraphemeTracking
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
    let mut final_clusters: Vec<_> = final_geometry
        .resolve_clusters()
        .into_iter()
        .enumerate()
        .map(|(cluster_index, mut cluster)| {
            cluster.advance = (trimmed_clusters[cluster_index].advance
                + justify_delta_by_cluster.get(&(cluster_index as i32)).copied().unwrap_or(0.0)).max(0.0);
            if let Some(&index) = plan.metric_decision_index_by_range.get(&cluster.range) {
                let metric = &plan.metric_decisions[index];
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
    // 两端对齐后 Tab 仍使用正文块原点和相同跳站阈值。
    for line in &line_solution.lines {
        if let Some(measurement) = &line.measurement {
            measurement.resolve_final_tabs(line.cluster_range, &mut final_clusters);
        }
    }
    let mut geometry_decisions = final_geometry.to_decision_info();
    let mut geometry_cursor = 0;
    for cluster in &final_clusters {
        if let Some(decision) = geometry_decisions.get_mut(geometry_cursor)
            && decision.range == cluster.range {
            decision.resolved_advance = cluster.advance;
            geometry_cursor += 1;
        }
    }
    for (index, (line, justification)) in line_solution.lines.iter().zip(&mut justification_plans).enumerate() {
        if let Some(justification) = justification {
            let occupied: f32 = line.cluster_range.into_iter().filter(|index| !line.hanging_cluster_indices.contains(index))
                .map(|index| final_clusters[index as usize].advance).sum();
            let hanging = line.measurement.as_ref().map_or(0.0, |measurement| measurement.final_hanging_advance(
                line.cluster_range, line.end_reason, line.measurement_limit, &final_clusters));
            let target = prep.measure - if line.cluster_range.first() == 0 { plan.first_line_indent } else { plan.block_indent }
                - line_hyphen_advance_at(index);
            justification.unfilled_deficit = (target - (occupied - hanging)).max(0.0);
        }
    }
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
        font_backend: request.font_backend,
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
    let size = Size {
        width: widest_line.min(prep.input.constraints.max_width()),
        height: total_height,
    };
    let prep = request.prep;
    let mut debug = build_layout_debug_info(LayoutDebugStageInput {
        text: &prep.text,
        font_resolutions: &prep.font_resolutions,
        substitution_rollbacks: &prep.substitution_rollbacks,
        punctuation_glyph_substitutor: &prep.punctuation_glyph_substitutor,
        substitutions: &prep.substitutions,
        shaping_decisions: prep.shaping_decisions,
        metric_decisions: &plan.metric_decisions,
        punctuation_atoms: &prep.punctuation_atoms,
        geometry_decisions,
        spacing_plan: &prep.spacing_plan,
        attached_punctuation_boundary: &prep.attached_punctuation_boundary,
        role_override_infos: &prep.role_override_infos,
        line_breaker_strategy_name: request.line_breaker_strategy_name,
        laid_out_lines: &line_boxes.laid_out_lines,
        line_solution,
        clusters: &prep.clusters,
        justification_plans: &justification_plans,
        auto_space_decisions: &prep.auto_space_decisions,
        edge_trim_decisions,
        decoration_decisions: annotation.decoration_decisions,
        decoration_segments: annotation.decoration_segments,
        ruby_decisions: annotation.ruby_decisions,
        bopomofo_decisions: annotation.bopomofo_decisions,
        mandatory_break_decisions: &prep.mandatory_break_decisions,
        max_lines_decision: line_boxes.max_lines_decision,
        line_spacing_decision: plan.line_spacing_decision.clone(),
        ruby_line_height_decision: vertical_geometry.ruby_line_height_decision,
        inline_object_line_height_decision: vertical_geometry.inline_object_line_height_decision,
        kinsoku_decision: plan.kinsoku_decision.clone(),
        contextual_kinsoku_decisions,
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
    debug.white_space_units = prep.natural_clusters.iter()
        .filter_map(|cluster| prep.white_space_units.get(&cluster.range).copied()).collect();
    for (index, line) in line_solution.lines.iter().enumerate() {
        for adjustment in &line.white_space_adjustments {
            let mut adjustment = *adjustment;
            if adjustment.action == LineWhiteSpaceAction::ResolveTab {
                adjustment.occupied_advance = final_clusters[adjustment.cluster_index as usize].advance;
            }
            if matches!(adjustment.action, LineWhiteSpaceAction::HangEnd | LineWhiteSpaceAction::ConditionallyHangEnd) {
                continue;
            }
            debug.line_white_space_adjustments.push(crate::core::layout_model::LineWhiteSpaceDecisionInfo {
                line_index: index as i32, line_range: line.source_range, adjustment,
            });
        }
        if let Some(measurement) = &line.measurement {
            for adjustment in measurement.final_hanging_adjustments(line.cluster_range, line.end_reason,
                line.measurement_limit, &final_clusters) {
                debug.line_white_space_adjustments.push(crate::core::layout_model::LineWhiteSpaceDecisionInfo {
                    line_index: index as i32, line_range: line.source_range, adjustment,
                });
            }
        }
    }
    LineAdjustmentStageOutcome::Finished(Box::new(LayoutResult::with_debug(
        prep.input,
        size,
        final_clusters,
        glyph_runs,
        lines,
        debug,
    )))
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
        for adjustment in line.white_space_adjustments.iter().rev() {
            if matches!(adjustment.action, LineWhiteSpaceAction::RemoveAutoSpaceStart
                | LineWhiteSpaceAction::RemoveAutoSpaceEnd) {
                let cluster_index = adjustment.cluster_index;
                let side = if adjustment.action == LineWhiteSpaceAction::RemoveAutoSpaceStart { LineEdgeTrimSide::Leading } else { LineEdgeTrimSide::Trailing };
                add_amount(&mut trims, cluster_index, adjustment.occupied_advance);
                decisions.push(LineEdgeTrimDecisionInfo {
                    line_range: line.source_range,
                    cluster_range: prep.natural_clusters[cluster_index as usize].range,
                    side,
                    trim_amount: adjustment.occupied_advance,
                    consumed_before: 0.0,
                    natural_glue: auto_space_gap,
                    reason: LineEdgeTrimReason::TextAutoSpaceLineEdgeTrim,
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
                side: LineEdgeTrimSide::Trailing,
                trim_amount: *amount,
                consumed_before: 0.0,
                natural_glue: *amount,
                reason: LineEdgeTrimReason::AttachedInlineVirtualBoundaryLineEndTrim,
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
                    side: LineEdgeTrimSide::Trailing,
                    trim_amount: remaining,
                    consumed_before,
                    natural_glue: discardable,
                    reason: LineEdgeTrimReason::InlineObjectLineEndDiscardableGlue,
                });
            }
        }
    }
    (trims, decisions)
}

fn build_glyph_runs(prep: &ParagraphLayoutPrep, final_clusters: &[Cluster]) -> Vec<GlyphRun> {
    renderable_glyph_run_clusters(final_clusters, &prep.open_type_features_by_cluster_range)
        .into_iter()
        .filter_map(|clusters| {
            let face = clusters[0].font_face.clone().or_else(|| {
                prep.font_resolutions
                    .values()
                    .find(|resolution| {
                        resolution.range.start() <= clusters[0].range.start()
                            && resolution.range.end() >= clusters[clusters.len() - 1].range.end()
                    })
                    .map(|resolution| resolution.face.clone())
            });
            let Some(face) = face else {
                log::warn!("renderable glyph run lacks font face; skipping glyph run");
                return None;
            };
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
            Some(GlyphRun::with_open_type_features(
                TextRange::new(
                    clusters.first().unwrap().range.start(),
                    clusters.last().unwrap().range.end(),
                ),
                face,
                glyphs,
                clusters.iter().map(|cluster| cluster.advance).sum(),
                open_type_features,
            ))
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
const CURRENT_LINE_TECHNICAL_BODY_STRETCH_LIMIT_EM: f32 = 0.0;
const TECHNICAL_STRETCH_EPSILON_PX: f32 = 0.001;
