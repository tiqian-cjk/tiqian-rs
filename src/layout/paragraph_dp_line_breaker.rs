// 对应 Kotlin 源文件：engine/src/commonMain/kotlin/org/tiqian/layout/ParagraphDpLineBreaker.kt

use crate::common::{HashMap, HashSet};
use std::rc::Rc;

use super::super::core::geometry::TextRange;
use super::super::core::int_range::IntRange;
use super::super::core::layout_model::{Cluster, LineEndReason};
use super::kinsoku_rule::{ClreqKinsokuRule, KinsokuRule};
use super::line_breaker::{
    LineBreaker, LineBreakerConfig, adjust_break_for_line_end, aligned_adjusted_clusters,
    empty_line_candidate,
};
use super::line_optimization::{
    LineCandidate, LineSolution, PushInAllocation, RepairDetail, RepairOption, RepairReasonCode,
};
use super::line_repair::{apply_kinsoku_repairs, distribute_push_in_shrink};
use super::progressive_break_decisions::{
    ProgressiveBreakOpportunity, ShrinkChannel, ShrinkOpportunity, UnbreakableRanges,
    adjust_break_for_unbreakables, decide_hyphen_break, decide_progressive_break, line_limit,
    progressive_candidate_allowed,
};

const HYPHEN_RUN_STATE_CAP: i32 = 3;
const STRETCH_RUN_STATE_CAP: i32 = 3;
const VISIBLE_STRETCH_FLOOR_PX: f32 = 0.5;

/// 基于摊销行调整成本的段落动态规划断行器。
pub struct ParagraphDpLineBreaker {
    pub candidate_window: i32,
    pub raggedness_weight: f32,
    pub kinsoku: Box<dyn KinsokuRule>,
    pub push_in_penalty: i32,
    pub carry_previous_penalty: i32,
    pub leave_ragged_penalty: i32,
    pub synthetic_hyphen_break_penalty: f32,
    pub consecutive_synthetic_hyphen_penalty: f32,
    pub consecutive_stretch_penalty: f32,
    pub compression_visibility: f32,
}

impl Default for ParagraphDpLineBreaker {
    fn default() -> Self {
        Self {
            candidate_window: 8,
            raggedness_weight: 0.5,
            kinsoku: Box::new(ClreqKinsokuRule::default()),
            push_in_penalty: 2,
            carry_previous_penalty: 10,
            leave_ragged_penalty: 20,
            synthetic_hyphen_break_penalty: 12.0,
            consecutive_synthetic_hyphen_penalty: 12.0,
            consecutive_stretch_penalty: 3.0,
            compression_visibility: 1.0,
        }
    }
}

struct DpContext<'a> {
    config: &'a LineBreakerConfig,
    natural_clusters: &'a [Cluster],
    adjusted_clusters: &'a [Cluster],
    max_width: f32,
    shrink_opportunities: &'a [ShrinkOpportunity],
    unbreakable_ranges: &'a UnbreakableRanges,
    first_line_indent: f32,
    forbidden_line_start_clusters: Option<&'a HashSet<i32>>,
    forbidden_line_end_clusters: &'a HashSet<i32>,
    hyphen_break_clusters: &'a HashSet<i32>,
    cjk_inter_char_boundaries: &'a HashSet<i32>,
    max_cjk_stretch_per_gap: f32,
    sino_western_boundaries: &'a HashSet<i32>,
    sino_western_stretch_cap: f32,
    non_rendering_control_clusters: &'a HashSet<i32>,
    candidate_window: i32,
    d_ref: f32,
    allow_compression_edges: bool,
    progressive_break_opportunities: &'a HashMap<i32, ProgressiveBreakOpportunity>,
    gap_prefix: Vec<i32>,
    sino_prefix: Vec<i32>,
    cjk_prefix: Vec<i32>,
    natural_prefix: Vec<f32>,
    adjusted_prefix: Vec<f32>,
    shrink_prefix: Vec<f32>,
    line_end_only_capacity: Vec<f32>,
}

impl<'a> DpContext<'a> {
    fn new(
        natural_clusters: &'a [Cluster],
        adjusted_clusters: &'a [Cluster],
        max_width: f32,
        config: &'a LineBreakerConfig,
        candidate_window: i32,
    ) -> Self {
        // 共享测量已保存宽度与间隙前缀；这些索引仅供未接入共享测量的独立调用使用。
        let count = if config.measurement.is_some() { 0 } else { adjusted_clusters.len() };
        let mut gap_boundaries = config.cjk_inter_char_boundaries.clone();
        gap_boundaries.extend(&config.sino_western_boundaries);
        let mut gap_prefix = vec![0; count + 1];
        let mut sino_prefix = vec![0; count + 1];
        let mut cjk_prefix = vec![0; count + 1];
        let mut natural_prefix = vec![0.0; count + 1];
        let mut adjusted_prefix = vec![0.0; count + 1];
        for index in 0..count {
            gap_prefix[index + 1] = gap_prefix[index]
                + if gap_boundaries.contains(&(index as i32)) {
                    1
                } else {
                    0
                };
            sino_prefix[index + 1] = sino_prefix[index]
                + if config.sino_western_boundaries.contains(&(index as i32)) {
                    1
                } else {
                    0
                };
            cjk_prefix[index + 1] = cjk_prefix[index]
                + if config.cjk_inter_char_boundaries.contains(&(index as i32)) {
                    1
                } else {
                    0
                };
            natural_prefix[index + 1] = natural_prefix[index] + natural_clusters[index].advance;
            adjusted_prefix[index + 1] = adjusted_prefix[index] + adjusted_clusters[index].advance;
        }
        let mut shrink_prefix = vec![0.0; count + 1];
        let mut line_end_only_capacity = vec![0.0; count];
        for opportunity in &config.shrink_opportunities {
            if opportunity.capacity <= 0.0
                || opportunity.cluster_index < 0
                || opportunity.cluster_index >= count as i32
            {
                continue;
            }
            if opportunity.line_end_only {
                line_end_only_capacity[opportunity.cluster_index as usize] += opportunity.capacity;
            } else {
                shrink_prefix[opportunity.cluster_index as usize + 1] += opportunity.capacity;
            }
        }
        for index in 0..count {
            shrink_prefix[index + 1] += shrink_prefix[index];
        }
        Self {
            config,
            natural_clusters,
            adjusted_clusters,
            max_width,
            shrink_opportunities: &config.shrink_opportunities,
            unbreakable_ranges: &config.unbreakable_ranges,
            first_line_indent: config.first_line_indent,
            forbidden_line_start_clusters: config.forbidden_line_start_clusters.as_ref(),
            forbidden_line_end_clusters: &config.forbidden_line_end_clusters,
            hyphen_break_clusters: &config.hyphen_break_clusters,
            cjk_inter_char_boundaries: &config.cjk_inter_char_boundaries,
            max_cjk_stretch_per_gap: config.max_cjk_stretch_per_gap,
            sino_western_boundaries: &config.sino_western_boundaries,
            sino_western_stretch_cap: config.sino_western_stretch_cap,
            non_rendering_control_clusters: &config.non_rendering_control_clusters,
            candidate_window,
            d_ref: config.max_cjk_stretch_per_gap,
            allow_compression_edges: config.line_adjustment_push_in,
            progressive_break_opportunities: &config.progressive_break_opportunities,
            gap_prefix,
            sino_prefix,
            cjk_prefix,
            natural_prefix,
            adjusted_prefix,
            shrink_prefix,
            line_end_only_capacity,
        }
    }

    fn build_line(&self, cluster_range: IntRange, end_reason: LineEndReason) -> LineCandidate {
        self.config.build_line(
            cluster_range,
            self.natural_clusters,
            self.adjusted_clusters,
            end_reason,
            self.max_width,
        )
    }

    fn candidate_line(&self, cluster_range: IntRange, end_reason: LineEndReason) -> LineCandidate {
        if self.config.measurement.is_some() {
            return self.build_line(cluster_range, end_reason);
        }
        let first = cluster_range.first() as usize;
        let last = cluster_range.last() as usize;
        let mut line = LineCandidate::new(
            cluster_range,
            TextRange::new(
                self.adjusted_clusters[first].range.start(),
                self.adjusted_clusters[last].range.end(),
            ),
            self.natural_prefix[last + 1] - self.natural_prefix[first],
            self.adjusted_prefix[last + 1] - self.adjusted_prefix[first],
        );
        line.end_reason = end_reason;
        line.measurement = self.config.measurement.clone();
        line.measurement_limit = line_limit(self.max_width, self.first_line_indent, first as i32);
        if let Some(measurement) = &line.measurement {
            let measured = measurement.measure(cluster_range, end_reason, line.measurement_limit, &[]);
            line.natural_width = measured.natural;
            line.adjusted_width = measured.adjusted;
        }
        line
    }

    fn compression_opportunities(
        &self,
        range: IntRange,
        reason: LineEndReason,
    ) -> Vec<ShrinkOpportunity> {
        let mut opportunities: Vec<_> = self.shrink_opportunities.iter().filter(|opp| {
            range.contains(opp.cluster_index)
                && opp.capacity > 0.0
                && (!opp.line_end_only || opp.cluster_index == range.last())
                && self.config.measurement.as_ref().is_none_or(|measurement| {
                    measurement.shrink_is_effective(range, reason, opp.cluster_index)
                })
        }).copied().collect();
        for opp in &mut opportunities {
            if opp.cluster_index == range.last()
                && matches!(opp.channel, ShrinkChannel::TrailingGlue | ShrinkChannel::LeadingAndTrailingGlue)
            {
                opp.tier = 1;
            }
        }
        opportunities
    }

    fn compressed_width(
        &self,
        range: IntRange,
        reason: LineEndReason,
        limit: f32,
    ) -> Option<(f32, Vec<PushInAllocation>)> {
        let opportunities = self.compression_opportunities(range, reason);
        let mut allocations = if let Some(measurement) = &self.config.measurement {
            measurement.compression(range, reason, limit, &opportunities)?
        } else {
            let total_shrink = self.adjusted_prefix[range.last() as usize + 1]
                - self.adjusted_prefix[range.first() as usize]
                - limit;
            if total_shrink > opportunities.iter().map(|opp| opp.capacity).sum() {
                return None;
            }
            distribute_push_in_shrink(&opportunities, total_shrink.max(0.0))
        };
        allocations.sort_by_key(|allocation| allocation.cluster_index);
        let width = self.config.measurement.as_ref().map_or_else(|| self.adjusted_prefix[range.last() as usize + 1]
            - self.adjusted_prefix[range.first() as usize]
            - allocations.iter().map(|allocation| allocation.shrink).sum::<f32>(),
            |measurement| measurement.measure(range, reason, limit, &allocations).adjusted);
        (width <= limit).then_some((width, allocations))
    }

    fn gap_count(&self, range: IntRange) -> i32 {
        if let Some(measurement) = &self.config.measurement {
            return measurement.effective_gap_count(range);
        }
        if range.is_empty() {
            0
        } else {
            self.gap_prefix[range.last() as usize] - self.gap_prefix[range.first() as usize]
        }
    }

    fn sino_gap_count(&self, range: IntRange) -> i32 {
        if let Some(measurement) = &self.config.measurement {
            return measurement.progressive_stats(range, None).1;
        }
        if range.is_empty() {
            0
        } else {
            self.sino_prefix[range.last() as usize] - self.sino_prefix[range.first() as usize]
        }
    }

    fn cjk_gap_count(&self, range: IntRange) -> i32 {
        if let Some(measurement) = &self.config.measurement {
            return measurement.progressive_stats(range, None).2;
        }
        if range.is_empty() {
            0
        } else {
            self.cjk_prefix[range.last() as usize] - self.cjk_prefix[range.first() as usize]
        }
    }

    fn shrink_capacity(&self, range: IntRange) -> f32 {
        self.shrink_prefix[range.last() as usize + 1] - self.shrink_prefix[range.first() as usize]
            + self.line_end_only_capacity[range.last() as usize]
    }
}

struct EdgeState {
    start: i32,
    end: i32,
    hyphen_run: i32,
    stretch_run: i32,
    cost: f32,
    parent: Option<Rc<EdgeState>>,
}

struct EdgeGeometry {
    base_cost: f32,
    visible_stretch: bool,
}

impl ParagraphDpLineBreaker {
    fn candidate_ends(
        &self,
        context: &DpContext<'_>,
        start: i32,
        segment_end_exclusive: i32,
        ends_with_mandatory: bool,
    ) -> Vec<i32> {
        let limit = line_limit(context.max_width, context.first_line_indent, start);
        let raw_greedy = context.config.greedy_end(
            context.adjusted_clusters,
            start,
            limit,
            segment_end_exclusive,
        );
        if raw_greedy >= segment_end_exclusive {
            return vec![segment_end_exclusive];
        }
        let progressive_greedy = decide_progressive_break(
            start,
            raw_greedy,
            context.progressive_break_opportunities,
            Some(context.adjusted_clusters),
            limit,
            context.cjk_inter_char_boundaries,
            context.max_cjk_stretch_per_gap,
            context.sino_western_boundaries,
            context.sino_western_stretch_cap,
            context.config.measurement.as_deref(),
        );
        let baseline = adjust_break_for_unbreakables(
            decide_hyphen_break(
                start,
                progressive_greedy,
                context.adjusted_clusters,
                limit,
                context.hyphen_break_clusters,
                context.cjk_inter_char_boundaries,
                context.max_cjk_stretch_per_gap,
                context.sino_western_boundaries,
                context.sino_western_stretch_cap,
                context.config.measurement.as_deref(),
            ),
            start,
            context.unbreakable_ranges,
        );
        let mut compressed = Vec::new();
        if context.allow_compression_edges {
            let first_end = raw_greedy + 1;
            let last_end = (first_end + context.candidate_window).min(segment_end_exclusive);
            for end in first_end..=last_end {
                if context.config.measurement.as_ref().is_some_and(|measurement| !measurement.boundary_allowed(end)) {
                    continue;
                }
                let reason = if end == segment_end_exclusive {
                    if ends_with_mandatory { LineEndReason::MandatoryBreak } else { LineEndReason::ParagraphEnd }
                } else { LineEndReason::AutoWrap };
                let range = IntRange::new(start, end - 1);
                let line = context.candidate_line(range, reason);
                let feasible = if context.config.measurement.is_some() {
                    line.adjusted_width <= limit
                        || context.compressed_width(range, reason, limit).is_some()
                } else {
                    line.adjusted_width - limit <= context.shrink_capacity(range)
                };
                if feasible { compressed.push(end); }
            }
        }
        let is_compressed_promotion = |candidate_end: i32| {
            if candidate_end <= raw_greedy {
                return false;
            }
            let Some(current) = context
                .progressive_break_opportunities
                .get(&progressive_greedy)
            else {
                return false;
            };
            let Some(resulting) = context.progressive_break_opportunities.get(&candidate_end)
            else {
                return false;
            };
            current.span_range == resulting.span_range
                && resulting.tier.priority() < current.tier.priority()
        };
        let mut filtered = Vec::new();
        for end in (raw_greedy - context.candidate_window)..=raw_greedy {
            if end < start + 1
                || end > segment_end_exclusive
                || (ends_with_mandatory && end == segment_end_exclusive - 1)
            {
                continue;
            }
            if context.unbreakable_ranges.contains_boundary(end) {
                continue;
            }
            if context.config.measurement.as_ref().is_some_and(|measurement| !measurement.boundary_allowed(end)) {
                continue;
            }
            if !is_compressed_promotion(end)
                && !progressive_candidate_allowed(
                    start,
                    raw_greedy,
                    end,
                    context.progressive_break_opportunities,
                    Some(context.adjusted_clusters),
                    limit,
                    context.cjk_inter_char_boundaries,
                    context.max_cjk_stretch_per_gap,
                    context.sino_western_boundaries,
                    context.sino_western_stretch_cap,
                    context.config.measurement.as_deref(),
                )
            {
                continue;
            }
            if end != segment_end_exclusive
                && !(start..end)
                    .any(|index| !context.non_rendering_control_clusters.contains(&index))
            {
                continue;
            }
            filtered.push(end);
        }
        for end in compressed {
            if end < start + 1
                || end > segment_end_exclusive
                || (ends_with_mandatory && end == segment_end_exclusive - 1)
            {
                continue;
            }
            if context.unbreakable_ranges.contains_boundary(end) {
                continue;
            }
            if context.config.measurement.as_ref().is_some_and(|measurement| !measurement.boundary_allowed(end)) {
                continue;
            }
            if !is_compressed_promotion(end)
                && !progressive_candidate_allowed(
                    start,
                    raw_greedy,
                    end,
                    context.progressive_break_opportunities,
                    Some(context.adjusted_clusters),
                    limit,
                    context.cjk_inter_char_boundaries,
                    context.max_cjk_stretch_per_gap,
                    context.sino_western_boundaries,
                    context.sino_western_stretch_cap,
                    context.config.measurement.as_deref(),
                )
            {
                continue;
            }
            if end != segment_end_exclusive
                && !(start..end)
                    .any(|index| !context.non_rendering_control_clusters.contains(&index))
            {
                continue;
            }
            filtered.push(end);
        }
        let clean: Vec<_> = filtered
            .iter()
            .copied()
            .filter(|end| {
                *end == segment_end_exclusive
                    || (!context
                        .forbidden_line_start_clusters
                        .is_some_and(|forbidden| forbidden.contains(end))
                        && !context.forbidden_line_end_clusters.contains(&(end - 1)))
            })
            .collect();
        let pool = if clean.is_empty() { filtered } else { clean };
        let promotions: Vec<_> = pool
            .iter()
            .copied()
            .filter(|end| is_compressed_promotion(*end))
            .collect();
        let tier_preferred_pool: Vec<_> = if promotions.is_empty() {
            pool
        } else {
            let mut best_priority: Option<i32> = None;
            let mut promoted_span = None;
            let mut promotions_complete = true;
            for end in &promotions {
                let Some(opportunity) = context.progressive_break_opportunities.get(end) else {
                    promotions_complete = false;
                    break;
                };
                best_priority = Some(
                    best_priority.map_or(opportunity.tier.priority(), |priority| {
                        priority.min(opportunity.tier.priority())
                    }),
                );
                promoted_span.get_or_insert(opportunity.span_range);
            }
            if let (Some(best_priority), Some(promoted_span)) = (best_priority, promoted_span)
                && promotions_complete
            {
                pool.into_iter()
                    .filter(|end| {
                        context
                            .progressive_break_opportunities
                            .get(end)
                            .is_none_or(|opportunity| {
                                opportunity.span_range != promoted_span
                                    || opportunity.tier.priority() <= best_priority
                            })
                    })
                    .collect()
            } else {
                pool
            }
        };
        let mut candidates = tier_preferred_pool;
        let baseline = context.config.measurement.as_ref().map_or(baseline, |measurement| {
            measurement.next_boundary(baseline, segment_end_exclusive)
        });
        if baseline > start && baseline <= segment_end_exclusive && promotions.is_empty()
            && context.config.measurement.as_ref().is_none_or(|measurement| measurement.boundary_allowed(baseline))
        {
            candidates.push(baseline);
        }
        let mut result = Vec::new();
        for candidate in candidates {
            if !result.contains(&candidate) {
                result.push(candidate);
            }
        }
        if result.is_empty() {
            let fallback = baseline.max(start + 1);
            vec![context.config.measurement.as_ref().map_or(fallback, |measurement| {
                measurement.next_boundary(fallback, segment_end_exclusive)
            })]
        } else {
            result
        }
    }

    fn edge_geometry(
        &self,
        context: &DpContext<'_>,
        line: &LineCandidate,
        is_segment_last: bool,
        hyphen_end: bool,
    ) -> EdgeGeometry {
        let limit = line_limit(
            context.max_width,
            context.first_line_indent,
            line.cluster_range.first(),
        );
        let in_measure = line.in_measure_cluster_range();
        let overflow = line.adjusted_width - limit;
        let orphan = if !is_segment_last && in_measure.first() == in_measure.last() {
            self.leave_ragged_penalty as f32
        } else {
            0.0
        };
        let hyphen_flat = if hyphen_end {
            self.synthetic_hyphen_break_penalty
        } else {
            0.0
        };
        let reference = context.d_ref.max(1.0);
        if overflow > 0.0 || matches!(&line.repair, Some(RepairOption::PushIn { .. })) {
            let shrink = match &line.repair {
                Some(RepairOption::PushIn { total_shrink, .. }) => *total_shrink,
                _ => overflow,
            };
            let density = shrink / context.gap_count(in_measure).max(1) as f32
                * self.compression_visibility;
            return EdgeGeometry {
                base_cost: orphan
                    + hyphen_flat
                    + density * density / reference * self.raggedness_weight,
                visible_stretch: false,
            };
        }
        let deficit = if is_segment_last {
            0.0
        } else {
            (limit - line.adjusted_width).max(0.0)
        };
        let sino_gaps = context.sino_gap_count(in_measure);
        let cjk_gaps = context.cjk_gap_count(in_measure);
        let sino_fill = if sino_gaps > 0 {
            deficit.min(sino_gaps as f32 * context.sino_western_stretch_cap)
        } else {
            0.0
        };
        let sino_density = if sino_gaps > 0 {
            sino_fill / sino_gaps as f32
        } else {
            0.0
        };
        let cjk_deficit = deficit - sino_fill;
        let cjk_density = if cjk_gaps > 0 {
            cjk_deficit / cjk_gaps as f32
        } else {
            0.0
        };
        let residual = if cjk_gaps == 0 { cjk_deficit } else { 0.0 };
        EdgeGeometry {
            base_cost: residual * self.raggedness_weight
                + orphan
                + hyphen_flat
                + (sino_density * sino_density + cjk_density * cjk_density) / reference
                    * self.raggedness_weight,
            visible_stretch: sino_density.max(cjk_density) > VISIBLE_STRETCH_FLOOR_PX,
        }
    }

    fn solve_segment(
        &self,
        context: &DpContext<'_>,
        segment_start: i32,
        segment_end_exclusive: i32,
        ends_with_mandatory: bool,
    ) -> Vec<i32> {
        let mut states_by_start: HashMap<i32, Vec<Rc<EdgeState>>> = HashMap::new();
        states_by_start.insert(segment_start, Vec::new());
        let mut best_by_key: HashMap<u64, Rc<EdgeState>> = HashMap::new();
        let mut terminal_best: Option<Rc<EdgeState>> = None;
        for start in segment_start..segment_end_exclusive {
            let incoming = if start == segment_start {
                vec![None]
            } else {
                match states_by_start.get(&start) {
                    Some(states) => states.iter().cloned().map(Some).collect(),
                    None => continue,
                }
            };
            if incoming.is_empty() {
                continue;
            }
            for end in
                self.candidate_ends(context, start, segment_end_exclusive, ends_with_mandatory)
            {
                let is_segment_last = end >= segment_end_exclusive;
                let reason = if !is_segment_last {
                    LineEndReason::AutoWrap
                } else if ends_with_mandatory {
                    LineEndReason::MandatoryBreak
                } else {
                    LineEndReason::ParagraphEnd
                };
                let range = IntRange::new(start, (end - 1).min(segment_end_exclusive - 1));
                let mut line = context.candidate_line(range, reason);
                let limit = line_limit(context.max_width, context.first_line_indent, start);
                let greedy_end = context.config.greedy_end(
                    context.adjusted_clusters,
                    start,
                    limit,
                    segment_end_exclusive,
                );
                let forced_natural_overflow = context.config.measurement.as_ref().is_some_and(|measurement| {
                    measurement.wrap == crate::core::text_model::TextWrapMode::NoWrap
                }) || greedy_end == end || (greedy_end == segment_end_exclusive && end == segment_end_exclusive);
                if line.adjusted_width > limit
                    && context.allow_compression_edges
                    && !forced_natural_overflow
                {
                    if let Some((width, allocations)) = context.compressed_width(range, reason, limit) {
                        line.adjusted_width = width;
                        line.repair = Some(RepairOption::PushIn {
                            penalty: self.push_in_penalty,
                            reason_code: RepairReasonCode::LineAdjustmentPushInDpFeasibility,
                            detail: RepairDetail::PushedIn,
                            offender_cluster_index: range.last(),
                            total_shrink: allocations.iter().map(|allocation| allocation.shrink).sum(),
                            total_available_capacity: context.compression_opportunities(range, reason).iter().map(|opp| opp.capacity).sum(),
                            allocations,
                        });
                    } else {
                        continue;
                    }
                }
                let hyphen_end = !is_segment_last && context.hyphen_break_clusters.contains(&end);
                let geometry = self.edge_geometry(context, &line, is_segment_last, hyphen_end);
                for previous in &incoming {
                    let previous_cost = previous.as_ref().map_or(0.0, |state| state.cost);
                    let previous_hyphen_run = previous.as_ref().map_or(0, |state| state.hyphen_run);
                    let previous_stretch_run =
                        previous.as_ref().map_or(0, |state| state.stretch_run);
                    let cost = previous_cost
                        + geometry.base_cost
                        + if hyphen_end {
                            self.consecutive_synthetic_hyphen_penalty * previous_hyphen_run as f32
                        } else {
                            0.0
                        }
                        + if geometry.visible_stretch {
                            self.consecutive_stretch_penalty * previous_stretch_run as f32
                        } else {
                            0.0
                        };
                    let hyphen_run = if hyphen_end {
                        (previous_hyphen_run + 1).min(HYPHEN_RUN_STATE_CAP)
                    } else {
                        0
                    };
                    let stretch_run = if geometry.visible_stretch {
                        (previous_stretch_run + 1).min(STRETCH_RUN_STATE_CAP)
                    } else {
                        0
                    };
                    let key = ((start as u64) << 36)
                        | ((end as u64) << 4)
                        | ((hyphen_run as u64) << 2)
                        | stretch_run as u64;
                    if best_by_key
                        .get(&key)
                        .is_some_and(|existing| existing.cost <= cost)
                    {
                        continue;
                    }
                    let state = Rc::new(EdgeState {
                        start,
                        end,
                        hyphen_run,
                        stretch_run,
                        cost,
                        parent: previous.clone(),
                    });
                    best_by_key.insert(key, state.clone());
                    if is_segment_last {
                        if terminal_best
                            .as_ref()
                            .is_none_or(|existing| cost < existing.cost)
                        {
                            terminal_best = Some(state);
                        }
                    } else {
                        let bucket = states_by_start.entry(end).or_default();
                        bucket.retain(|existing| {
                            !(existing.start == start
                                && existing.hyphen_run == hyphen_run
                                && existing.stretch_run == stretch_run)
                        });
                        bucket.push(state);
                    }
                }
            }
        }
        let Some(terminal) = terminal_best else {
            return self.greedy_fallback_ends(context, segment_start, segment_end_exclusive);
        };
        let mut ends = Vec::new();
        let mut cursor = Some(terminal);
        while let Some(state) = cursor {
            ends.push(state.end);
            cursor = state.parent.clone();
        }
        ends.reverse();
        ends
    }

    fn greedy_fallback_ends(
        &self,
        context: &DpContext<'_>,
        segment_start: i32,
        segment_end_exclusive: i32,
    ) -> Vec<i32> {
        let mut ends = Vec::new();
        let mut start = segment_start;
        while start < segment_end_exclusive {
            let limit = line_limit(context.max_width, context.first_line_indent, start);
            let raw_greedy = context.config.greedy_end(
                context.adjusted_clusters,
                start,
                limit,
                segment_end_exclusive,
            );
            let end = if raw_greedy >= segment_end_exclusive {
                segment_end_exclusive
            } else {
                adjust_break_for_unbreakables(
                    decide_hyphen_break(
                        start,
                        decide_progressive_break(
                            start,
                            raw_greedy,
                            context.progressive_break_opportunities,
                            Some(context.adjusted_clusters),
                            limit,
                            context.cjk_inter_char_boundaries,
                            context.max_cjk_stretch_per_gap,
                            context.sino_western_boundaries,
                            context.sino_western_stretch_cap,
                            context.config.measurement.as_deref(),
                        ),
                        context.adjusted_clusters,
                        limit,
                        context.hyphen_break_clusters,
                        context.cjk_inter_char_boundaries,
                        context.max_cjk_stretch_per_gap,
                        context.sino_western_boundaries,
                        context.sino_western_stretch_cap,
                        context.config.measurement.as_deref(),
                    ),
                    start,
                    context.unbreakable_ranges,
                )
                .max(start + 1)
            };
            let end = context.config.measurement.as_ref().map_or(end, |measurement| {
                measurement.next_boundary(end, segment_end_exclusive)
            });
            ends.push(end);
            start = end;
        }
        ends
    }

    fn commit_segment(
        &self,
        committed: &mut Vec<LineCandidate>,
        ends: &[i32],
        segment_start: i32,
        mandatory_end: Option<i32>,
        context: &DpContext<'_>,
        hard_break_after_clusters: &HashSet<i32>,
    ) {
        let Some(last_end) = ends.last().copied() else {
            return;
        };
        let mut line_start = segment_start;
        for chosen_end in ends {
            if line_start >= *chosen_end {
                continue;
            }
            let is_final = *chosen_end == last_end;
            let end_reason = if is_final && mandatory_end.is_some() {
                LineEndReason::MandatoryBreak
            } else if is_final {
                LineEndReason::ParagraphEnd
            } else {
                LineEndReason::AutoWrap
            };
            let last_index = if is_final {
                mandatory_end.unwrap_or(chosen_end - 1)
            } else {
                chosen_end - 1
            };
            let limit = line_limit(context.max_width, context.first_line_indent, line_start);
            let segment_end_exclusive = mandatory_end.map_or(
                context.adjusted_clusters.len() as i32,
                |end| end + 1,
            );
            let range = IntRange::new(line_start, last_index);
            let mut line = context.build_line(range, end_reason);
            line.remeasure();
            let mut compression_failed = false;
            let raw_greedy = context.config.greedy_end(
                    context.adjusted_clusters,
                    line_start,
                    limit,
                    segment_end_exclusive,
                );
            let forced_natural_overflow = context.config.measurement.as_ref().is_some_and(|measurement| {
                measurement.wrap == crate::core::text_model::TextWrapMode::NoWrap
            }) || raw_greedy == *chosen_end
                || (raw_greedy == last_end && *chosen_end == last_end);
            if line.adjusted_width > limit
                && context.allow_compression_edges
                && !forced_natural_overflow
            {
                let original_break = context.progressive_break_opportunities.get(
                    &decide_progressive_break(
                        line_start,
                        raw_greedy,
                        context.progressive_break_opportunities,
                        Some(context.adjusted_clusters),
                        limit,
                        context.cjk_inter_char_boundaries,
                        context.max_cjk_stretch_per_gap,
                        context.sino_western_boundaries,
                        context.sino_western_stretch_cap,
                        context.config.measurement.as_deref(),
                    ),
                );
                let promotes_tier = original_break
                    .zip(context.progressive_break_opportunities.get(chosen_end))
                    .is_some_and(|(original, resulting)| {
                        original.span_range == resulting.span_range
                            && resulting.tier.priority() < original.tier.priority()
                    });
                let opportunities = context.compression_opportunities(range, end_reason);
                let compressed = if let Some(measurement) = &context.config.measurement {
                    measurement.compression(range, end_reason, limit, &opportunities)
                } else {
                    let overflow = line.adjusted_width - limit;
                    let capacity: f32 = opportunities.iter().map(|opp| opp.capacity).sum();
                    if overflow <= capacity {
                        Some(distribute_push_in_shrink(&opportunities, overflow.max(0.0)))
                    } else {
                        None
                    }
                };
                if let Some(mut allocations) = compressed {
                    allocations.sort_by_key(|allocation| allocation.cluster_index);
                    let total_shrink = allocations.iter().map(|allocation| allocation.shrink).sum();
                    let width = context.config.measurement.as_ref().map_or(
                        line.adjusted_width - total_shrink,
                        |measurement| measurement.measure(range, end_reason, limit, &allocations).adjusted,
                    );
                    if width <= limit {
                        line.adjusted_width = width;
                        line.repair = Some(RepairOption::PushIn {
                            penalty: self.push_in_penalty,
                            reason_code: if promotes_tier {
                                RepairReasonCode::ProgressiveTechnicalTierPromotion
                            } else {
                                RepairReasonCode::LineAdjustmentPushIn
                            },
                            detail: RepairDetail::PushedIn,
                            offender_cluster_index: range.last(),
                            allocations,
                            total_shrink,
                            total_available_capacity: opportunities.iter().map(|opp| opp.capacity).sum(),
                        });
                        line.remeasure();
                    } else {
                        compression_failed = true;
                    }
                } else {
                    compression_failed = true;
                }
            }
            if compression_failed {
                log::warn!("warn");
                let fallback = self.greedy_fallback_ends(context, line_start, last_end);
                let mut fallback_start = line_start;
                for fallback_end in fallback {
                    let is_final_fallback = fallback_end == last_end;
                    let fallback_reason = if is_final_fallback && mandatory_end.is_some() {
                        LineEndReason::MandatoryBreak
                    } else if is_final_fallback {
                        LineEndReason::ParagraphEnd
                    } else {
                        LineEndReason::AutoWrap
                    };
                    let fallback_last = if is_final_fallback {
                        mandatory_end.unwrap_or(fallback_end - 1)
                    } else {
                        fallback_end - 1
                    };
                    let mut fallback_line = context.build_line(
                        IntRange::new(fallback_start, fallback_last),
                        fallback_reason,
                    );
                    fallback_line.remeasure();
                    committed.push(fallback_line);
                    fallback_start = fallback_end;
                }
                return;
            }
            if is_final && let Some(mandatory_end) = mandatory_end {
                committed.push(line);
                line_start = mandatory_end + 1;
                if line_start == context.adjusted_clusters.len() as i32 {
                    if let Some(last) = context.adjusted_clusters.last() {
                        committed.push(empty_line_candidate(
                            last.range.end(),
                            LineEndReason::ParagraphEnd,
                        ));
                    }
                }
                continue;
            }
            if is_final {
                committed.push(line);
                line_start = *chosen_end;
                continue;
            }
            if line.repair.is_some() {
                committed.push(line);
                line_start = *chosen_end;
                continue;
            }
            let committed_end = adjust_break_for_line_end(
                *chosen_end,
                line_start,
                context.forbidden_line_end_clusters,
            );
            if hard_break_after_clusters.contains(&committed_end) && line_start < committed_end {
                let mut hard_line = context.build_line(
                    IntRange::new(line_start, committed_end),
                    LineEndReason::MandatoryBreak,
                );
                hard_line.remeasure();
                committed.push(hard_line);
                line_start = committed_end + 1;
                continue;
            }
            let committed_end = context.config.measurement.as_ref().map_or(committed_end,
                |measurement| measurement.next_boundary(committed_end, last_end));
            if committed_end != *chosen_end {
                line = context.build_line(IntRange::new(line_start, committed_end - 1), end_reason);
            }
            committed.push(line);
            line_start = committed_end;
        }
    }
}

impl LineBreaker for ParagraphDpLineBreaker {
    fn strategy_name(&self) -> &'static str {
        "paragraph-dp"
    }

    fn break_lines(
        &self,
        natural_clusters: &[Cluster],
        adjusted_clusters: &[Cluster],
        max_width: f32,
        config: &LineBreakerConfig,
    ) -> LineSolution {
        let adjusted_clusters = aligned_adjusted_clusters(natural_clusters, adjusted_clusters);
        let adjusted_clusters = adjusted_clusters.as_ref();
        if adjusted_clusters.is_empty() {
            return LineSolution::new(Vec::new());
        }
        let candidate_window = self.candidate_window.max(0);
        if self.candidate_window < 0 {
            log::warn!("paragraph DP candidate window is negative; using zero window");
        }
        let context = DpContext::new(
            natural_clusters,
            adjusted_clusters,
            max_width,
            config,
            candidate_window,
        );
        let mut committed = Vec::new();
        let mut sorted_breaks: Vec<_> = config.hard_break_after_clusters.iter().copied().collect();
        sorted_breaks.sort();
        let mut break_cursor = 0usize;
        let mut segment_start = 0i32;
        while segment_start < adjusted_clusters.len() as i32 {
            while break_cursor < sorted_breaks.len() && sorted_breaks[break_cursor] < segment_start
            {
                break_cursor += 1;
            }
            let mandatory_end = sorted_breaks.get(break_cursor).copied();
            let segment_end_exclusive =
                mandatory_end.map_or(adjusted_clusters.len() as i32, |end| end + 1);
            let ends = self.solve_segment(
                &context,
                segment_start,
                segment_end_exclusive,
                mandatory_end.is_some(),
            );
            self.commit_segment(
                &mut committed,
                &ends,
                segment_start,
                mandatory_end,
                &context,
                &config.hard_break_after_clusters,
            );
            segment_start = segment_end_exclusive;
        }
        apply_kinsoku_repairs(
            &committed,
            natural_clusters,
            adjusted_clusters,
            max_width,
            self.kinsoku.as_ref(),
            &config.shrink_opportunities,
            self.push_in_penalty,
            self.carry_previous_penalty,
            self.leave_ragged_penalty,
            &config.unbreakable_ranges,
            config.first_line_indent,
            &config.hangable_clusters,
            &config.extendable_hang_ranges,
            5,
            config.forbidden_line_start_clusters.as_ref(),
        ).resolve_white_space()
    }
}
