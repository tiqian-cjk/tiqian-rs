// 对应 Kotlin 源文件：engine/src/commonMain/kotlin/org/tiqian/layout/ProgressiveBreakDecisions.kt

use crate::common::{HashMap, HashSet};

use super::super::core::Geometry::TextRange;
use super::super::core::LayoutModel::Cluster;

/// 一个 progressive technical span 内的有序 fallback tier。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProgressiveBreakTier {
    Whitespace,
    Structural,
    Syllable,
    WholeToken,
    Emergency,
}
impl ProgressiveBreakTier {
    pub fn priority(self) -> i32 {
        match self {
            Self::Whitespace => 0,
            Self::Structural => 1,
            Self::Syllable => 2,
            Self::WholeToken => 3,
            Self::Emergency => 4,
        }
    }
}

/// 一个 [`LineBreakSpan`](super::super::core::TextModel::LineBreakSpan) 暴露的 cluster boundary。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProgressiveBreakOpportunity {
    pub tier: ProgressiveBreakTier,
    pub span_range: TextRange,
    /// 此 boundary 之前 source whitespace 所拥有的有界正 glue。
    pub preceding_whitespace_stretch_capacity: f32,
}
impl ProgressiveBreakOpportunity {
    pub fn new(tier: ProgressiveBreakTier, span_range: TextRange) -> Self {
        Self {
            tier,
            span_range,
            preceding_whitespace_stretch_capacity: 0.0,
        }
    }
    pub fn with_preceding_whitespace_stretch_capacity(
        tier: ProgressiveBreakTier,
        span_range: TextRange,
        preceding_whitespace_stretch_capacity: f32,
    ) -> Self {
        Self {
            tier,
            span_range,
            preceding_whitespace_stretch_capacity,
        }
    }
}

/** `ProgressiveTechnicalBreakSelection`：overflow 位于一个技术 span 内时，选择最佳可用 tier 中最靠右的 fitting boundary。 */
pub fn decide_progressive_break(
    line_start: i32,
    overflow_at: i32,
    opportunities: &HashMap<i32, ProgressiveBreakOpportunity>,
    adjusted_clusters: Option<&[Cluster]>,
    line_limit: f32,
    cjk_inter_char_boundaries: &HashSet<i32>,
    max_cjk_stretch_per_gap: f32,
    sino_western_boundaries: &HashSet<i32>,
    sino_western_stretch_cap: f32,
) -> i32 {
    let Some(active) = opportunities.get(&overflow_at) else {
        return overflow_at;
    };
    let best_priority = progressive_break_priority_for_line(
        line_start,
        overflow_at,
        *active,
        opportunities,
        adjusted_clusters,
        line_limit,
        cjk_inter_char_boundaries,
        max_cjk_stretch_per_gap,
        sino_western_boundaries,
        sino_western_stretch_cap,
    );
    ((line_start + 1)..=overflow_at)
        .rfind(|boundary| {
            opportunities.get(boundary).is_some_and(|opportunity| {
                opportunity.span_range == active.span_range
                    && opportunity.tier.priority() == best_priority
            })
        })
        .unwrap_or(overflow_at)
}

pub const PROGRESSIVE_TECHNICAL_VISIBLE_STRETCH_FRACTION: f32 = 0.0;

pub fn progressive_candidate_allowed(
    line_start: i32,
    raw_greedy: i32,
    candidate_end: i32,
    opportunities: &HashMap<i32, ProgressiveBreakOpportunity>,
    adjusted_clusters: Option<&[Cluster]>,
    line_limit: f32,
    cjk_inter_char_boundaries: &HashSet<i32>,
    max_cjk_stretch_per_gap: f32,
    sino_western_boundaries: &HashSet<i32>,
    sino_western_stretch_cap: f32,
) -> bool {
    let Some(active) = opportunities.get(&raw_greedy) else {
        return true;
    };
    let Some(candidate) = opportunities.get(&candidate_end) else {
        let Some(clusters) = adjusted_clusters else {
            return true;
        };
        let Some(cluster) = clusters.get(candidate_end as usize) else {
            return true;
        };
        return cluster.range.start() <= active.span_range.start()
            || cluster.range.start() >= active.span_range.end();
    };
    if candidate.span_range != active.span_range {
        return true;
    }
    if candidate_end > raw_greedy {
        return candidate.tier.priority() <= active.tier.priority();
    }
    // `ProgressiveTechnicalRightmostTierBoundary`：一旦在线内断开技术 span，lookahead/DP 必须重放
    // tier policy 选中的唯一 boundary，不能为平滑下一行而选择同 tier 的更早 boundary。
    candidate_end
        == decide_progressive_break(
            line_start,
            raw_greedy,
            opportunities,
            adjusted_clusters,
            line_limit,
            cjk_inter_char_boundaries,
            max_cjk_stretch_per_gap,
            sino_western_boundaries,
            sino_western_stretch_cap,
        )
}

fn progressive_break_priority_for_line(
    line_start: i32,
    overflow_at: i32,
    active: ProgressiveBreakOpportunity,
    opportunities: &HashMap<i32, ProgressiveBreakOpportunity>,
    adjusted_clusters: Option<&[Cluster]>,
    line_limit: f32,
    cjk_inter_char_boundaries: &HashSet<i32>,
    max_cjk_stretch_per_gap: f32,
    sino_western_boundaries: &HashSet<i32>,
    sino_western_stretch_cap: f32,
) -> i32 {
    let mut priorities: Vec<i32> = ((line_start + 1)..=overflow_at)
        .filter_map(|index| opportunities.get(&index))
        .filter(|opportunity| opportunity.span_range == active.span_range)
        .map(|opportunity| opportunity.tier.priority())
        .collect();
    priorities.sort_unstable();
    priorities.dedup();
    if priorities.is_empty() {
        return active.tier.priority();
    }
    let Some(clusters) = adjusted_clusters else {
        return priorities[0];
    };
    if !line_limit.is_finite() || !max_cjk_stretch_per_gap.is_finite() {
        return priorities[0];
    }
    let progressive_stretch_limit =
        max_cjk_stretch_per_gap * PROGRESSIVE_TECHNICAL_VISIBLE_STRETCH_FRACTION;
    let mut least_loose_priority = priorities[0];
    let mut least_loose_density = f32::INFINITY;
    let mut least_loose_boundary = line_start + 1;
    for priority in &priorities {
        let Some(boundary) = ((line_start + 1)..=overflow_at).rfind(|candidate| {
            opportunities.get(candidate).is_some_and(|opportunity| {
                opportunity.span_range == active.span_range
                    && opportunity.tier.priority() == *priority
            })
        }) else {
            continue;
        };
        let density = progressive_candidate_stretch_density(
            line_start,
            boundary,
            opportunities,
            clusters,
            line_limit,
            cjk_inter_char_boundaries,
            sino_western_boundaries,
            sino_western_stretch_cap,
        );
        if density < least_loose_density {
            least_loose_density = density;
            least_loose_priority = *priority;
            least_loose_boundary = boundary;
        }
        if density <= progressive_stretch_limit {
            return *priority;
        }
    }
    let emergency_boundary = ((line_start + 1)..=overflow_at).rfind(|candidate| {
        opportunities.get(candidate).is_some_and(|opportunity| {
            opportunity.span_range == active.span_range
                && opportunity.tier == ProgressiveBreakTier::Emergency
        })
    });
    if emergency_boundary.is_some_and(|boundary| boundary >= least_loose_boundary) {
        ProgressiveBreakTier::Emergency.priority()
    } else {
        least_loose_priority
    }
}

fn progressive_candidate_stretch_density(
    line_start: i32,
    boundary: i32,
    opportunities: &HashMap<i32, ProgressiveBreakOpportunity>,
    adjusted_clusters: &[Cluster],
    line_limit: f32,
    cjk_inter_char_boundaries: &HashSet<i32>,
    sino_western_boundaries: &HashSet<i32>,
    sino_western_stretch_cap: f32,
) -> f32 {
    let width: f32 = (line_start..boundary)
        .map(|index| adjusted_clusters[index as usize].advance)
        .sum();
    let deficit = (line_limit - width).max(0.0);
    // `ProgressiveTechnicalWhitespaceBreakPricing`：k boundary 处的 Whitespace opportunity 拥有真实的
    // source space cluster k-1；行若在 k 结束，该 space 作为 trailing line-edge whitespace 被折叠。
    let technical_whitespace_capacity: f32 = ((line_start + 1)..boundary)
        .filter_map(|candidate| opportunities.get(&candidate))
        .filter(|opportunity| opportunity.tier == ProgressiveBreakTier::Whitespace)
        .map(|opportunity| opportunity.preceding_whitespace_stretch_capacity)
        .sum();
    let sino_western_gap_count = ((line_start + 1)..boundary)
        .filter(|candidate| sino_western_boundaries.contains(candidate))
        .count() as f32;
    let cjk_deficit = (deficit
        - technical_whitespace_capacity
        - sino_western_gap_count * sino_western_stretch_cap)
        .max(0.0);
    let active_span = opportunities
        .get(&boundary)
        .map(|opportunity| opportunity.span_range);
    let terminal_technical_source_units = active_span.map_or(0, |span| {
        (line_start..boundary)
            .map(|index| &adjusted_clusters[index as usize])
            .filter(|cluster| {
                cluster.range.start() >= span.start()
                    && cluster.range.end() <= span.end()
                    && !cluster.text.chars().any(char::is_whitespace)
            })
            .map(|cluster| cluster.text.utf16_len())
            .sum()
    });
    // `TerminalTechnicalTrackingDensityEstimate`：技术 prefix 到达行末时，最终可用的 tracking 是其自身
    // source-unit gap，而不是无关的 CJK body gap。当前技术分段面向 Latin/ASCII，UTF-16 unit 与其 grapheme cut 一致。
    let terminal_technical_gap_count = (terminal_technical_source_units - 1).max(0);
    if terminal_technical_gap_count > 0 {
        return cjk_deficit / terminal_technical_gap_count as f32;
    }
    let cjk_gap_count = ((line_start + 1)..boundary)
        .filter(|candidate| cjk_inter_char_boundaries.contains(candidate))
        .count();
    if cjk_gap_count == 0 {
        cjk_deficit
    } else {
        cjk_deficit / cjk_gap_count as f32
    }
}

pub fn decide_hyphen_break(
    line_start: i32,
    overflow_at: i32,
    adjusted_clusters: &[Cluster],
    line_limit: f32,
    hyphen_break_clusters: &HashSet<i32>,
    cjk_inter_char_boundaries: &HashSet<i32>,
    max_cjk_stretch_per_gap: f32,
    sino_western_boundaries: &HashSet<i32>,
    sino_western_stretch_cap: f32,
) -> i32 {
    if !hyphen_break_clusters.contains(&overflow_at) {
        return overflow_at;
    }
    let mut whole_word_end = overflow_at;
    while whole_word_end > line_start && hyphen_break_clusters.contains(&whole_word_end) {
        whole_word_end -= 1;
    }
    if whole_word_end <= line_start {
        return overflow_at;
    }
    let width: f32 = (line_start..whole_word_end)
        .map(|index| adjusted_clusters[index as usize].advance)
        .sum();
    let deficit = line_limit - width;
    if deficit <= 0.0 {
        return whole_word_end;
    }
    let sino_western = ((line_start + 1)..whole_word_end)
        .filter(|index| sino_western_boundaries.contains(index))
        .count() as f32;
    let cjk_deficit = (deficit - sino_western * sino_western_stretch_cap).max(0.0);
    if cjk_deficit <= 0.0 {
        return whole_word_end;
    }
    let gaps = ((line_start + 1)..whole_word_end)
        .filter(|index| cjk_inter_char_boundaries.contains(index))
        .count();
    if gaps == 0 || cjk_deficit / gaps as f32 > max_cjk_stretch_per_gap {
        overflow_at
    } else {
        whole_word_end
    }
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

pub fn line_limit(max_width: f32, first_line_indent: f32, line_start_cluster: i32) -> f32 {
    if line_start_cluster == 0 {
        max_width - first_line_indent
    } else {
        max_width
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShrinkOpportunity {
    pub cluster_index: i32,
    pub tier: i32,
    pub capacity: f32,
    pub channel: ShrinkChannel,
    pub line_end_only: bool,
}
impl ShrinkOpportunity {
    pub fn new(cluster_index: i32, tier: i32, capacity: f32, channel: ShrinkChannel) -> Self {
        Self {
            cluster_index,
            tier,
            capacity,
            channel,
            line_end_only: false,
        }
    }
    pub fn with_line_end_only(
        cluster_index: i32,
        tier: i32,
        capacity: f32,
        channel: ShrinkChannel,
        line_end_only: bool,
    ) -> Self {
        Self {
            cluster_index,
            tier,
            capacity,
            channel,
            line_end_only,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShrinkChannel {
    TrailingGlue,
    LeadingGlue,
    LeadingAndTrailingGlue,
    RawAdvance,
}

pub fn adjust_break_for_unbreakables(
    break_at: i32,
    line_start: i32,
    unbreakable_ranges: &[(i32, i32)],
) -> i32 {
    let mut candidate = break_at;
    loop {
        let Some(&(first, _)) = unbreakable_ranges
            .iter()
            .find(|&&(first, last)| candidate > first && candidate <= last)
        else {
            return candidate;
        };
        if first <= line_start {
            return break_at;
        }
        candidate = first;
    }
}
