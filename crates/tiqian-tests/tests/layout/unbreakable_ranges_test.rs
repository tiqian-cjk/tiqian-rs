use tiqian::core::int_range::IntRange;
use tiqian::layout::progressive_break_decisions::{
    UnbreakableRanges, adjust_break_for_unbreakables,
};

#[test]
fn contains_boundary_handles_unsorted_nested_ranges() {
    let ranges = UnbreakableRanges::new(vec![
        IntRange::new(8, 10),
        IntRange::new(1, 4),
        IntRange::new(3, 7),
    ]);

    assert!(!ranges.contains_boundary(1));
    assert!(ranges.contains_boundary(2));
    assert!(ranges.contains_boundary(4));
    assert!(ranges.contains_boundary(5));
    assert!(ranges.contains_boundary(7));
    assert!(!ranges.contains_boundary(8));
    assert!(ranges.contains_boundary(9));
    assert!(!ranges.contains_boundary(11));
}

#[test]
fn containing_queries_keep_original_range_priority() {
    let ranges = UnbreakableRanges::new(vec![
        IntRange::new(3, 8),
        IntRange::new(1, 6),
        IntRange::new(4, 5),
    ]);

    assert_eq!(Some(IntRange::new(3, 8)), ranges.containing_or_null(5));
    assert_eq!(
        Some(IntRange::new(3, 8)),
        ranges.containing_from_closed_start_or_null(4),
    );
}

#[test]
fn adjust_break_reaches_the_same_fixed_point_for_unsorted_ranges() {
    let ranges = UnbreakableRanges::new(vec![
        IntRange::new(3, 5),
        IntRange::new(1, 3),
        IntRange::new(2, 4),
    ]);

    assert_eq!(1, adjust_break_for_unbreakables(5, 0, &ranges));
    assert_eq!(5, adjust_break_for_unbreakables(5, 1, &ranges));
}
