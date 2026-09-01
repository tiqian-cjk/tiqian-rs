use tiqian::common::{HashMap, HashSet};
use tiqian::core::geometry::TextRange;
use tiqian::core::int_range::IntRange;
use tiqian::core::layout_model::{Cluster, LineEndReason};
use tiqian::core::text::Text;
use tiqian::layout::kinsoku_rule::ClreqKinsokuRule;
use tiqian::layout::line_breaker::{empty_line_candidate, rebuild_line};
use tiqian::layout::line_optimization::{LineCandidate, LineSolution, RepairOption};
use tiqian::layout::line_repair::{
    apply_fill_push_in, apply_kinsoku_repairs, try_push_in, with_fill_push_in,
};
use tiqian::layout::progressive_break_decisions::{
    ProgressiveBreakOpportunity, ProgressiveBreakTier, ShrinkChannel, ShrinkOpportunity,
    UnbreakableRanges,
};

const EM: f32 = 16.0;

fn cluster(text: &str, index: i32, advance: f32, display_text: &str) -> Cluster {
    Cluster::with_display_text(
        TextRange::new(index, index + text.encode_utf16().count() as i32),
        Text::from(text),
        Text::from(display_text),
        "k".to_owned(),
        advance,
    )
}

fn clusters(text: &str, start: i32) -> Vec<Cluster> {
    text.chars()
        .enumerate()
        .map(|(offset, character)| {
            let text = character.to_string();
            cluster(&text, start + offset as i32, EM, &text)
        })
        .collect()
}

fn line(
    range: IntRange,
    natural: &[Cluster],
    adjusted: &[Cluster],
    end_reason: LineEndReason,
    repair: Option<RepairOption>,
    hanging: HashSet<i32>,
) -> LineCandidate {
    let mut line = rebuild_line(range, natural, adjusted, end_reason, repair, Vec::new());
    line.hanging_cluster_indices = hanging;
    line.validate_hanging_suffix();
    line
}

#[allow(clippy::too_many_arguments)]
fn repairs(
    initial: &[LineCandidate],
    natural: &[Cluster],
    adjusted: &[Cluster],
    max_width: f32,
    opportunities: &[ShrinkOpportunity],
    unbreakable: &[IntRange],
    hangable: &HashSet<i32>,
    extendable_hang: &[IntRange],
    forbidden_start: Option<&HashSet<i32>>,
) -> LineSolution {
    apply_kinsoku_repairs(
        initial,
        natural,
        adjusted,
        max_width,
        &ClreqKinsokuRule::default(),
        opportunities,
        10,
        20,
        30,
        &UnbreakableRanges::new(unbreakable.to_vec()),
        0.0,
        hangable,
        extendable_hang,
        5,
        forbidden_start,
    )
}

fn comma_paragraph() -> (Vec<Cluster>, Vec<LineCandidate>) {
    let mut natural = clusters("AAAA", 0);
    natural.push(cluster("，", 4, EM, "，"));
    natural.extend(clusters("BBBB", 5));
    let initial = vec![
        line(IntRange::new(0, 3), &natural, &natural, LineEndReason::AutoWrap, None, HashSet::new()),
        line(IntRange::new(4, 8), &natural, &natural, LineEndReason::AutoWrap, None, HashSet::new()),
    ];
    (natural, initial)
}

#[allow(clippy::too_many_arguments)]
fn fill(
    lines: &[LineCandidate],
    natural: &[Cluster],
    max_width: f32,
    opportunities: &[ShrinkOpportunity],
    compress_bias: f32,
    forbidden_start: Option<&HashSet<i32>>,
    forbidden_end: &HashSet<i32>,
    unbreakable: &[IntRange],
    gap_boundaries: &HashSet<i32>,
    progressive: &HashMap<i32, ProgressiveBreakOpportunity>,
) -> Vec<LineCandidate> {
    apply_fill_push_in(
        lines,
        natural,
        natural,
        max_width,
        opportunities,
        0.0,
        compress_bias,
        forbidden_start,
        forbidden_end,
        &UnbreakableRanges::new(unbreakable.to_vec()),
        10,
        gap_boundaries,
        progressive,
    )
}

#[test]
fn push_in_fits_without_shrink_when_the_merged_line_already_matches() {
    let (natural, initial) = comma_paragraph();
    let solution = repairs(&initial, &natural, &natural, 80.0, &[], &[], &HashSet::new(), &[], None);
    assert_eq!(2, solution.lines.len());
    let RepairOption::PushIn { total_shrink, reason, .. } = solution.lines[0].repair.as_ref().unwrap() else { panic!() };
    assert_eq!(IntRange::new(0, 4), solution.lines[0].cluster_range);
    assert_eq!(0.0, *total_shrink);
    assert_eq!(IntRange::new(5, 8), solution.lines[1].cluster_range);
    assert!(reason.ends_with("fits-no-shrink"));
}

#[test]
fn push_in_promotes_the_offenders_own_trailing_glue_to_tier_one() {
    let (natural, initial) = comma_paragraph();
    let solution = repairs(
        &initial, &natural, &natural, 76.0,
        &[
            ShrinkOpportunity::new(4, 3, 8.0, ShrinkChannel::TrailingGlue),
            ShrinkOpportunity::new(0, 2, 8.0, ShrinkChannel::TrailingGlue),
        ],
        &[], &HashSet::new(), &[], None,
    );
    let RepairOption::PushIn { total_shrink, allocations, reason, .. } = solution.lines[0].repair.as_ref().unwrap() else { panic!() };
    assert_eq!(4.0, *total_shrink);
    assert_eq!(1, allocations.len());
    assert_eq!(4, allocations[0].cluster_index);
    assert_eq!(4.0, allocations[0].shrink);
    assert_eq!(8.0, allocations[0].available_capacity);
    assert_eq!(ShrinkChannel::TrailingGlue, allocations[0].channel);
    assert_eq!("ForbiddenAtLineStart:，:pushed-in=4.0/16.0", reason);
}

#[test]
fn push_in_rejects_when_capacity_is_insufficient() {
    let (natural, initial) = comma_paragraph();
    let solution = repairs(
        &initial, &natural, &natural, 60.0,
        &[ShrinkOpportunity::new(0, 1, 8.0, ShrinkChannel::TrailingGlue)],
        &[], &HashSet::new(), &[], None,
    );
    let current = &solution.lines[1];
    let rejected = current.repair_candidates.iter().find(|candidate| candidate.kind == "PushIn").unwrap();
    assert!(!rejected.accepted);
    assert_eq!(Some("insufficient-capacity"), rejected.rejection_reason.as_deref());
    assert_eq!(20.0, rejected.required_shrink);
    assert_eq!(8.0, rejected.available_capacity);
    assert!(matches!(current.repair, Some(RepairOption::LeaveRagged { .. })));
}

#[test]
#[should_panic(expected = "must belong to the current line")]
fn push_in_rejects_merge_through_outside_the_current_line() {
    let (natural, initial) = comma_paragraph();
    try_push_in(&initial[0], &initial[1], &natural, &natural, 80.0, &[], 10, Some(9), "ForbiddenAtLineStart");
}

#[test]
fn push_in_filters_out_of_range_zero_capacity_and_foreign_line_end_only_opportunities() {
    let (natural, initial) = comma_paragraph();
    let result = try_push_in(
        &initial[0], &initial[1], &natural, &natural, 72.0,
        &[
            ShrinkOpportunity::new(9, 1, 8.0, ShrinkChannel::TrailingGlue),
            ShrinkOpportunity::new(0, 1, 0.0, ShrinkChannel::TrailingGlue),
            ShrinkOpportunity::with_line_end_only(1, 1, 8.0, ShrinkChannel::TrailingGlue, true),
            ShrinkOpportunity::with_line_end_only(4, 4, 16.0, ShrinkChannel::LeadingAndTrailingGlue, true),
        ],
        10, None, "ForbiddenAtLineStart",
    );
    assert!(result.candidate.accepted);
    let RepairOption::PushIn { allocations, total_available_capacity, .. } = result.previous.repair.as_ref().unwrap() else { panic!() };
    assert_eq!(16.0, *total_available_capacity);
    assert_eq!(1, allocations.len());
    assert_eq!(4, allocations[0].cluster_index);
    assert_eq!(8.0, allocations[0].shrink);
    assert_eq!(ShrinkChannel::LeadingAndTrailingGlue, allocations[0].channel);
}

#[test]
fn push_in_reports_infinity_capacity_with_a_portable_debug_string() {
    let (natural, initial) = comma_paragraph();
    let result = try_push_in(
        &initial[0], &initial[1], &natural, &natural, f32::NEG_INFINITY,
        &[ShrinkOpportunity::new(0, 1, f32::INFINITY, ShrinkChannel::TrailingGlue)],
        10, None, "ForbiddenAtLineStart",
    );
    assert!(result.candidate.accepted);
    assert!(result.candidate.shrink.is_infinite());
    let RepairOption::PushIn { reason, .. } = result.previous.repair.as_ref().unwrap() else { panic!() };
    assert_eq!("ForbiddenAtLineStart:，:pushed-in=Infinity.0/Infinity.0", reason);
}

#[test]
fn push_in_underflow_shares_skip_zero_valued_proportional_shares() {
    let min_denormal = f32::from_bits(1);
    let natural = vec![cluster("a", 0, min_denormal, "a"), cluster("b", 1, min_denormal, "b")];
    let result = try_push_in(
        &line(IntRange::new(0, 0), &natural, &natural, LineEndReason::AutoWrap, None, HashSet::new()),
        &line(IntRange::new(1, 1), &natural, &natural, LineEndReason::AutoWrap, None, HashSet::new()),
        &natural, &natural, 0.0,
        &[
            ShrinkOpportunity::new(0, 1, min_denormal, ShrinkChannel::TrailingGlue),
            ShrinkOpportunity::new(1, 1, min_denormal, ShrinkChannel::TrailingGlue),
        ],
        10, None, "ForbiddenAtLineStart",
    );
    assert!(result.candidate.accepted);
    let RepairOption::PushIn { allocations, .. } = result.previous.repair.as_ref().unwrap() else { panic!() };
    assert_eq!(1, allocations.len());
    assert_eq!(1, allocations[0].cluster_index);
    assert_eq!(min_denormal, allocations[0].shrink);
}

#[test]
fn hang_merges_the_offender_beyond_the_measure() {
    let (natural, initial) = comma_paragraph();
    let solution = repairs(&initial, &natural, &natural, 64.0, &[], &[], &HashSet::from([4]), &[], None);
    let merged = &solution.lines[0];
    let RepairOption::Hang { penalty, offender_cluster_index, .. } = merged.repair.as_ref().unwrap() else { panic!() };
    assert_eq!(IntRange::new(0, 4), merged.cluster_range);
    assert_eq!(HashSet::from([4]), merged.hanging_cluster_indices);
    assert_eq!(4, *offender_cluster_index);
    assert_eq!(64.0, merged.adjusted_width);
    assert_eq!(80.0, merged.natural_width);
    assert_eq!(IntRange::new(5, 8), solution.lines[1].cluster_range);
    assert_eq!(5, *penalty);
}

#[test]
#[should_panic(expected = "must belong to the current line")]
fn push_in_rejects_a_merge_through_cluster_outside_the_current_line() {
    let natural = [clusters("AAAA", 0), clusters("BB", 4)].concat();
    try_push_in(
        &line(IntRange::new(0, 3), &natural, &natural, LineEndReason::AutoWrap, None, HashSet::new()),
        &line(IntRange::new(4, 5), &natural, &natural, LineEndReason::AutoWrap, None, HashSet::new()),
        &natural, &natural, 96.0, &[], 10, Some(0), "ForbiddenAtLineStart",
    );
}

#[test]
fn hang_consumes_a_zero_width_mandatory_break_tail() {
    let natural = [clusters("AAAA", 0), vec![cluster("，", 4, EM, "，"), cluster("\n", 5, 0.0, "")]].concat();
    let initial = vec![
        line(IntRange::new(0, 3), &natural, &natural, LineEndReason::AutoWrap, None, HashSet::new()),
        line(IntRange::new(4, 5), &natural, &natural, LineEndReason::MandatoryBreak, None, HashSet::new()),
    ];
    let solution = repairs(&initial, &natural, &natural, 64.0, &[], &[], &HashSet::from([4]), &[], None);
    let merged = &solution.lines[0];
    assert_eq!(1, solution.lines.len());
    assert_eq!(IntRange::new(0, 5), merged.cluster_range);
    assert_eq!(HashSet::from([4, 5]), merged.hanging_cluster_indices);
    assert_eq!(LineEndReason::MandatoryBreak, merged.end_reason);
}

#[test]
fn hang_stops_before_a_non_zero_width_mandatory_break_tail() {
    let natural = [clusters("AAAA", 0), vec![cluster("，", 4, EM, "，"), cluster("\n", 5, 8.0, "")]].concat();
    let initial = vec![
        line(IntRange::new(0, 3), &natural, &natural, LineEndReason::AutoWrap, None, HashSet::new()),
        line(IntRange::new(4, 5), &natural, &natural, LineEndReason::MandatoryBreak, None, HashSet::new()),
    ];
    let solution = repairs(&initial, &natural, &natural, 64.0, &[], &[], &HashSet::from([4]), &[], None);
    assert_eq!(IntRange::new(0, 4), solution.lines[0].cluster_range);
    assert_eq!(HashSet::from([4]), solution.lines[0].hanging_cluster_indices);
    assert_eq!(LineEndReason::AutoWrap, solution.lines[0].end_reason);
    assert_eq!(IntRange::new(5, 5), solution.lines[1].cluster_range);
}

#[test]
fn contextual_hang_extends_only_inside_its_protected_group() {
    let natural = [clusters("AAA，", 0), vec![cluster("，", 4, EM, "，")], clusters("BBB", 5)].concat();
    let initial = vec![
        line(IntRange::new(0, 3), &natural, &natural, LineEndReason::AutoWrap, None, HashSet::from([3])),
        line(IntRange::new(4, 7), &natural, &natural, LineEndReason::AutoWrap, None, HashSet::new()),
    ];
    let extended = repairs(&initial, &natural, &natural, 64.0, &[], &[], &HashSet::from([4]), &[IntRange::new(3, 5)], None);
    assert_eq!(HashSet::from([3, 4]), extended.lines[0].hanging_cluster_indices);

    let outside_natural = [clusters("AAAA", 0), vec![cluster("，", 4, EM, "，")], clusters("BB", 5)].concat();
    let outside_initial = vec![
        line(IntRange::new(0, 3), &outside_natural, &outside_natural, LineEndReason::AutoWrap, None, HashSet::from([3])),
        line(IntRange::new(4, 5), &outside_natural, &outside_natural, LineEndReason::AutoWrap, None, HashSet::new()),
    ];
    let outside = repairs(&outside_initial, &outside_natural, &outside_natural, 64.0, &[], &[], &HashSet::from([4]), &[IntRange::new(6, 7)], None);
    assert!(outside.lines[0].hanging_cluster_indices.is_empty());
    let RepairOption::CarryPrevious { carried_cluster_index, .. } = outside.lines[1].repair.as_ref().unwrap() else { panic!() };
    assert_eq!(IntRange::new(3, 5), outside.lines[1].cluster_range);
    assert_eq!(3, *carried_cluster_index);

    let partial = repairs(&outside_initial, &outside_natural, &outside_natural, 64.0, &[], &[], &HashSet::from([4]), &[IntRange::new(4, 5)], None);
    assert!(matches!(partial.lines[1].repair, Some(RepairOption::CarryPrevious { .. })));

    let gap_natural = [clusters("AAA，", 0), vec![cluster("中", 4, EM, "中"), cluster("，", 5, EM, "，")], clusters("BB", 6)].concat();
    let gapped_initial = vec![
        line(IntRange::new(0, 3), &gap_natural, &gap_natural, LineEndReason::AutoWrap, None, HashSet::from([3])),
        line(IntRange::new(5, 7), &gap_natural, &gap_natural, LineEndReason::AutoWrap, None, HashSet::new()),
    ];
    let gapped = repairs(&gapped_initial, &gap_natural, &gap_natural, 64.0, &[], &[], &HashSet::from([5]), &[], None);
    assert_eq!(HashSet::from([3]), gapped.lines[0].hanging_cluster_indices);
    assert!(matches!(gapped.lines[1].repair, Some(RepairOption::LeaveRagged { .. })));
}

#[test]
fn leave_ragged_records_no_room_to_carry_for_a_single_cluster_line() {
    let natural = vec![cluster("中", 0, EM, "中"), cluster("，", 1, EM, "，"), cluster("中", 2, EM, "中")];
    let initial = vec![
        line(IntRange::new(0, 0), &natural, &natural, LineEndReason::AutoWrap, None, HashSet::new()),
        line(IntRange::new(1, 2), &natural, &natural, LineEndReason::AutoWrap, None, HashSet::new()),
    ];
    let solution = repairs(&initial, &natural, &natural, 16.0, &[], &[], &HashSet::new(), &[], None);
    let current = &solution.lines[1];
    let carry = current.repair_candidates.iter().find(|candidate| candidate.kind == "CarryPrevious").unwrap();
    assert!(!carry.accepted);
    assert_eq!(Some("no-room-to-carry"), carry.rejection_reason.as_deref());
    let RepairOption::LeaveRagged { reason, .. } = current.repair.as_ref().unwrap() else { panic!() };
    assert!(reason.contains("no-room-to-carry"));
}

#[test]
fn leave_ragged_refuses_carries_that_would_split_an_unbreakable_span() {
    let (natural, initial) = comma_paragraph();
    let solution = repairs(&initial, &natural, &natural, 60.0, &[], &[IntRange::new(2, 3)], &HashSet::new(), &[], None);
    let current = &solution.lines[1];
    let carry = current.repair_candidates.iter().find(|candidate| candidate.kind == "CarryPrevious").unwrap();
    assert_eq!(Some("carry-would-split-mourning-span"), carry.rejection_reason.as_deref());
    assert_eq!(Some(3), carry.carried_cluster_index);
    let RepairOption::LeaveRagged { reason, .. } = current.repair.as_ref().unwrap() else { panic!() };
    assert!(reason.contains("carry-would-split-mourning-span"));
}

#[test]
fn carry_previous_moves_the_previous_tail_down_when_it_fits() {
    let natural = [clusters("AAAA", 0), vec![cluster("，", 4, EM, "，")], clusters("BB", 5)].concat();
    let initial = vec![
        line(IntRange::new(0, 3), &natural, &natural, LineEndReason::AutoWrap, None, HashSet::new()),
        line(IntRange::new(4, 6), &natural, &natural, LineEndReason::AutoWrap, None, HashSet::new()),
    ];
    let solution = repairs(
        &initial, &natural, &natural, 70.0,
        &[ShrinkOpportunity::new(0, 1, 4.0, ShrinkChannel::TrailingGlue)],
        &[], &HashSet::new(), &[], None,
    );
    assert_eq!(IntRange::new(0, 2), solution.lines[0].cluster_range);
    assert_eq!(IntRange::new(3, 6), solution.lines[1].cluster_range);
    let RepairOption::CarryPrevious { carried_cluster_index, reason, .. } = solution.lines[1].repair.as_ref().unwrap() else { panic!() };
    assert_eq!(3, *carried_cluster_index);
    assert!(reason.contains("carried=A"));
    assert!(solution.lines[1].repair_candidates.iter().any(|candidate| candidate.kind == "CarryPrevious" && candidate.accepted && candidate.carried_cluster_index == Some(3)));
}

#[test]
fn mandatory_break_and_empty_lines_skip_the_repair_loop() {
    let (natural, _) = comma_paragraph();
    let mandatory_initial = vec![
        line(IntRange::new(0, 3), &natural, &natural, LineEndReason::MandatoryBreak, None, HashSet::new()),
        line(IntRange::new(4, 8), &natural, &natural, LineEndReason::AutoWrap, None, HashSet::new()),
    ];
    let mandatory = repairs(&mandatory_initial, &natural, &natural, 16.0, &[], &[], &HashSet::new(), &[], None);
    assert_eq!(None, mandatory.lines[1].repair);
    let empty_initial = vec![
        line(IntRange::new(0, 3), &natural, &natural, LineEndReason::AutoWrap, None, HashSet::new()),
        empty_line_candidate(64, LineEndReason::ParagraphEnd),
    ];
    let empty = repairs(&empty_initial, &natural, &natural, 16.0, &[], &[], &HashSet::new(), &[], None);
    assert_eq!(2, empty.lines.len());
    assert!(empty.lines[1].cluster_range.is_empty());
}

#[test]
fn forbidden_start_override_controls_the_kinsoku_check() {
    let (natural, initial) = comma_paragraph();
    let disabled = repairs(&initial, &natural, &natural, 64.0, &[], &[], &HashSet::new(), &[], Some(&HashSet::new()));
    assert_eq!(None, disabled.lines[1].repair);

    let plain = clusters("AAAAB", 0);
    let forced_initial = vec![
        line(IntRange::new(0, 3), &plain, &plain, LineEndReason::AutoWrap, None, HashSet::new()),
        line(IntRange::new(4, 4), &plain, &plain, LineEndReason::AutoWrap, None, HashSet::new()),
    ];
    let forced = repairs(&forced_initial, &plain, &plain, 20.0, &[], &[], &HashSet::new(), &[], Some(&HashSet::from([4])));
    assert!(matches!(forced.lines[1].repair, Some(RepairOption::LeaveRagged { .. })));
}

#[test]
fn default_arguments_run_the_full_ragged_chain() {
    let (natural, initial) = comma_paragraph();
    let solution = repairs(&initial, &natural, &natural, 60.0, &[], &[], &HashSet::new(), &[], None);
    let current = &solution.lines[1];
    let rejected = current.repair_candidates.iter().find(|candidate| candidate.kind == "PushIn").unwrap();
    assert!(!rejected.accepted);
    assert_eq!(10, rejected.penalty);
    let RepairOption::LeaveRagged { penalty, .. } = current.repair.as_ref().unwrap() else { panic!() };
    assert_eq!(30, *penalty);
}

#[test]
fn fill_push_in_skips_short_inputs_and_zero_bias() {
    let natural = clusters("ABCDE", 0);
    let single = fill(
        &[line(IntRange::new(0, 3), &natural, &natural, LineEndReason::AutoWrap, None, HashSet::new())],
        &natural, 64.0, &[], 1.0, None, &HashSet::new(), &[], &HashSet::new(), &HashMap::new(),
    );
    assert_eq!(1, single.len());
    let zero_bias = fill(
        &[
            line(IntRange::new(0, 3), &natural, &natural, LineEndReason::AutoWrap, None, HashSet::new()),
            line(IntRange::new(4, 4), &natural, &natural, LineEndReason::AutoWrap, None, HashSet::new()),
        ],
        &natural, 64.0, &[], 0.0, None, &HashSet::new(), &[], &HashSet::new(), &HashMap::new(),
    );
    assert_eq!(2, zero_bias.len());
}

#[test]
fn fill_push_in_skips_repaired_hanging_and_non_auto_wrap_lines() {
    let natural = clusters("ABCDEFGH", 0);
    let repaired = line(
        IntRange::new(0, 3), &natural, &natural, LineEndReason::AutoWrap,
        Some(RepairOption::Hang { penalty: 5, reason: "ForbiddenAtLineStart:，:hang".to_owned(), offender_cluster_index: 3 }),
        HashSet::new(),
    );
    let repaired_result = fill(
        &[repaired, line(IntRange::new(4, 7), &natural, &natural, LineEndReason::AutoWrap, None, HashSet::new())],
        &natural, 128.0, &[], 1.0, None, &HashSet::new(), &[], &HashSet::new(), &HashMap::new(),
    );
    assert_eq!(IntRange::new(0, 3), repaired_result[0].cluster_range);
    let hanging = line(IntRange::new(0, 3), &natural, &natural, LineEndReason::AutoWrap, None, HashSet::from([3]));
    let hanging_result = fill(
        &[hanging, line(IntRange::new(4, 7), &natural, &natural, LineEndReason::AutoWrap, None, HashSet::new())],
        &natural, 128.0, &[], 1.0, None, &HashSet::new(), &[], &HashSet::new(), &HashMap::new(),
    );
    assert_eq!(IntRange::new(0, 3), hanging_result[0].cluster_range);
    let mandatory = line(IntRange::new(0, 3), &natural, &natural, LineEndReason::MandatoryBreak, None, HashSet::new());
    let mandatory_result = fill(
        &[mandatory, line(IntRange::new(4, 7), &natural, &natural, LineEndReason::AutoWrap, None, HashSet::new())],
        &natural, 128.0, &[], 1.0, None, &HashSet::new(), &[], &HashSet::new(), &HashMap::new(),
    );
    assert_eq!(IntRange::new(0, 3), mandatory_result[0].cluster_range);
}

#[test]
fn fill_push_in_skips_full_lines_and_unpullable_groups() {
    let natural = clusters("ABCDEFGH", 0);
    let base = [
        line(IntRange::new(0, 3), &natural, &natural, LineEndReason::AutoWrap, None, HashSet::new()),
        line(IntRange::new(4, 7), &natural, &natural, LineEndReason::AutoWrap, None, HashSet::new()),
    ];
    let full = fill(&base, &natural, 64.0, &[], 1.0, None, &HashSet::new(), &[], &HashSet::new(), &HashMap::new());
    assert_eq!(IntRange::new(0, 3), full[0].cluster_range);
    let spill = fill(&base, &natural, 128.0, &[], 1.0, None, &HashSet::new(), &[IntRange::new(4, 9)], &HashSet::new(), &HashMap::new());
    assert_eq!(IntRange::new(0, 3), spill[0].cluster_range);
    let exhausted = fill(
        &[
            line(IntRange::new(0, 3), &natural, &natural, LineEndReason::AutoWrap, None, HashSet::new()),
            line(IntRange::new(4, 5), &natural, &natural, LineEndReason::AutoWrap, None, HashSet::new()),
        ],
        &natural, 128.0, &[], 1.0, Some(&HashSet::from([5])), &HashSet::from([4, 5]), &[], &HashSet::new(), &HashMap::new(),
    );
    assert_eq!(IntRange::new(0, 3), exhausted[0].cluster_range);
}

#[test]
fn mandatory_break_tail_end_returns_the_merge_through_at_the_line_end() {
    let natural = [clusters("AAAA", 0), vec![cluster("，", 4, EM, "，"), cluster("\n", 5, 0.0, "")]].concat();
    let result = try_push_in(
        &line(IntRange::new(0, 3), &natural, &natural, LineEndReason::AutoWrap, None, HashSet::new()),
        &line(IntRange::new(4, 5), &natural, &natural, LineEndReason::MandatoryBreak, None, HashSet::new()),
        &natural, &natural, 96.0, &[], 10, Some(5), "ForbiddenAtLineStart",
    );
    assert_eq!(IntRange::new(0, 5), result.previous.cluster_range);
}

#[test]
fn fill_push_in_default_arguments_omit_the_optional_boundaries() {
    let natural = [clusters("AAAA", 0), clusters("BBBB", 4)].concat();
    let result = fill(
        &[
            line(IntRange::new(0, 3), &natural, &natural, LineEndReason::AutoWrap, None, HashSet::new()),
            line(IntRange::new(4, 7), &natural, &natural, LineEndReason::AutoWrap, None, HashSet::new()),
        ],
        &natural, 80.0, &[], 1.0 / 3.0, None, &HashSet::new(), &[], &HashSet::new(), &HashMap::new(),
    );
    assert_eq!(vec![IntRange::new(0, 4), IntRange::new(5, 7)], result.into_iter().map(|line| line.cluster_range).collect::<Vec<_>>());
}

#[test]
fn fill_push_in_pulls_the_group_and_cascades_zero_shrink_fills() {
    let natural = [clusters("AAAA", 0), clusters("BBBB", 4)].concat();
    let result = fill(
        &[
            line(IntRange::new(0, 3), &natural, &natural, LineEndReason::AutoWrap, None, HashSet::new()),
            line(IntRange::new(4, 7), &natural, &natural, LineEndReason::AutoWrap, None, HashSet::new()),
        ],
        &natural, 80.0, &[], 1.0, None, &HashSet::new(), &[], &HashSet::new(), &HashMap::new(),
    );
    assert_eq!(IntRange::new(0, 4), result[0].cluster_range);
    assert_eq!(IntRange::new(5, 7), result[1].cluster_range);
    let RepairOption::PushIn { total_shrink, reason, .. } = result[0].repair.as_ref().unwrap() else { panic!() };
    assert_eq!(0.0, *total_shrink);
    assert!(reason.starts_with("LineAdjustmentPushIn:B:fits-no-shrink"));
}

#[test]
fn fill_push_in_extends_past_forbidden_heads_and_unbreakable_chains() {
    let natural = clusters("ABCDEFGH", 0);
    let base = [
        line(IntRange::new(0, 3), &natural, &natural, LineEndReason::AutoWrap, None, HashSet::new()),
        line(IntRange::new(4, 7), &natural, &natural, LineEndReason::AutoWrap, None, HashSet::new()),
    ];
    let forbidden_start = fill(&base, &natural, 96.0, &[], 1.0, Some(&HashSet::from([5])), &HashSet::new(), &[], &HashSet::new(), &HashMap::new());
    assert_eq!(IntRange::new(0, 5), forbidden_start[0].cluster_range);
    let forbidden_end = fill(&base, &natural, 96.0, &[], 1.0, None, &HashSet::from([4]), &[], &HashSet::new(), &HashMap::new());
    assert_eq!(IntRange::new(0, 5), forbidden_end[0].cluster_range);
    let chained = fill(&base, &natural, 128.0, &[], 1.0, None, &HashSet::new(), &[IntRange::new(4, 5), IntRange::new(5, 6)], &HashSet::new(), &HashMap::new());
    assert_eq!(1, chained.len());
    assert_eq!(IntRange::new(0, 7), chained[0].cluster_range);
}

#[test]
fn fill_push_in_rejects_overlarge_pulls_and_worse_compression_density() {
    let natural = [clusters("AAAA", 0), clusters("BBBB", 4)].concat();
    let lines = [
        line(IntRange::new(0, 3), &natural, &natural, LineEndReason::AutoWrap, None, HashSet::new()),
        line(IntRange::new(4, 7), &natural, &natural, LineEndReason::AutoWrap, None, HashSet::new()),
    ];
    let biased = fill(&lines, &natural, 80.0, &[], 1.0, None, &HashSet::new(), &[IntRange::new(4, 7)], &HashSet::new(), &HashMap::new());
    assert_eq!(IntRange::new(0, 3), biased[0].cluster_range);
    assert_eq!(IntRange::new(4, 7), biased[1].cluster_range);
    let dense = fill(&lines, &natural, 72.0, &[], 1.0, None, &HashSet::new(), &[], &HashSet::new(), &HashMap::new());
    assert_eq!(IntRange::new(0, 3), dense[0].cluster_range);
    assert_eq!(IntRange::new(4, 7), dense[1].cluster_range);
}

#[test]
fn fill_push_in_accepts_compression_denser_than_the_cured_stretch() {
    let natural = [clusters("AAAA", 0), clusters("BB", 4)].concat();
    let result = fill(
        &[
            line(IntRange::new(0, 3), &natural, &natural, LineEndReason::AutoWrap, None, HashSet::new()),
            line(IntRange::new(4, 5), &natural, &natural, LineEndReason::AutoWrap, None, HashSet::new()),
        ],
        &natural, 84.0,
        &[ShrinkOpportunity::new(0, 1, 16.0, ShrinkChannel::TrailingGlue)],
        1.0, Some(&HashSet::from([5])), &HashSet::new(), &[], &HashSet::from([0, 1, 2, 3, 4]), &HashMap::new(),
    );
    assert_eq!(IntRange::new(0, 5), result[0].cluster_range);
    let RepairOption::PushIn { total_shrink, .. } = result[0].repair.as_ref().unwrap() else { panic!() };
    assert_eq!(12.0, *total_shrink);
}

#[test]
fn fill_push_in_honours_progressive_tier_promotion_boundaries() {
    let natural = clusters("AABBBB", 0);
    let lines = [
        line(IntRange::new(0, 1), &natural, &natural, LineEndReason::AutoWrap, None, HashSet::new()),
        line(IntRange::new(2, 5), &natural, &natural, LineEndReason::AutoWrap, None, HashSet::new()),
    ];
    let span = TextRange::new(0, 6);
    let emergency = ProgressiveBreakOpportunity::new(ProgressiveBreakTier::Emergency, span);
    let whitespace = ProgressiveBreakOpportunity::new(ProgressiveBreakTier::Whitespace, span);
    let promoted = fill(&lines, &natural, 48.0, &[], 1.0, None, &HashSet::new(), &[], &HashSet::new(), &HashMap::from([(2, emergency), (3, whitespace)]));
    let RepairOption::PushIn { reason, .. } = promoted[0].repair.as_ref().unwrap() else { panic!() };
    assert_eq!("ProgressiveTechnicalTierPromotion", reason.split(':').next().unwrap());
    let degraded = fill(&lines, &natural, 48.0, &[], 1.0, None, &HashSet::new(), &[], &HashSet::new(), &HashMap::from([(2, whitespace), (3, emergency)]));
    assert_eq!(None, degraded[0].repair);
    let short_promotion = fill(
        &[
            line(IntRange::new(0, 1), &natural, &natural, LineEndReason::AutoWrap, None, HashSet::new()),
            line(IntRange::new(2, 3), &natural, &natural, LineEndReason::AutoWrap, None, HashSet::new()),
        ],
        &natural, 64.0, &[], 1.0, None, &HashSet::new(), &[], &HashSet::new(), &HashMap::from([(2, emergency), (3, whitespace)]),
    );
    assert_eq!(None, short_promotion[0].repair);
    let empty_search = fill(
        &[
            line(IntRange::new(0, 1), &natural, &natural, LineEndReason::AutoWrap, None, HashSet::new()),
            line(IntRange::new(2, 3), &natural, &natural, LineEndReason::AutoWrap, None, HashSet::new()),
        ],
        &natural, 96.0, &[], 1.0, Some(&HashSet::from([3])), &HashSet::new(), &[], &HashSet::new(), &HashMap::from([(2, emergency), (4, whitespace)]),
    );
    assert_eq!(IntRange::new(0, 1), empty_search[0].cluster_range);
    let refill = fill(&lines, &natural, 96.0, &[], 1.0, None, &HashSet::new(), &[], &HashSet::new(), &HashMap::from([
        (2, emergency),
        (3, whitespace),
        (4, ProgressiveBreakOpportunity::new(ProgressiveBreakTier::Whitespace, TextRange::new(0, 6))),
        (5, ProgressiveBreakOpportunity::new(ProgressiveBreakTier::Structural, TextRange::new(0, 3))),
        (6, emergency),
    ]));
    assert_eq!(IntRange::new(0, 5), refill[0].cluster_range);
    let RepairOption::PushIn { reason, .. } = refill[0].repair.as_ref().unwrap() else { panic!() };
    assert_eq!("LineAdjustmentPushIn", reason.split(':').next().unwrap());
}

#[test]
fn with_fill_push_in_gate_applies_or_returns_the_solution() {
    let natural = [clusters("AAAA", 0), clusters("BBBB", 4)].concat();
    let solution = LineSolution::new(vec![
        line(IntRange::new(0, 3), &natural, &natural, LineEndReason::AutoWrap, None, HashSet::new()),
        line(IntRange::new(4, 7), &natural, &natural, LineEndReason::AutoWrap, None, HashSet::new()),
    ]);
    let unbreakable = UnbreakableRanges::default();
    let disabled = with_fill_push_in(solution.clone(), false, &natural, &natural, 80.0, &[], 0.0, 1.0 / 3.0, None, &HashSet::new(), &unbreakable, 10, &HashSet::new(), &HashMap::new());
    assert_eq!(solution, disabled);
    let enabled = with_fill_push_in(solution, true, &natural, &natural, 80.0, &[], 0.0, 1.0 / 3.0, None, &HashSet::new(), &unbreakable, 10, &HashSet::new(), &HashMap::new());
    assert_eq!(IntRange::new(0, 4), enabled.lines[0].cluster_range);
}

#[test]
fn fill_pull_across_different_technical_spans_skips_tier_comparisons() {
    let natural = clusters("XXXXXXXX", 0);
    let result = fill(
        &[
            line(IntRange::new(0, 3), &natural, &natural, LineEndReason::AutoWrap, None, HashSet::new()),
            line(IntRange::new(4, 7), &natural, &natural, LineEndReason::AutoWrap, None, HashSet::new()),
        ],
        &natural, 100.0, &[], 1.0, None, &HashSet::new(), &[IntRange::new(4, 5)], &HashSet::new(),
        &HashMap::from([
            (4, ProgressiveBreakOpportunity::new(ProgressiveBreakTier::Structural, TextRange::new(0, 4))),
            (6, ProgressiveBreakOpportunity::new(ProgressiveBreakTier::Syllable, TextRange::new(4, 8))),
        ]),
    );
    assert_eq!(IntRange::new(0, 5), result[0].cluster_range);
    assert_eq!(IntRange::new(6, 7), result[1].cluster_range);
}
