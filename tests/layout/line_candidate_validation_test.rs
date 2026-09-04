use tiqian::common::HashSet;
use tiqian::core::geometry::{text_range};
use tiqian::core::int_range::IntRange;
use tiqian::layout::line_optimization::LineCandidate;

fn candidate(hanging: HashSet<i32>, range: IntRange) -> LineCandidate {
    let mut candidate = LineCandidate::new(range, text_range(0, 4), 64.0, 64.0);
    candidate.hanging_cluster_indices = hanging;
    candidate.validate_hanging_suffix();
    candidate
}

#[test]
fn hanging_below_line_range_is_rejected() {
    let error = std::panic::catch_unwind(|| candidate(HashSet::from([-1, 3]), IntRange::new(0, 3)))
        .expect_err("expected hanging suffix rejection");
    let message = error
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| error.downcast_ref::<&str>().copied())
        .expect("panic message");
    assert_eq!(
        "Hanging clusters must be a trailing line suffix: line=IntRange { start: 0, end_inclusive: 3 } hanging={-1, 3}",
        message
    );
}

#[test]
#[should_panic(expected = "Hanging clusters must be a trailing line suffix")]
fn hanging_entirely_above_line_is_rejected() {
    candidate(HashSet::from([5, 6]), IntRange::new(0, 3));
}

#[test]
#[should_panic(expected = "Hanging clusters must be a trailing line suffix")]
fn hanging_above_line_last_is_rejected() {
    candidate(HashSet::from([1, 4]), IntRange::new(0, 3));
}

#[test]
#[should_panic(expected = "Hanging clusters must be contiguous")]
fn non_contiguous_hanging_is_rejected() {
    candidate(HashSet::from([0, 2, 3]), IntRange::new(0, 3));
}

#[test]
fn in_measure_range_excludes_hanging_suffix() {
    assert_eq!(
        IntRange::new(0, 1),
        candidate(HashSet::from([2, 3]), IntRange::new(0, 3)).in_measure_cluster_range()
    );
}

#[test]
fn in_measure_range_is_full_line_without_hanging() {
    assert_eq!(
        IntRange::new(0, 3),
        candidate(HashSet::new(), IntRange::new(0, 3)).in_measure_cluster_range()
    );
}
