use tiqian::common::HashSet;
use tiqian::core::geometry::{text_range};
use tiqian::core::int_range::IntRange;
use tiqian::layout::line_optimization::{
    BreakCandidate, LineCandidate, LineOptimizationStrategy, LineSolution, RepairCandidate,
    RepairOption,
};
use tiqian::linebreak::line_break::BreakKind;

fn line_with_hanging(hanging_cluster_indices: HashSet<i32>) -> LineCandidate {
    let mut line = LineCandidate::new(IntRange::new(0, 4), text_range(0, 5), 80.0, 80.0);
    line.hanging_cluster_indices = hanging_cluster_indices;
    line.validate_hanging_suffix();
    line
}

#[test]
fn break_candidate_defaults_are_usable() {
    let candidate = BreakCandidate {
        index: 3,
        kind: BreakKind::Allowed,
        natural_width: 16.0,
        compressed_width: 14.0,
        expanded_width: 18.0,
        forbidden_reason: None,
        repair_options: Vec::new(),
    };

    assert_eq!(None, candidate.forbidden_reason);
    assert!(candidate.repair_options.is_empty());
}

#[test]
fn break_candidate_carries_explicit_forbidden_reason_and_repairs() {
    let repair = RepairOption::LeaveRagged {
        penalty: 30,
        reason: "ForbiddenAtLineStart:，:leave-ragged".to_owned(),
        offender_cluster_index: 3,
    };
    let candidate = BreakCandidate {
        index: 2,
        kind: BreakKind::Problematic,
        natural_width: 32.0,
        compressed_width: 28.0,
        expanded_width: 36.0,
        forbidden_reason: Some("kinsoku".to_owned()),
        repair_options: vec![repair.clone()],
    };

    assert_eq!(Some("kinsoku"), candidate.forbidden_reason.as_deref());
    assert_eq!(vec![repair], candidate.repair_options);
}

#[test]
fn line_candidate_rejects_hanging_that_is_not_a_trailing_suffix() {
    for hanging in [HashSet::from([0, 1]), HashSet::from([2, 3]), HashSet::from([7])] {
        assert!(
            std::panic::catch_unwind(|| line_with_hanging(hanging)).is_err(),
            "expected hanging suffix rejection"
        );
    }
}

#[test]
fn line_candidate_rejects_discontiguous_hanging() {
    assert!(std::panic::catch_unwind(|| line_with_hanging(HashSet::from([2, 4]))).is_err());
}

#[test]
fn line_candidate_accepts_a_contiguous_trailing_hanging_suffix() {
    let line = line_with_hanging(HashSet::from([3, 4]));
    assert_eq!(HashSet::from([3, 4]), line.hanging_cluster_indices);
}

#[test]
fn hanging_cluster_index_prefers_the_hang_offender_over_the_suffix_end() {
    let mut with_repair = line_with_hanging(HashSet::from([3, 4]));
    with_repair.repair = Some(RepairOption::Hang {
        penalty: 5,
        reason: "ForbiddenAtLineStart:，:hang".to_owned(),
        offender_cluster_index: 3,
    });
    assert_eq!(Some(3), with_repair.hanging_cluster_index());

    let without_repair = line_with_hanging(HashSet::from([3, 4]));
    assert_eq!(Some(4), without_repair.hanging_cluster_index());
}

#[test]
fn in_measure_cluster_range_excludes_the_hanging_suffix() {
    let hanging = line_with_hanging(HashSet::from([3, 4]));
    assert_eq!(IntRange::new(0, 2), hanging.in_measure_cluster_range());

    let plain = LineCandidate::new(IntRange::new(0, 4), text_range(0, 5), 80.0, 80.0);
    assert_eq!(IntRange::new(0, 4), plain.in_measure_cluster_range());
}

#[test]
fn carry_next_records_the_moved_mark() {
    let carry_next = RepairOption::CarryNext {
        penalty: 15,
        reason: "ForbiddenAtLineEnd:“:carry-next".to_owned(),
        moved_cluster_index: 4,
    };

    assert_eq!(15, carry_next.penalty());
    assert_eq!("ForbiddenAtLineEnd:“:carry-next", carry_next.reason());
    assert!(matches!(carry_next, RepairOption::CarryNext { moved_cluster_index: 4, .. }));
}

#[test]
fn repair_candidate_defaults_are_usable() {
    let candidate = RepairCandidate::new(
        "PushIn".to_owned(),
        "ForbiddenAtLineStart".to_owned(),
        4,
        10,
        true,
    );

    assert_eq!(None, candidate.rejection_reason);
    assert_eq!(None, candidate.target_cluster_index);
    assert_eq!(None, candidate.carried_cluster_index);
    assert_eq!(0.0, candidate.shrink);
    assert_eq!(0.0, candidate.required_shrink);
    assert_eq!(0.0, candidate.available_capacity);
}

#[test]
fn line_solution_defaults_to_zero_badness() {
    let solution = LineSolution::new(Vec::new());
    assert_eq!(0.0, solution.total_badness);
}

#[test]
fn optimization_strategy_enumerates_all_three_strategies() {
    assert_eq!("Greedy", format!("{:?}", LineOptimizationStrategy::Greedy));
    assert_eq!("Lookahead", format!("{:?}", LineOptimizationStrategy::Lookahead));
    assert_eq!(
        "ParagraphDynamicProgramming",
        format!("{:?}", LineOptimizationStrategy::ParagraphDynamicProgramming)
    );
}
