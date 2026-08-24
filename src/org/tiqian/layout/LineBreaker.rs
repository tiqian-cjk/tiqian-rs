// 对应 Kotlin 源文件：engine/src/commonMain/kotlin/org/tiqian/layout/LineBreaker.kt

use std::collections::{HashMap, HashSet};

use super::super::core::IntRange::IntRange;
use super::super::core::LayoutModel::{Cluster, LineEndReason};
use super::KinsokuRule::{ClreqKinsokuRule, KinsokuRule};
pub use super::LineOptimization::LineCandidate;
use super::LineOptimization::{LineSolution, RepairCandidate, RepairOption};
use super::LineRepair::{apply_kinsoku_repairs, with_fill_push_in};
use super::ProgressiveBreakDecisions::{
    ProgressiveBreakOpportunity, ShrinkOpportunity, adjust_break_for_unbreakables,
    decide_hyphen_break, decide_progressive_break, line_limit, progressive_candidate_allowed,
};

/// Kotlin `LineBreaker` 的 Rust trait；默认参数由 `LineBreakerConfig` 映射。
pub trait LineBreaker {
    fn strategy_name(&self) -> &'static str {
        "custom"
    }

    fn break_lines(
        &self,
        natural_clusters: &[Cluster],
        adjusted_clusters: &[Cluster],
        max_width: f32,
        config: &LineBreakerConfig,
    ) -> LineSolution;
}

/// `LineBreaker.breakLines` 的全部策略参数，严格保留 Kotlin 默认值。
#[derive(Clone, Debug)]
pub struct LineBreakerConfig {
    pub shrink_opportunities: Vec<ShrinkOpportunity>,
    pub unbreakable_ranges: Vec<IntRange>,
    pub first_line_indent: f32,
    pub hangable_clusters: HashSet<i32>,
    pub extendable_hang_ranges: Vec<IntRange>,
    pub forbidden_line_start_clusters: Option<HashSet<i32>>,
    pub forbidden_line_end_clusters: HashSet<i32>,
    pub hyphen_break_clusters: HashSet<i32>,
    pub cjk_inter_char_boundaries: HashSet<i32>,
    pub max_cjk_stretch_per_gap: f32,
    pub sino_western_boundaries: HashSet<i32>,
    pub sino_western_stretch_cap: f32,
    pub line_adjustment_push_in: bool,
    pub line_adjustment_compress_bias: f32,
    pub hard_break_after_clusters: HashSet<i32>,
    pub non_rendering_control_clusters: HashSet<i32>,
    pub progressive_break_opportunities: HashMap<i32, ProgressiveBreakOpportunity>,
}

impl Default for LineBreakerConfig {
    fn default() -> Self {
        Self {
            shrink_opportunities: Vec::new(),
            unbreakable_ranges: Vec::new(),
            first_line_indent: 0.0,
            hangable_clusters: HashSet::new(),
            extendable_hang_ranges: Vec::new(),
            forbidden_line_start_clusters: None,
            forbidden_line_end_clusters: HashSet::new(),
            hyphen_break_clusters: HashSet::new(),
            cjk_inter_char_boundaries: HashSet::new(),
            max_cjk_stretch_per_gap: f32::INFINITY,
            sino_western_boundaries: HashSet::new(),
            sino_western_stretch_cap: 0.0,
            line_adjustment_push_in: false,
            line_adjustment_compress_bias: 1.0,
            hard_break_after_clusters: HashSet::new(),
            non_rendering_control_clusters: HashSet::new(),
            progressive_break_opportunities: HashMap::new(),
        }
    }
}

/// `GreedyLineBreaker`：先贪心填行，再应用避头尾 repair 与可选 fill PushIn。
pub struct GreedyLineBreaker {
    pub kinsoku: Box<dyn KinsokuRule>,
    pub push_in_penalty: i32,
    pub carry_previous_penalty: i32,
    pub leave_ragged_penalty: i32,
}

impl Default for GreedyLineBreaker {
    fn default() -> Self {
        Self::new(Box::new(ClreqKinsokuRule::default()), 2, 10, 20)
    }
}

impl GreedyLineBreaker {
    pub fn new(
        kinsoku: Box<dyn KinsokuRule>,
        push_in_penalty: i32,
        carry_previous_penalty: i32,
        leave_ragged_penalty: i32,
    ) -> Self {
        Self {
            kinsoku,
            push_in_penalty,
            carry_previous_penalty,
            leave_ragged_penalty,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn greedy_fill(
        &self,
        natural_clusters: &[Cluster],
        adjusted_clusters: &[Cluster],
        max_width: f32,
        config: &LineBreakerConfig,
    ) -> Vec<LineCandidate> {
        let mut lines = Vec::new();
        let mut line_start = 0i32;
        let mut adjusted_accum = 0.0;
        let mut has_rendering_content = false;
        let mut index = 0i32;

        while index < adjusted_clusters.len() as i32 {
            let next_adjusted = adjusted_accum + adjusted_clusters[index as usize].advance;
            let overflows = next_adjusted
                > line_limit(max_width, config.first_line_indent, line_start)
                && has_rendering_content;
            if overflows {
                let limit = line_limit(max_width, config.first_line_indent, line_start);
                let progressive = decide_progressive_break(
                    line_start,
                    index,
                    &config.progressive_break_opportunities,
                    Some(adjusted_clusters),
                    limit,
                    &config.cjk_inter_char_boundaries,
                    config.max_cjk_stretch_per_gap,
                    &config.sino_western_boundaries,
                    config.sino_western_stretch_cap,
                );
                let decided = decide_hyphen_break(
                    line_start,
                    progressive,
                    adjusted_clusters,
                    limit,
                    &config.hyphen_break_clusters,
                    &config.cjk_inter_char_boundaries,
                    config.max_cjk_stretch_per_gap,
                    &config.sino_western_boundaries,
                    config.sino_western_stretch_cap,
                );
                let after_unbreak = adjust_break_for_unbreakables(
                    decided,
                    line_start,
                    &config
                        .unbreakable_ranges
                        .iter()
                        .map(|range| (range.first(), range.last()))
                        .collect::<Vec<_>>(),
                );
                let break_at = adjust_break_for_line_end(
                    after_unbreak,
                    line_start,
                    &config.forbidden_line_end_clusters,
                );
                lines.push(close_filled_line(
                    IntRange::new(line_start, break_at - 1),
                    after_unbreak,
                    natural_clusters,
                    adjusted_clusters,
                ));
                line_start = break_at;
                adjusted_accum = adjusted_clusters[break_at as usize].advance;
                has_rendering_content = !config.non_rendering_control_clusters.contains(&break_at);
                index = break_at + 1;
            } else {
                adjusted_accum = next_adjusted;
                if !config.non_rendering_control_clusters.contains(&index) {
                    has_rendering_content = true;
                }
                if config.hard_break_after_clusters.contains(&index) {
                    lines.push(rebuild_line(
                        IntRange::new(line_start, index),
                        natural_clusters,
                        adjusted_clusters,
                        LineEndReason::MandatoryBreak,
                        None,
                        Vec::new(),
                    ));
                    line_start = index + 1;
                    adjusted_accum = 0.0;
                    has_rendering_content = false;
                }
                index += 1;
            }
        }

        if line_start < adjusted_clusters.len() as i32 {
            lines.push(rebuild_line(
                IntRange::new(line_start, adjusted_clusters.len() as i32 - 1),
                natural_clusters,
                adjusted_clusters,
                LineEndReason::ParagraphEnd,
                None,
                Vec::new(),
            ));
        } else if config
            .hard_break_after_clusters
            .contains(&(adjusted_clusters.len() as i32 - 1))
        {
            lines.push(empty_line_candidate(
                adjusted_clusters
                    .last()
                    .expect("non-empty clusters have last cluster")
                    .range
                    .end(),
                LineEndReason::ParagraphEnd,
            ));
        }
        lines
    }
}

impl LineBreaker for GreedyLineBreaker {
    fn strategy_name(&self) -> &'static str {
        "greedy"
    }

    fn break_lines(
        &self,
        natural_clusters: &[Cluster],
        adjusted_clusters: &[Cluster],
        max_width: f32,
        config: &LineBreakerConfig,
    ) -> LineSolution {
        if adjusted_clusters.is_empty() {
            return LineSolution::new(Vec::new());
        }
        assert_eq!(
            natural_clusters.len(),
            adjusted_clusters.len(),
            "naturalClusters and adjustedClusters must align cluster-for-cluster."
        );
        let greedy = self.greedy_fill(natural_clusters, adjusted_clusters, max_width, config);
        let repaired = apply_kinsoku_repairs(
            &greedy,
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
        );
        let mut gap_boundaries = config.cjk_inter_char_boundaries.clone();
        gap_boundaries.extend(&config.sino_western_boundaries);
        with_fill_push_in(
            repaired,
            config.line_adjustment_push_in,
            natural_clusters,
            adjusted_clusters,
            max_width,
            &config.shrink_opportunities,
            config.first_line_indent,
            config.line_adjustment_compress_bias,
            config.forbidden_line_start_clusters.as_ref(),
            &config.forbidden_line_end_clusters,
            &config.unbreakable_ranges,
            self.push_in_penalty,
            &gap_boundaries,
            &config.progressive_break_opportunities,
        )
    }
}

/// 将 line-end forbidden mark 留给下一行，并保留 Kotlin `CarryNext` structured repair。
pub fn close_filled_line(
    range: IntRange,
    natural_break_at: i32,
    natural_clusters: &[Cluster],
    adjusted_clusters: &[Cluster],
) -> LineCandidate {
    let mut line = rebuild_line(
        range,
        natural_clusters,
        adjusted_clusters,
        LineEndReason::AutoWrap,
        None,
        Vec::new(),
    );
    if range.last() + 1 == natural_break_at {
        return line;
    }
    let moved = range.last() + 1;
    line.repair = Some(RepairOption::CarryNext {
        penalty: 0,
        reason: format!(
            "ForbiddenAtLineEnd:{}:moved-to-next-line",
            adjusted_clusters[moved as usize].text
        ),
        moved_cluster_index: moved,
    });
    line
}

pub fn ends_with_synthetic_hyphen(
    line: &LineCandidate,
    hyphen_break_clusters: &HashSet<i32>,
) -> bool {
    line.end_reason == LineEndReason::AutoWrap
        && !line.cluster_range.is_empty()
        && hyphen_break_clusters.contains(&(line.cluster_range.last() + 1))
}

pub fn ends_with_progressive_break(
    line: &LineCandidate,
    opportunities: &HashMap<i32, ProgressiveBreakOpportunity>,
) -> bool {
    line.end_reason == LineEndReason::AutoWrap
        && !line.cluster_range.is_empty()
        && opportunities.contains_key(&(line.cluster_range.last() + 1))
}

pub fn line_gap_count(range: IntRange, gap_boundaries: &HashSet<i32>) -> i32 {
    if range.is_empty() {
        return 0;
    }
    (range.first()..range.last())
        .filter(|index| gap_boundaries.contains(index))
        .count() as i32
}

pub fn line_adjustment_density(
    line: &LineCandidate,
    limit: f32,
    is_last: bool,
    gap_boundaries: &HashSet<i32>,
) -> f32 {
    if is_last || line.end_reason != LineEndReason::AutoWrap {
        return 0.0;
    }
    let gaps = line_gap_count(line.in_measure_cluster_range(), gap_boundaries);
    if gaps == 0 {
        return 0.0;
    }
    (limit - line.adjusted_width).max(0.0) / gaps as f32
}

pub fn amortized_adjustment_cost(
    density: f32,
    previous_density: f32,
    reference_density: f32,
) -> f32 {
    let reference = reference_density.max(1.0);
    let difference = density - previous_density;
    (density * density + difference * difference) / reference
}

pub fn find_greedy_end(
    adjusted_clusters: &[Cluster],
    start: i32,
    max_width: f32,
    end_exclusive: i32,
    non_rendering_control_clusters: &HashSet<i32>,
) -> i32 {
    let mut accumulated = 0.0;
    let mut index = start;
    let mut has_rendering_content = false;
    while index < end_exclusive {
        let next = accumulated + adjusted_clusters[index as usize].advance;
        if next > max_width && has_rendering_content {
            return index;
        }
        accumulated = next;
        if !non_rendering_control_clusters.contains(&index) {
            has_rendering_content = true;
        }
        index += 1;
    }
    end_exclusive
}

pub fn adjust_break_for_line_end(
    break_at: i32,
    line_start: i32,
    forbidden_line_end_clusters: &HashSet<i32>,
) -> i32 {
    let mut boundary = break_at;
    while boundary - 1 > line_start && forbidden_line_end_clusters.contains(&(boundary - 1)) {
        boundary -= 1;
    }
    boundary
}

pub fn rebuild_line(
    range: IntRange,
    natural: &[Cluster],
    adjusted: &[Cluster],
    end_reason: LineEndReason,
    repair: Option<RepairOption>,
    repair_candidates: Vec<RepairCandidate>,
) -> LineCandidate {
    assert!(
        !range.is_empty(),
        "Use emptyLineCandidate for an empty line."
    );
    let candidate = LineCandidate {
        cluster_range: range,
        source_range: super::super::core::Geometry::TextRange::new(
            adjusted[range.first() as usize].range.start(),
            adjusted[range.last() as usize].range.end(),
        ),
        natural_width: range
            .into_iter()
            .map(|index| natural[index as usize].advance)
            .sum(),
        adjusted_width: range
            .into_iter()
            .map(|index| adjusted[index as usize].advance)
            .sum(),
        end_reason,
        repair,
        repair_candidates,
        hanging_cluster_indices: HashSet::new(),
    };
    candidate.validate_hanging_suffix();
    candidate
}

pub fn empty_line_candidate(source_offset: i32, end_reason: LineEndReason) -> LineCandidate {
    let mut line = LineCandidate::new(
        IntRange::EMPTY,
        super::super::core::Geometry::TextRange::new(source_offset, source_offset),
        0.0,
        0.0,
    );
    line.end_reason = end_reason;
    line
}

/// `LookaheadLineBreaker`：对 greedy break 左侧 window 内候选进行有限未来行评分。
pub struct LookaheadLineBreaker {
    pub window: i32,
    pub future_line_horizon: i32,
    pub raggedness_weight: f32,
    pub kinsoku: Box<dyn KinsokuRule>,
    pub push_in_penalty: i32,
    pub carry_previous_penalty: i32,
    pub leave_ragged_penalty: i32,
    pub consecutive_synthetic_hyphen_penalty: f32,
}

impl Default for LookaheadLineBreaker {
    fn default() -> Self {
        Self::new(
            Box::new(ClreqKinsokuRule::default()),
            2,
            2,
            0.5,
            2,
            10,
            20,
            12.0,
        )
    }
}

impl LookaheadLineBreaker {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        kinsoku: Box<dyn KinsokuRule>,
        window: i32,
        future_line_horizon: i32,
        raggedness_weight: f32,
        push_in_penalty: i32,
        carry_previous_penalty: i32,
        leave_ragged_penalty: i32,
        consecutive_synthetic_hyphen_penalty: f32,
    ) -> Self {
        Self {
            window,
            future_line_horizon,
            raggedness_weight,
            kinsoku,
            push_in_penalty,
            carry_previous_penalty,
            leave_ragged_penalty,
            consecutive_synthetic_hyphen_penalty,
        }
    }

    fn repair(
        &self,
        initial: &[LineCandidate],
        natural: &[Cluster],
        adjusted: &[Cluster],
        max_width: f32,
        config: &LineBreakerConfig,
    ) -> LineSolution {
        apply_kinsoku_repairs(
            initial,
            natural,
            adjusted,
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
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn raw_greedy_lines_from(
        &self,
        start: i32,
        natural: &[Cluster],
        adjusted: &[Cluster],
        max_width: f32,
        end_exclusive: i32,
        max_lines: i32,
        config: &LineBreakerConfig,
    ) -> Vec<LineCandidate> {
        if start >= end_exclusive {
            return Vec::new();
        }
        assert!(max_lines > 0, "maxLines must be positive");
        let unbreakable: Vec<_> = config
            .unbreakable_ranges
            .iter()
            .map(|range| (range.first(), range.last()))
            .collect();
        let mut lines = Vec::new();
        let mut line_start = start;
        let mut accumulated = 0.0;
        let mut has_rendering_content = false;
        let mut index = start;
        while index < end_exclusive {
            let next = accumulated + adjusted[index as usize].advance;
            if next > max_width && has_rendering_content {
                let progressive = decide_progressive_break(
                    line_start,
                    index,
                    &config.progressive_break_opportunities,
                    Some(adjusted),
                    max_width,
                    &config.cjk_inter_char_boundaries,
                    config.max_cjk_stretch_per_gap,
                    &config.sino_western_boundaries,
                    config.sino_western_stretch_cap,
                );
                let hyphen = decide_hyphen_break(
                    line_start,
                    progressive,
                    adjusted,
                    max_width,
                    &config.hyphen_break_clusters,
                    &config.cjk_inter_char_boundaries,
                    config.max_cjk_stretch_per_gap,
                    &config.sino_western_boundaries,
                    config.sino_western_stretch_cap,
                );
                let break_at = adjust_break_for_unbreakables(hyphen, line_start, &unbreakable);
                lines.push(rebuild_line(
                    IntRange::new(line_start, break_at - 1),
                    natural,
                    adjusted,
                    LineEndReason::AutoWrap,
                    None,
                    Vec::new(),
                ));
                if lines.len() as i32 >= max_lines {
                    return lines;
                }
                line_start = break_at;
                accumulated = adjusted[break_at as usize].advance;
                has_rendering_content = !config.non_rendering_control_clusters.contains(&break_at);
                index = break_at + 1;
            } else {
                accumulated = next;
                if !config.non_rendering_control_clusters.contains(&index) {
                    has_rendering_content = true;
                }
                index += 1;
            }
        }
        lines.push(rebuild_line(
            IntRange::new(line_start, end_exclusive - 1),
            natural,
            adjusted,
            LineEndReason::ParagraphEnd,
            None,
            Vec::new(),
        ));
        lines
    }

    #[allow(clippy::too_many_arguments)]
    fn score_candidate(
        &self,
        start: i32,
        end: i32,
        natural: &[Cluster],
        adjusted: &[Cluster],
        max_width: f32,
        segment_end_exclusive: i32,
        previous_density: f32,
        previous_synthetic_hyphen_run: i32,
        gap_boundaries: &HashSet<i32>,
        reference_density: f32,
        config: &LineBreakerConfig,
    ) -> f32 {
        let first_line = rebuild_line(
            IntRange::new(start, end - 1),
            natural,
            adjusted,
            LineEndReason::AutoWrap,
            None,
            Vec::new(),
        );
        let future = self.raw_greedy_lines_from(
            end,
            natural,
            adjusted,
            max_width,
            segment_end_exclusive,
            self.future_line_horizon + 1,
            config,
        );
        let mut splice = vec![first_line];
        splice.extend(future);
        let spliced = self
            .repair(&splice, natural, adjusted, max_width, config)
            .lines;
        let horizon = (1 + self.future_line_horizon).min(spliced.len() as i32);
        let mut score = 0.0;
        let mut previous = previous_density;
        let mut synthetic_hyphen_run = previous_synthetic_hyphen_run;
        for index in 0..horizon as usize {
            let line = &spliced[index];
            let is_last = index == spliced.len() - 1;
            score += self.badness(
                line,
                max_width,
                is_last,
                previous,
                gap_boundaries,
                reference_density,
                config,
            );
            if ends_with_synthetic_hyphen(line, &config.hyphen_break_clusters) {
                score += self.consecutive_synthetic_hyphen_penalty * synthetic_hyphen_run as f32;
                synthetic_hyphen_run += 1;
            } else {
                synthetic_hyphen_run = 0;
            }
            let limit = line_limit(
                max_width,
                config.first_line_indent,
                line.cluster_range.first(),
            );
            previous = line_adjustment_density(line, limit, is_last, gap_boundaries);
        }
        score
    }

    fn badness(
        &self,
        line: &LineCandidate,
        max_width: f32,
        is_last: bool,
        previous_density: f32,
        gap_boundaries: &HashSet<i32>,
        reference_density: f32,
        config: &LineBreakerConfig,
    ) -> f32 {
        let limit = line_limit(
            max_width,
            config.first_line_indent,
            line.cluster_range.first(),
        );
        let ragged = if is_last {
            0.0
        } else {
            (limit - line.adjusted_width).max(0.0)
        };
        let in_measure = line.in_measure_cluster_range();
        let gaps = line_gap_count(in_measure, gap_boundaries);
        let residual = if gaps == 0 { ragged } else { 0.0 };
        let density = line_adjustment_density(line, limit, is_last, gap_boundaries);
        let orphan =
            if !is_last && !in_measure.is_empty() && in_measure.first() == in_measure.last() {
                self.leave_ragged_penalty as f32
            } else {
                0.0
            };
        residual * self.raggedness_weight
            + orphan
            + amortized_adjustment_cost(density, previous_density, reference_density)
                * self.raggedness_weight
            + line.repair.as_ref().map_or(0, RepairOption::penalty) as f32
    }
}

impl LineBreaker for LookaheadLineBreaker {
    fn strategy_name(&self) -> &'static str {
        "lookahead"
    }

    fn break_lines(
        &self,
        natural: &[Cluster],
        adjusted: &[Cluster],
        max_width: f32,
        config: &LineBreakerConfig,
    ) -> LineSolution {
        if adjusted.is_empty() {
            return LineSolution::new(Vec::new());
        }
        assert_eq!(
            natural.len(),
            adjusted.len(),
            "naturalClusters and adjustedClusters must align cluster-for-cluster."
        );
        assert!(self.window >= 0, "window must be non-negative.");
        assert!(
            self.future_line_horizon >= 0,
            "futureLineHorizon must be non-negative."
        );
        let mut committed = Vec::new();
        let mut line_start = 0i32;
        let mut gap_boundaries = config.cjk_inter_char_boundaries.clone();
        gap_boundaries.extend(&config.sino_western_boundaries);
        let reference_density = config.max_cjk_stretch_per_gap;
        let mut committed_density = 0.0;
        let mut committed_synthetic_hyphen_run = 0;
        let mut sorted_breaks: Vec<_> = config.hard_break_after_clusters.iter().copied().collect();
        sorted_breaks.sort();
        let mut break_cursor = 0usize;
        let unbreakable: Vec<_> = config
            .unbreakable_ranges
            .iter()
            .map(|range| (range.first(), range.last()))
            .collect();

        while line_start < adjusted.len() as i32 {
            while break_cursor < sorted_breaks.len() && sorted_breaks[break_cursor] < line_start {
                break_cursor += 1;
            }
            let mandatory_end = sorted_breaks.get(break_cursor).copied();
            let segment_end_exclusive = mandatory_end.map_or(adjusted.len() as i32, |end| end + 1);
            let limit = line_limit(max_width, config.first_line_indent, line_start);
            let raw_greedy_end = find_greedy_end(
                adjusted,
                line_start,
                limit,
                segment_end_exclusive,
                &config.non_rendering_control_clusters,
            );
            let progressive = decide_progressive_break(
                line_start,
                raw_greedy_end,
                &config.progressive_break_opportunities,
                Some(adjusted),
                limit,
                &config.cjk_inter_char_boundaries,
                config.max_cjk_stretch_per_gap,
                &config.sino_western_boundaries,
                config.sino_western_stretch_cap,
            );
            let hyphen = decide_hyphen_break(
                line_start,
                progressive,
                adjusted,
                limit,
                &config.hyphen_break_clusters,
                &config.cjk_inter_char_boundaries,
                config.max_cjk_stretch_per_gap,
                &config.sino_western_boundaries,
                config.sino_western_stretch_cap,
            );
            let greedy_end = adjust_break_for_unbreakables(hyphen, line_start, &unbreakable);
            if greedy_end >= segment_end_exclusive {
                if let Some(mandatory_end) = mandatory_end {
                    committed.push(rebuild_line(
                        IntRange::new(line_start, mandatory_end),
                        natural,
                        adjusted,
                        LineEndReason::MandatoryBreak,
                        None,
                        Vec::new(),
                    ));
                    committed_density = 0.0;
                    committed_synthetic_hyphen_run = 0;
                    line_start = mandatory_end + 1;
                    if line_start == adjusted.len() as i32 {
                        committed.push(empty_line_candidate(
                            adjusted
                                .last()
                                .expect("non-empty clusters have last cluster")
                                .range
                                .end(),
                            LineEndReason::ParagraphEnd,
                        ));
                    }
                    continue;
                }
                committed.push(rebuild_line(
                    IntRange::new(line_start, adjusted.len() as i32 - 1),
                    natural,
                    adjusted,
                    LineEndReason::ParagraphEnd,
                    None,
                    Vec::new(),
                ));
                break;
            }
            let mut candidates: Vec<_> = ((greedy_end - self.window)..=greedy_end)
                .filter(|end| {
                    *end > line_start
                        && *end <= adjusted.len() as i32
                        && *end <= segment_end_exclusive
                })
                .filter(|end| {
                    !config
                        .unbreakable_ranges
                        .iter()
                        .any(|range| *end > range.first() && *end <= range.last())
                })
                .filter(|end| {
                    progressive_candidate_allowed(
                        line_start,
                        raw_greedy_end,
                        *end,
                        &config.progressive_break_opportunities,
                        Some(adjusted),
                        limit,
                        &config.cjk_inter_char_boundaries,
                        config.max_cjk_stretch_per_gap,
                        &config.sino_western_boundaries,
                        config.sino_western_stretch_cap,
                    )
                })
                .filter(|end| {
                    (line_start..*end)
                        .any(|index| !config.non_rendering_control_clusters.contains(&index))
                        || *end == segment_end_exclusive
                })
                .collect();
            candidates.sort();
            candidates.dedup();
            if candidates.is_empty() {
                candidates.push(adjust_break_for_unbreakables(
                    greedy_end,
                    line_start,
                    &unbreakable,
                ));
            }
            let mut best_end = greedy_end;
            let mut best_score = f32::INFINITY;
            for end in candidates {
                let score = self.score_candidate(
                    line_start,
                    end,
                    natural,
                    adjusted,
                    max_width,
                    segment_end_exclusive,
                    committed_density,
                    committed_synthetic_hyphen_run,
                    &gap_boundaries,
                    reference_density,
                    config,
                );
                if score < best_score {
                    best_score = score;
                    best_end = end;
                }
            }
            let committed_end = adjust_break_for_line_end(
                best_end,
                line_start,
                &config.forbidden_line_end_clusters,
            );
            if config.hard_break_after_clusters.contains(&committed_end)
                && line_start < committed_end
            {
                committed.push(rebuild_line(
                    IntRange::new(line_start, committed_end),
                    natural,
                    adjusted,
                    LineEndReason::MandatoryBreak,
                    None,
                    Vec::new(),
                ));
                committed_density = 0.0;
                committed_synthetic_hyphen_run = 0;
                line_start = committed_end + 1;
                if line_start == adjusted.len() as i32 {
                    committed.push(empty_line_candidate(
                        adjusted
                            .last()
                            .expect("non-empty clusters have last cluster")
                            .range
                            .end(),
                        LineEndReason::ParagraphEnd,
                    ));
                }
                continue;
            }
            committed.push(close_filled_line(
                IntRange::new(line_start, committed_end - 1),
                best_end,
                natural,
                adjusted,
            ));
            let line = committed.last().expect("committed line was pushed");
            let line_limit_value = line_limit(
                max_width,
                config.first_line_indent,
                line.cluster_range.first(),
            );
            committed_density =
                line_adjustment_density(line, line_limit_value, false, &gap_boundaries);
            committed_synthetic_hyphen_run =
                if ends_with_synthetic_hyphen(line, &config.hyphen_break_clusters) {
                    committed_synthetic_hyphen_run + 1
                } else {
                    0
                };
            line_start = committed_end;
        }
        let repaired = self.repair(&committed, natural, adjusted, max_width, config);
        with_fill_push_in(
            repaired,
            config.line_adjustment_push_in,
            natural,
            adjusted,
            max_width,
            &config.shrink_opportunities,
            config.first_line_indent,
            config.line_adjustment_compress_bias,
            config.forbidden_line_start_clusters.as_ref(),
            &config.forbidden_line_end_clusters,
            &config.unbreakable_ranges,
            self.push_in_penalty,
            &gap_boundaries,
            &config.progressive_break_opportunities,
        )
    }
}
