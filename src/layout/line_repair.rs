// 对应 Kotlin 源文件：engine/src/commonMain/kotlin/org/tiqian/layout/LineRepair.kt

use alloc::{vec, vec::Vec};
use super::super::core::geometry::TextRange;
use super::super::core::int_range::IntRange;
use super::super::core::layout_model::{Cluster, LineEndReason};
use super::kinsoku_rule::KinsokuRule;
use super::line_breaker::{line_gap_count, rebuild_line};
use super::line_optimization::{
    LineCandidate, LineSolution, PushInAllocation, RepairCandidate, RepairDetail, RepairKind,
    RepairOption, RepairReasonCode, RepairRejection,
};
use super::progressive_break_decisions::{
    ProgressiveBreakOpportunity, ShrinkChannel, ShrinkOpportunity, UnbreakableRanges, line_limit,
};
use crate::common::{HashMap, HashSet};

#[derive(Clone, Debug, PartialEq)]
pub struct PushInResult {
    pub previous: LineCandidate,
    pub current: Option<LineCandidate>,
    pub candidate: RepairCandidate,
}

#[allow(clippy::too_many_arguments)]
pub fn apply_kinsoku_repairs(
    initial: &[LineCandidate],
    natural: &[Cluster],
    adjusted: &[Cluster],
    max_width: f32,
    kinsoku: &dyn KinsokuRule,
    opportunities: &[ShrinkOpportunity],
    push_in_penalty: i32,
    carry_previous_penalty: i32,
    leave_ragged_penalty: i32,
    unbreakable: &UnbreakableRanges,
    first_line_indent: f32,
    hangable: &HashSet<i32>,
    extendable_hang: &[IntRange],
    hang_penalty: i32,
    forbidden_start: Option<&HashSet<i32>>,
) -> LineSolution {
    if initial.len() < 2 {
        return LineSolution::new(initial.to_vec());
    }
    let mut lines = initial.to_vec();
    let mut i = 1;
    while i < lines.len() {
        let curr = &lines[i];
        let first = curr.cluster_range.first();
        let previous = &lines[i - 1];
        if previous.end_reason == LineEndReason::MandatoryBreak || curr.cluster_range.is_empty() {
            i += 1;
            continue;
        }
        let first_cluster = &adjusted[first as usize];
        let forbidden = forbidden_start.map_or_else(
            || kinsoku.forbidden_at_line_start(first_cluster),
            |set| set.contains(&first),
        );
        if !forbidden {
            i += 1;
            continue;
        }
        let curr = curr.clone();
        let previous = previous.clone();
        let mut candidates = Vec::new();
        let pushed = try_push_in(
            &previous,
            &curr,
            natural,
            adjusted,
            line_limit(max_width, first_line_indent, previous.cluster_range.first()),
            opportunities,
            push_in_penalty,
            None,
            RepairReasonCode::ForbiddenAtLineStart,
        );
        candidates.push(pushed.candidate.clone());
        if pushed.candidate.accepted {
            lines[i - 1] = pushed.previous;
            if let Some(current) = pushed.current {
                lines[i] = current;
                continue;
            } else {
                lines.remove(i);
            }
            continue;
        }
        let offender = first;
        let extends = !previous.hanging_cluster_indices.is_empty()
            && previous.cluster_range.last() + 1 == offender
            && extendable_hang.iter().any(|range| {
                range.contains(offender)
                    && previous
                        .hanging_cluster_indices
                        .iter()
                        .all(|index| range.contains(*index))
            });
        if hangable.contains(&offender) && (previous.hanging_cluster_indices.is_empty() || extends)
        {
            let end = mandatory_break_tail_end(&curr, offender, adjusted);
            let end = previous.measurement.as_ref().map_or(end, |measurement| {
                measurement.next_boundary(end + 1, curr.cluster_range.last() + 1) - 1
            });
            let candidate = RepairCandidate::new(
                RepairKind::Hang,
                RepairReasonCode::ForbiddenAtLineStart,
                offender,
                hang_penalty,
                true,
            );
            candidates.push(candidate.clone());
            let range = IntRange::new(previous.cluster_range.first(), end);
            let mut hanging = previous.hanging_cluster_indices.clone();
            hanging.extend(offender..=end);
            let repaired = LineCandidate {
                cluster_range: range,
                source_range: TextRange::new(
                    adjusted[range.first() as usize].range.start(),
                    adjusted[end as usize].range.end(),
                ),
                natural_width: previous.natural_width
                    + ((previous.cluster_range.last() + 1)..=end)
                        .map(|index| natural[index as usize].advance)
                        .sum::<f32>(),
                adjusted_width: previous.adjusted_width,
                end_reason: if end == curr.cluster_range.last() {
                    curr.end_reason
                } else {
                    previous.end_reason
                },
                repair: Some(RepairOption::Hang {
                    penalty: hang_penalty,
                    reason_code: RepairReasonCode::ForbiddenAtLineStart,
                    detail: RepairDetail::Hang,
                    offender_cluster_index: offender,
                }),
                repair_candidates: [
                    previous.repair_candidates.clone(),
                    vec![pushed.candidate, candidate],
                ]
                .concat(),
                hanging_cluster_indices: hanging,
                white_space_adjustments: previous.white_space_adjustments.clone(),
                measurement: previous.measurement.clone(),
                measurement_limit: previous.measurement_limit,
            };
            let mut repaired = repaired;
            repaired.end_reason = if end == curr.cluster_range.last() {
                curr.end_reason
            } else {
                previous.end_reason
            };
            repaired.remeasure();
            if !repaired.hanging_cluster_indices.is_empty()
                && let Some(measurement) = &repaired.measurement
            {
                let in_measure = repaired.in_measure_cluster_range();
                let natural_width = measurement.measure(
                    repaired.cluster_range,
                    repaired.end_reason,
                    repaired.measurement_limit,
                    &[],
                ).natural;
                let adjusted_width = measurement.measure(
                    in_measure,
                    repaired.end_reason,
                    repaired.measurement_limit,
                    &[],
                ).adjusted;
                repaired.natural_width = natural_width;
                repaired.adjusted_width = adjusted_width;
            }
            repaired.validate_hanging_suffix();
            lines[i - 1] = repaired;
            if end == curr.cluster_range.last() {
                lines.remove(i);
            } else {
                let mut remainder = rebuild_line(
                    IntRange::new(end + 1, curr.cluster_range.last()),
                    natural,
                    adjusted,
                    curr.end_reason,
                    None,
                    Vec::new(),
                );
                remainder.measurement = curr.measurement.clone();
                remainder.measurement_limit = line_limit(
                    max_width,
                    first_line_indent,
                    remainder.cluster_range.first(),
                );
                remainder.remeasure();
                lines[i] = remainder;
            }
            continue;
        }
        if previous.cluster_range.first() >= previous.cluster_range.last() {
            lines[i] = leave_ragged(
                curr,
                leave_ragged_penalty,
                carry_previous_penalty,
                RepairRejection::NoRoomToCarry,
                None,
                candidates,
            );
            i += 1;
            continue;
        }
        let carried = previous.cluster_range.last();
        if unbreakable.contains_boundary(carried)
            || previous.measurement.as_ref().is_some_and(|measurement| !measurement.boundary_allowed(carried))
        {
            lines[i] = leave_ragged(
                curr,
                leave_ragged_penalty,
                carry_previous_penalty,
                RepairRejection::CarryWouldSplitMourningSpan,
                Some(carried),
                candidates,
            );
            i += 1;
            continue;
        }
        let mut next = rebuild_line(
            IntRange::new(carried, curr.cluster_range.last()),
            natural,
            adjusted,
            curr.end_reason,
            None,
            Vec::new(),
        );
        next.measurement = curr.measurement.clone();
        next.measurement_limit = line_limit(max_width, first_line_indent, next.cluster_range.first());
        next.remeasure();
        if next.adjusted_width > max_width {
            lines[i] = leave_ragged(
                curr,
                leave_ragged_penalty,
                carry_previous_penalty,
                RepairRejection::CarryOverflows,
                Some(carried),
                candidates,
            );
            i += 1;
            continue;
        }
        candidates.push(RepairCandidate {
            kind: RepairKind::CarryPrevious,
            reason_code: RepairReasonCode::ForbiddenAtLineStart,
            offender_cluster_index: first,
            penalty: carry_previous_penalty,
            accepted: true,
            rejection_reason: None,
            target_cluster_index: None,
            carried_cluster_index: Some(carried),
            shrink: 0.,
            required_shrink: 0.,
            available_capacity: 0.,
        });
        let mut shortened_previous = rebuild_line(
            IntRange::new(previous.cluster_range.first(), carried - 1),
            natural,
            adjusted,
            previous.end_reason,
            None,
            Vec::new(),
        );
        shortened_previous.measurement = previous.measurement.clone();
        shortened_previous.measurement_limit = line_limit(
            max_width,
            first_line_indent,
            shortened_previous.cluster_range.first(),
        );
        shortened_previous.remeasure();
        lines[i - 1] = shortened_previous;
        let mut next = next;
        next.repair = Some(RepairOption::CarryPrevious {
            penalty: carry_previous_penalty,
            reason_code: RepairReasonCode::ForbiddenAtLineStart,
            detail: RepairDetail::Carried {
                carried_cluster_index: carried,
            },
            offender_cluster_index: first,
            carried_cluster_index: carried,
        });
        next.repair_candidates = candidates;
        lines[i] = next;
        i += 1
    }
    let total = lines
        .iter()
        .map(|line| line.repair.as_ref().map_or(0, RepairOption::penalty) as f32)
        .sum();
    LineSolution::with_badness(lines, total)
}

pub fn try_push_in(
    previous: &LineCandidate,
    current: &LineCandidate,
    natural: &[Cluster],
    adjusted: &[Cluster],
    max_width: f32,
    opportunities: &[ShrinkOpportunity],
    penalty: i32,
    merge_through: Option<i32>,
    reason_code: RepairReasonCode,
) -> PushInResult {
    let offender = merge_through.unwrap_or(current.cluster_range.first());
    assert!(
        current.cluster_range.contains(offender),
        "PushIn merge-through cluster must belong to the current line."
    );
    let end = mandatory_break_tail_end(current, offender, adjusted);
    let end = previous.measurement.as_ref().map_or(end, |measurement| {
        measurement.next_boundary(end + 1, current.cluster_range.last() + 1) - 1
    });
    let range = IntRange::new(previous.cluster_range.first(), end);
    let mut expanded = if previous.measurement.is_some() {
        LineCandidate::new(range, TextRange::new(adjusted[range.first() as usize].range.start(),
            adjusted[end as usize].range.end()), 0.0, 0.0)
    } else { rebuild_line(range, natural, adjusted, LineEndReason::AutoWrap, None, Vec::new()) };
    expanded.measurement = previous.measurement.clone();
    expanded.measurement_limit = max_width;
    expanded.end_reason = if end == current.cluster_range.last() { current.end_reason } else { previous.end_reason };
    expanded.remeasure();
    let overflow = expanded.adjusted_width - max_width;
    let mut in_line: Vec<_> = opportunities
        .iter()
        .filter(|opp| {
            expanded.cluster_range.contains(opp.cluster_index)
                && opp.capacity > 0.
                && (!opp.line_end_only || opp.cluster_index == offender)
                && expanded.measurement.as_ref().is_none_or(|measurement| {
                    measurement.shrink_is_effective(expanded.cluster_range, expanded.end_reason, opp.cluster_index)
                })
        })
        .copied()
        .collect();
    for opp in &mut in_line {
        if opp.cluster_index == offender
            && matches!(
                opp.channel,
                ShrinkChannel::TrailingGlue | ShrinkChannel::LeadingAndTrailingGlue
            )
        {
            opp.tier = 1
        }
    }
    let capacity: f32 = in_line.iter().map(|opp| opp.capacity).sum();
    let resolved_allocations = if let Some(measurement) = &expanded.measurement {
        measurement.compression(expanded.cluster_range, expanded.end_reason, max_width, &in_line)
    } else if overflow <= capacity {
        Some(distribute_push_in_shrink(&in_line, overflow.max(0.0)))
    } else { None };
    if resolved_allocations.is_none() {
        return PushInResult {
            previous: previous.clone(),
            current: Some(current.clone()),
            candidate: RepairCandidate {
                kind: RepairKind::PushIn,
                reason_code,
                offender_cluster_index: offender,
                penalty,
                accepted: false,
                rejection_reason: Some(RepairRejection::InsufficientCapacity),
                target_cluster_index: Some(offender),
                carried_cluster_index: None,
                shrink: 0.,
                required_shrink: overflow.max(0.),
                available_capacity: capacity,
            },
        };
    }
    let allocations = resolved_allocations.unwrap();
    let shrink: f32 = allocations.iter().map(|allocation| allocation.shrink).sum();
    let mut repaired = expanded;
    repaired.adjusted_width -= shrink;
    repaired.end_reason = if end == current.cluster_range.last() {
        current.end_reason
    } else {
        previous.end_reason
    };
    repaired.repair = Some(RepairOption::PushIn {
        penalty,
        reason_code,
        detail: if shrink > 0. {
            RepairDetail::PushedIn
        } else {
            RepairDetail::FitsNoShrink
        },
        offender_cluster_index: offender,
        allocations,
        total_shrink: shrink,
        total_available_capacity: capacity,
    });
    repaired.remeasure();
    repaired.repair_candidates = [
        previous.repair_candidates.clone(),
        vec![RepairCandidate {
            kind: RepairKind::PushIn,
            reason_code,
            offender_cluster_index: offender,
            penalty,
            accepted: true,
            rejection_reason: None,
            target_cluster_index: Some(offender),
            carried_cluster_index: None,
            shrink,
            required_shrink: shrink,
            available_capacity: capacity,
        }],
    ]
    .concat();
    let following = (end < current.cluster_range.last()).then(|| {
        let mut line = rebuild_line(
            IntRange::new(end + 1, current.cluster_range.last()),
            natural,
            adjusted,
            current.end_reason,
            None,
            Vec::new(),
        );
        line.measurement = current.measurement.clone();
        line.measurement_limit = current.measurement_limit;
        line.remeasure();
        line
    });
    PushInResult {
        previous: repaired,
        current: following,
        candidate: RepairCandidate {
            kind: RepairKind::PushIn,
            reason_code,
            offender_cluster_index: offender,
            penalty,
            accepted: true,
            rejection_reason: None,
            target_cluster_index: Some(offender),
            carried_cluster_index: None,
            shrink,
            required_shrink: shrink,
            available_capacity: capacity,
        },
    }
}

#[allow(clippy::too_many_arguments)]
pub fn apply_fill_push_in(
    lines: &[LineCandidate],
    natural: &[Cluster],
    adjusted: &[Cluster],
    max_width: f32,
    opportunities: &[ShrinkOpportunity],
    first_line_indent: f32,
    compress_bias: f32,
    forbidden_start: Option<&HashSet<i32>>,
    forbidden_end: &HashSet<i32>,
    unbreakable: &UnbreakableRanges,
    penalty: i32,
    gap_boundaries: &HashSet<i32>,
    progressive: &HashMap<i32, ProgressiveBreakOpportunity>,
) -> Vec<LineCandidate> {
    if lines.len() < 2 || compress_bias <= 0. {
        return lines.to_vec();
    }
    let mut out = lines.to_vec();
    let mut i = 0;
    while i + 1 < out.len() {
        let previous = &out[i];
        let current = &out[i + 1];
        let zero = matches!(&previous.repair, Some(RepairOption::PushIn { total_shrink, reason_code, .. }) if *total_shrink <= 0.001 && *reason_code == RepairReasonCode::LineAdjustmentPushIn);
        if (previous.repair.is_some() && !zero)
            || previous.hanging_cluster_index().is_some()
            || previous.end_reason != LineEndReason::AutoWrap
        {
            i += 1;
            continue;
        }
        let limit = line_limit(max_width, first_line_indent, previous.cluster_range.first());
        let deficit = limit - previous.adjusted_width;
        if deficit <= 0. {
            i += 1;
            continue;
        }
        let Some(mut end) =
            fill_push_in_group_end(current, forbidden_start, forbidden_end, unbreakable)
        else {
            i += 1;
            continue;
        };
        let measure_added = |end: i32| {
            let range = IntRange::new(previous.cluster_range.first(), end);
            let reason = if end == current.cluster_range.last() {
                current.end_reason
            } else {
                previous.end_reason
            };
            previous.measurement.as_ref().map_or_else(
                || {
                    (current.cluster_range.first()..=end)
                        .map(|index| adjusted[index as usize].advance)
                        .sum::<f32>()
                },
                |measurement| {
                    measurement.adjusted_width(range, reason, limit, &[])
                        - previous.adjusted_width
                },
            )
        };
        if let Some(measurement) = &previous.measurement {
            let boundary = measurement.next_boundary(end + 1, current.cluster_range.last() + 1);
            if boundary > current.cluster_range.last() + 1 {
                i += 1;
                continue;
            }
            end = boundary - 1;
        }
        let mut added = measure_added(end);
        let current_break = progressive.get(&(previous.cluster_range.last() + 1));
        let mut next_break = progressive.get(&(end + 1));
        let mut promotes = current_break
            .zip(next_break)
            .is_some_and(|(before, after)| {
                before.span_range == after.span_range
                    && after.tier.priority() < before.tier.priority()
            });
        if promotes
            && added < deficit - PROGRESSIVE_TIER_PROMOTION_FILL_EPSILON
            && let Some(before) = current_break
            && let Some(boundary) =
                ((end + 2)..=current.cluster_range.last() + 1).find(|boundary| {
                    progressive.get(boundary).is_some_and(|candidate| {
                        candidate.span_range == before.span_range && candidate.tier == before.tier
                    })
                })
        {
            end = boundary - 1;
            next_break = progressive.get(&boundary);
            if let Some(measurement) = &previous.measurement {
                let boundary = measurement.next_boundary(end + 1, current.cluster_range.last() + 1);
                if boundary > current.cluster_range.last() + 1 {
                    i += 1;
                    continue;
                }
                end = boundary - 1;
            }
            added = measure_added(end);
            promotes = false
        }
        if current_break
            .zip(next_break)
            .is_some_and(|(before, after)| {
                before.span_range == after.span_range
                    && after.tier.priority() > before.tier.priority()
            })
        {
            i += 1;
            continue;
        }
        let overflow = added - deficit;
        if promotes && overflow < -PROGRESSIVE_TIER_PROMOTION_FILL_EPSILON {
            i += 1;
            continue;
        }
        let has_tabs = previous.measurement.as_ref().is_some_and(|measurement| {
            measurement.has_tabs(IntRange::new(previous.cluster_range.first(), end))
        });
        // 无 Tab 的成本已确定，先排除无需压缩求解的候选。
        if !has_tabs && !promotes {
            if overflow >= deficit * compress_bias {
                i += 1;
                continue;
            }
            if overflow > 0. {
                let stretch = line_gap_count(previous.cluster_range, gap_boundaries);
                let compression = line_gap_count(
                    IntRange::new(previous.cluster_range.first(), end), gap_boundaries);
                if overflow / (compression.max(1) as f32)
                    > if stretch == 0 { 0. } else { deficit / stretch as f32 }
                {
                    i += 1;
                    continue;
                }
            }
        }
        let result = try_push_in(
            previous,
            current,
            natural,
            adjusted,
            limit,
            opportunities,
            penalty,
            Some(end),
            if promotes {
                RepairReasonCode::ProgressiveTechnicalTierPromotion
            } else {
                RepairReasonCode::LineAdjustmentPushIn
            },
        );
        if !result.candidate.accepted {
            i += 1;
            continue;
        }
        let cost = result.candidate.shrink;
        if has_tabs && !promotes && cost >= deficit * compress_bias {
            i += 1;
            continue;
        }
        if has_tabs && cost > 0. && !promotes {
            let stretch = line_gap_count(previous.cluster_range, gap_boundaries);
            let compression = line_gap_count(
                IntRange::new(previous.cluster_range.first(), end),
                gap_boundaries,
            );
            if cost / (compression.max(1) as f32)
                > if stretch == 0 { 0. } else { deficit / stretch as f32 }
            {
                i += 1;
                continue;
            }
        }
        if result.candidate.accepted {
            let continues = matches!(&result.previous.repair, Some(RepairOption::PushIn { total_shrink, reason_code, .. }) if *total_shrink <= 0.001 && *reason_code == RepairReasonCode::LineAdjustmentPushIn);
            out[i] = result.previous;
            if let Some(current) = result.current {
                out[i + 1] = current;
                if continues {
                    continue;
                }
            } else {
                out.remove(i + 1);
            }
        }
        i += 1
    }
    out
}
pub fn with_fill_push_in(
    solution: LineSolution,
    enabled: bool,
    natural: &[Cluster],
    adjusted: &[Cluster],
    max_width: f32,
    opportunities: &[ShrinkOpportunity],
    first_line_indent: f32,
    compress_bias: f32,
    forbidden_start: Option<&HashSet<i32>>,
    forbidden_end: &HashSet<i32>,
    unbreakable: &UnbreakableRanges,
    penalty: i32,
    gap_boundaries: &HashSet<i32>,
    progressive: &HashMap<i32, ProgressiveBreakOpportunity>,
) -> LineSolution {
    if enabled {
        LineSolution::with_badness(
            apply_fill_push_in(
                &solution.lines,
                natural,
                adjusted,
                max_width,
                opportunities,
                first_line_indent,
                compress_bias,
                forbidden_start,
                forbidden_end,
                unbreakable,
                penalty,
                gap_boundaries,
                progressive,
            ),
            solution.total_badness,
        )
    } else {
        solution
    }
}
fn leave_ragged(
    mut line: LineCandidate,
    penalty: i32,
    carry_penalty: i32,
    cause: RepairRejection,
    carried: Option<i32>,
    mut candidates: Vec<RepairCandidate>,
) -> LineCandidate {
    candidates.push(RepairCandidate {
        kind: RepairKind::CarryPrevious,
        reason_code: RepairReasonCode::ForbiddenAtLineStart,
        offender_cluster_index: line.cluster_range.first(),
        penalty: carry_penalty,
        accepted: false,
        rejection_reason: Some(cause),
        target_cluster_index: None,
        carried_cluster_index: carried,
        shrink: 0.,
        required_shrink: 0.,
        available_capacity: 0.,
    });
    candidates.push(RepairCandidate {
        kind: RepairKind::LeaveRagged,
        reason_code: RepairReasonCode::ForbiddenAtLineStart,
        offender_cluster_index: line.cluster_range.first(),
        penalty,
        accepted: true,
        rejection_reason: None,
        target_cluster_index: None,
        carried_cluster_index: None,
        shrink: 0.,
        required_shrink: 0.,
        available_capacity: 0.,
    });
    line.repair = Some(RepairOption::LeaveRagged {
        penalty,
        reason_code: RepairReasonCode::ForbiddenAtLineStart,
        detail: RepairDetail::Rejected(cause),
        offender_cluster_index: line.cluster_range.first(),
    });
    line.repair_candidates = candidates;
    line
}
fn mandatory_break_tail_end(current: &LineCandidate, through: i32, adjusted: &[Cluster]) -> i32 {
    if current.end_reason != LineEndReason::MandatoryBreak
        || through >= current.cluster_range.last()
    {
        return through;
    }
    if ((through + 1)..=current.cluster_range.last()).all(|index| {
        adjusted[index as usize].display_text.is_empty() && adjusted[index as usize].advance == 0.
    }) {
        current.cluster_range.last()
    } else {
        through
    }
}
fn fill_push_in_group_end(
    current: &LineCandidate,
    forbidden_start: Option<&HashSet<i32>>,
    forbidden_end: &HashSet<i32>,
    unbreakable: &UnbreakableRanges,
) -> Option<i32> {
    let mut end = current.cluster_range.first();
    while end <= current.cluster_range.last() {
        if let Some(range) = unbreakable.containing_from_closed_start_or_null(end) {
            end = range.last();
            if end > current.cluster_range.last() {
                return None;
            }
            continue;
        }
        if forbidden_end.contains(&end) {
            end += 1;
            continue;
        }
        let next = end + 1;
        if next <= current.cluster_range.last()
            && forbidden_start.is_some_and(|set| set.contains(&next))
        {
            end = next;
            continue;
        }
        return Some(end);
    }
    None
}
pub(crate) fn distribute_push_in_shrink(
    opportunities: &[ShrinkOpportunity],
    total: f32,
) -> Vec<PushInAllocation> {
    if opportunities.is_empty() || total <= 0. {
        return Vec::new();
    }
    let mut result = Vec::new();
    let mut remaining = total;
    let mut tiers: Vec<_> = opportunities
        .iter()
        .map(|opportunity| opportunity.tier)
        .collect();
    tiers.sort();
    tiers.dedup();
    for tier in tiers {
        if remaining <= 0. {
            break;
        }
        let mut items: Vec<_> = opportunities
            .iter()
            .filter(|opportunity| opportunity.tier == tier)
            .copied()
            .collect();
        items.sort_by_key(|opportunity| opportunity.cluster_index);
        let capacity: f32 = items.iter().map(|opportunity| opportunity.capacity).sum();
        if capacity <= 0. {
            continue;
        }
        let target = remaining.min(capacity);
        let mut left = target;
        let last = items.len() - 1;
        for (index, opportunity) in items.iter().enumerate() {
            let amount = if index == last {
                left.min(opportunity.capacity)
            } else {
                (target * opportunity.capacity / capacity).min(opportunity.capacity)
            };
            if amount > 0. {
                result.push(PushInAllocation {
                    cluster_index: opportunity.cluster_index,
                    shrink: amount,
                    available_capacity: opportunity.capacity,
                    channel: opportunity.channel,
                });
                left -= amount
            }
        }
        remaining -= target - left.max(0.)
    }
    result
}
const PROGRESSIVE_TIER_PROMOTION_FILL_EPSILON: f32 = 0.001;
