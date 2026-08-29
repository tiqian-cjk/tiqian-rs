use std::collections::HashSet;

use tiqian::org::tiqian::core::Geometry::TextRange;
use tiqian::org::tiqian::core::IntRange::IntRange;
use tiqian::org::tiqian::core::LayoutModel::{Cluster, LineEndReason};
use tiqian::org::tiqian::core::Text::Text;
use tiqian::org::tiqian::layout::LineBreaker::{LineBreaker, LineBreakerConfig};
use tiqian::org::tiqian::layout::LineOptimization::RepairOption;
use tiqian::org::tiqian::layout::ParagraphDpLineBreaker::ParagraphDpLineBreaker;
use tiqian::org::tiqian::layout::ProgressiveBreakDecisions::{ShrinkChannel, ShrinkOpportunity};

fn cluster(index: i32, text: &str, advance: f32) -> Cluster {
    Cluster::new(
        TextRange::new(index, index + 1),
        Text::from(text),
        "test".to_owned(),
        advance,
    )
}

fn han_clusters(count: i32) -> Vec<Cluster> {
    (0..count).map(|index| cluster(index, "中", 16.0)).collect()
}

fn break_lines(
    clusters: &[Cluster],
    max_width: f32,
    config: LineBreakerConfig,
) -> tiqian::org::tiqian::layout::LineOptimization::LineSolution {
    ParagraphDpLineBreaker::default().break_lines(clusters, clusters, max_width, &config)
}

fn assert_tiles(
    solution: &tiqian::org::tiqian::layout::LineOptimization::LineSolution,
    cluster_count: i32,
) {
    let mut expected = 0;
    for line in &solution.lines {
        if line.cluster_range.is_empty() {
            continue;
        }
        assert_eq!(
            expected,
            line.cluster_range.first(),
            "lines must tile clusters in order"
        );
        expected = line.cluster_range.last() + 1;
    }
    assert_eq!(cluster_count, expected, "lines must cover every cluster");
}

#[test]
fn tiles_all_clusters_in_order() {
    let clusters = han_clusters(23);
    let solution = break_lines(&clusters, 100.0, LineBreakerConfig::default());

    assert_tiles(&solution, 23);
    assert_eq!(
        LineEndReason::ParagraphEnd,
        solution.lines.last().unwrap().end_reason
    );
}

#[test]
fn single_line_when_everything_fits() {
    let clusters = han_clusters(4);
    let solution = break_lines(&clusters, 400.0, LineBreakerConfig::default());

    assert_eq!(1, solution.lines.len());
    assert_eq!(LineEndReason::ParagraphEnd, solution.lines[0].end_reason);
}

#[test]
fn mandatory_break_binds_control_to_previous_line() {
    let clusters = vec![
        cluster(0, "中", 16.0),
        cluster(1, "中", 16.0),
        cluster(2, "\n", 0.0),
        cluster(3, "中", 16.0),
        cluster(4, "中", 16.0),
    ];
    let mut config = LineBreakerConfig::default();
    config.hard_break_after_clusters = HashSet::from([2]);
    let solution = break_lines(&clusters, 200.0, config);

    assert_tiles(&solution, 5);
    assert_eq!(2, solution.lines.len());
    assert_eq!(LineEndReason::MandatoryBreak, solution.lines[0].end_reason);
    assert_eq!(IntRange::new(0, 2), solution.lines[0].cluster_range);
    assert_eq!(LineEndReason::ParagraphEnd, solution.lines[1].end_reason);
}

#[test]
fn trailing_mandatory_break_emits_paragraph_end_line() {
    let clusters = vec![cluster(0, "中", 16.0), cluster(1, "\n", 0.0)];
    let mut config = LineBreakerConfig::default();
    config.hard_break_after_clusters = HashSet::from([1]);
    let solution = break_lines(&clusters, 200.0, config);

    assert_eq!(LineEndReason::MandatoryBreak, solution.lines[0].end_reason);
    assert_eq!(LineEndReason::ParagraphEnd, solution.lines[1].end_reason);
    assert!(solution.lines[1].cluster_range.is_empty());
}

#[test]
fn never_breaks_inside_unbreakable_range() {
    let clusters = han_clusters(10);
    let mut config = LineBreakerConfig::default();
    config.unbreakable_ranges = vec![IntRange::new(3, 6)];
    let solution = break_lines(&clusters, 64.0, config);

    assert_tiles(&solution, 10);
    for line in &solution.lines {
        assert!(
            !(4..=6).contains(&line.cluster_range.first()),
            "break inside unbreakable range: {:?}",
            line.cluster_range
        );
    }
}

#[test]
fn kinsoku_avoidance_routes_around_forbidden_line_start() {
    let mut clusters = han_clusters(7);
    clusters[6] = cluster(6, "。", 16.0);
    let mut config = LineBreakerConfig::default();
    config.forbidden_line_start_clusters = Some(HashSet::from([6]));
    let solution = break_lines(&clusters, 48.0, config);

    assert_tiles(&solution, 7);
    for line in &solution.lines {
        assert!(
            line.cluster_range.first() != 6 || line.repair.is_some(),
            "。 must not start a line without recorded repair"
        );
    }
}

#[test]
fn compression_edge_records_push_in_repair_only_when_enabled() {
    let clusters = vec![
        cluster(0, "中", 16.0),
        cluster(1, "中", 16.0),
        cluster(2, "中", 16.0),
        cluster(3, "，", 16.0),
        cluster(4, "中", 16.0),
        cluster(5, "中", 16.0),
        cluster(6, "中", 16.0),
    ];
    let opportunity = ShrinkOpportunity::new(3, 5, 8.0, ShrinkChannel::TrailingGlue);
    let mut enabled = LineBreakerConfig::default();
    enabled.shrink_opportunities = vec![opportunity];
    enabled.line_adjustment_push_in = true;
    let compressed = break_lines(&clusters, 56.0, enabled);
    assert_tiles(&compressed, 7);
    let line = compressed
        .lines
        .iter()
        .find(|line| matches!(line.repair, Some(RepairOption::PushIn { .. })))
        .expect("expected a PushIn compressed line");
    assert_eq!(IntRange::new(0, 3), line.cluster_range);
    assert!(line.adjusted_width <= 56.01);
    let Some(RepairOption::PushIn { reason, .. }) = &line.repair else {
        unreachable!()
    };
    assert!(reason.starts_with("LineAdjustmentPushIn"));

    let mut disabled = LineBreakerConfig::default();
    disabled.shrink_opportunities = vec![opportunity];
    let without_compression = break_lines(&clusters, 56.0, disabled);
    assert!(without_compression.lines.iter().all(|line| !matches!(&line.repair, Some(RepairOption::PushIn { reason, .. }) if reason.starts_with("LineAdjustmentPushIn"))));
}

#[test]
fn overwide_single_cluster_still_progresses() {
    let clusters = vec![
        cluster(0, "中", 16.0),
        cluster(1, "Ｗ", 300.0),
        cluster(2, "中", 16.0),
    ];
    let solution = break_lines(&clusters, 48.0, LineBreakerConfig::default());
    assert_tiles(&solution, 3);
}
