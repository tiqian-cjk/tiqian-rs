use std::collections::HashSet;

use tiqian::org::tiqian::core::Geometry::TextRange;
use tiqian::org::tiqian::core::IntRange::IntRange;
use tiqian::org::tiqian::core::LayoutModel::{Cluster, LineEndReason};
use tiqian::org::tiqian::layout::LineBreaker::{GreedyLineBreaker, LineBreaker, LineBreakerConfig};
use tiqian::org::tiqian::layout::LineOptimization::RepairOption;
use tiqian::org::tiqian::layout::ProgressiveBreakDecisions::{ShrinkChannel, ShrinkOpportunity};

fn cluster(start: i32, end: i32, text: &str, advance: f32) -> Cluster {
    Cluster::new(TextRange::new(start, end), text.to_owned(), "test".to_owned(), advance)
}

fn break_lines(clusters: &[Cluster], max_width: f32, config: LineBreakerConfig) -> tiqian::org::tiqian::layout::LineOptimization::LineSolution {
    GreedyLineBreaker::default().break_lines(clusters, clusters, max_width, &config)
}

#[test]
fn empty_input_produces_no_lines() {
    let solution = break_lines(&[], 100.0, LineBreakerConfig::default());
    assert!(solution.lines.is_empty());
}

#[test]
fn fills_line_until_overflow_then_starts_new_line() {
    let clusters: Vec<_> = (0..5).map(|index| cluster(index, index + 1, "x", 16.0)).collect();
    let solution = break_lines(&clusters, 48.0, LineBreakerConfig::default());

    assert_eq!(2, solution.lines.len());
    assert_eq!(IntRange::new(0, 2), solution.lines[0].cluster_range);
    assert_eq!(48.0, solution.lines[0].adjusted_width);
    assert_eq!(IntRange::new(3, 4), solution.lines[1].cluster_range);
    assert_eq!(32.0, solution.lines[1].adjusted_width);
}

#[test]
fn natural_and_adjusted_widths_track_independently() {
    let natural = vec![cluster(0, 1, "，", 16.0), cluster(1, 2, "。", 16.0)];
    let adjusted = vec![cluster(0, 1, "，", 16.0), cluster(1, 2, "。", 12.0)];
    let solution = GreedyLineBreaker::default().break_lines(&natural, &adjusted, 64.0, &LineBreakerConfig::default());

    assert_eq!(32.0, solution.lines[0].natural_width);
    assert_eq!(28.0, solution.lines[0].adjusted_width);
}

#[test]
fn cluster_wider_than_max_width_gets_its_own_line_rather_than_stalling() {
    let clusters = vec![cluster(0, 1, "中", 16.0), cluster(1, 8, "English", 112.0)];
    let solution = break_lines(&clusters, 80.0, LineBreakerConfig::default());

    assert_eq!(2, solution.lines.len());
    assert_eq!(IntRange::new(0, 0), solution.lines[0].cluster_range);
    assert_eq!(IntRange::new(1, 1), solution.lines[1].cluster_range);
    assert_eq!(112.0, solution.lines[1].adjusted_width);
}

#[test]
fn kinsoku_carries_previous_when_push_in_capacity_cannot_cover_overflow() {
    let clusters = vec![
        cluster(0, 1, "a", 16.0), cluster(1, 2, "b", 16.0),
        cluster(2, 3, "c", 16.0), cluster(3, 4, "。", 16.0),
    ];
    let mut config = LineBreakerConfig::default();
    config.shrink_opportunities = vec![ShrinkOpportunity::new(3, 6, 4.0, ShrinkChannel::TrailingGlue)];
    let solution = break_lines(&clusters, 59.0, config);

    assert_eq!(2, solution.lines.len());
    assert_eq!(IntRange::new(0, 1), solution.lines[0].cluster_range);
    assert_eq!(IntRange::new(2, 3), solution.lines[1].cluster_range);
    assert!(matches!(solution.lines[1].repair, Some(RepairOption::CarryPrevious { .. })));
    assert_eq!(2, solution.lines[1].repair_candidates.len());
    assert_eq!("PushIn", solution.lines[1].repair_candidates[0].kind);
    assert!(!solution.lines[1].repair_candidates[0].accepted);
    assert_eq!(Some("insufficient-capacity".to_owned()), solution.lines[1].repair_candidates[0].rejection_reason);
    assert_eq!("CarryPrevious", solution.lines[1].repair_candidates[1].kind);
    assert!(solution.lines[1].repair_candidates[1].accepted);
}

#[test]
fn kinsoku_pushes_forbidden_punctuation_into_previous_line_when_glue_covers_overflow() {
    let clusters = vec![
        cluster(0, 1, "a", 16.0), cluster(1, 2, "b", 16.0),
        cluster(2, 3, "c", 16.0), cluster(3, 4, "。", 16.0),
    ];
    let mut config = LineBreakerConfig::default();
    config.shrink_opportunities = vec![ShrinkOpportunity::new(3, 6, 4.0, ShrinkChannel::TrailingGlue)];
    let solution = break_lines(&clusters, 60.0, config);

    assert_eq!(1, solution.lines.len());
    let line = &solution.lines[0];
    assert_eq!(IntRange::new(0, 3), line.cluster_range);
    assert_eq!(64.0, line.natural_width);
    assert_eq!(60.0, line.adjusted_width);
    let Some(RepairOption::PushIn { offender_cluster_index, total_shrink, total_available_capacity, allocations, .. }) = &line.repair else { panic!("expected PushIn repair") };
    assert_eq!(3, *offender_cluster_index);
    assert_eq!(4.0, *total_shrink);
    assert_eq!(4.0, *total_available_capacity);
    assert_eq!(vec![3], allocations.iter().map(|allocation| allocation.cluster_index).collect::<Vec<_>>());
    assert_eq!(2.0, solution.total_badness);
}

#[test]
fn leave_ragged_when_carry_previous_would_overflow() {
    let clusters = vec![
        cluster(0, 1, "a", 16.0), cluster(1, 2, "b", 16.0), cluster(2, 3, "c", 16.0), cluster(3, 4, "d", 16.0),
        cluster(4, 5, "。", 16.0), cluster(5, 6, "e", 16.0), cluster(6, 7, "f", 16.0), cluster(7, 8, "g", 16.0),
    ];
    let solution = break_lines(&clusters, 64.0, LineBreakerConfig::default());

    assert_eq!(IntRange::new(4, 7), solution.lines[1].cluster_range);
    assert!(matches!(solution.lines[1].repair, Some(RepairOption::LeaveRagged { .. })));
    assert_eq!(3, solution.lines[1].repair_candidates.len());
    assert_eq!(Some("carry-overflows".to_owned()), solution.lines[1].repair_candidates[1].rejection_reason);
    assert_eq!(20.0, solution.total_badness);
}

#[test]
fn hangs_pause_stop_past_measure_when_enabled_and_push_in_cannot_fit() {
    let clusters = vec![
        cluster(0, 1, "a", 16.0), cluster(1, 2, "b", 16.0), cluster(2, 3, "c", 16.0),
        cluster(3, 4, "d", 16.0), cluster(4, 5, "。", 16.0),
    ];
    let mut config = LineBreakerConfig::default();
    config.hangable_clusters = HashSet::from([4]);
    let solution = break_lines(&clusters, 64.0, config);

    assert_eq!(1, solution.lines.len());
    assert_eq!(IntRange::new(0, 4), solution.lines[0].cluster_range);
    assert_eq!(Some(4), solution.lines[0].hanging_cluster_index());
    assert_eq!(64.0, solution.lines[0].adjusted_width);
    assert!(matches!(solution.lines[0].repair, Some(RepairOption::Hang { .. })));
}

#[test]
fn retreats_break_so_line_does_not_end_on_opening_mark() {
    let clusters = vec![
        cluster(0, 1, "中", 16.0), cluster(1, 2, "中", 16.0), cluster(2, 3, "（", 16.0),
        cluster(3, 4, "中", 16.0), cluster(4, 5, "中", 16.0),
    ];
    let mut config = LineBreakerConfig::default();
    config.forbidden_line_end_clusters = HashSet::from([2]);
    let solution = break_lines(&clusters, 48.0, config);

    assert_eq!(IntRange::new(0, 1), solution.lines[0].cluster_range);
    assert_eq!(IntRange::new(2, 4), solution.lines[1].cluster_range);
    assert!(matches!(solution.lines[0].repair, Some(RepairOption::CarryNext { moved_cluster_index: 2, .. })));
}

#[test]
fn mandatory_break_closes_line_and_preserves_trailing_empty_line() {
    let clusters = vec![cluster(0, 1, "中", 16.0), cluster(1, 2, "\n", 0.0)];
    let mut config = LineBreakerConfig::default();
    config.hard_break_after_clusters = HashSet::from([1]);
    let solution = break_lines(&clusters, 160.0, config);

    assert_eq!(2, solution.lines.len());
    assert_eq!(IntRange::new(0, 1), solution.lines[0].cluster_range);
    assert_eq!(LineEndReason::MandatoryBreak, solution.lines[0].end_reason);
    assert_eq!(TextRange::new(2, 2), solution.lines[1].source_range);
    assert_eq!(LineEndReason::ParagraphEnd, solution.lines[1].end_reason);
}
