use tiqian::org::tiqian::core::Geometry::TextRange;
use tiqian::org::tiqian::core::IntRange::IntRange;
use tiqian::org::tiqian::core::LayoutModel::Cluster;
use tiqian::org::tiqian::core::Text::Text;
use tiqian::org::tiqian::layout::LineBreaker::{GreedyLineBreaker, LineBreaker, LineBreakerConfig};
use tiqian::org::tiqian::layout::LineOptimization::RepairOption;
use tiqian::org::tiqian::layout::ProgressiveBreakDecisions::{ShrinkChannel, ShrinkOpportunity};

fn cluster(index: i32, text: &str) -> Cluster {
    Cluster::new(
        TextRange::new(index, index + 1),
        Text::from(text),
        "test".to_owned(),
        16.0,
    )
}

fn punctuation_line() -> Vec<Cluster> {
    let mut clusters: Vec<_> = (0..5).map(|index| cluster(index, "中")).collect();
    clusters.push(cluster(5, "、"));
    clusters.extend((6..10).map(|index| cluster(index, "文")));
    clusters.push(cluster(10, "。"));
    clusters
}

fn break_lines(
    clusters: &[Cluster],
    opportunities: Vec<ShrinkOpportunity>,
    max_width: f32,
) -> tiqian::org::tiqian::layout::LineOptimization::LineSolution {
    let mut config = LineBreakerConfig::default();
    config.shrink_opportunities = opportunities;
    GreedyLineBreaker::default().break_lines(clusters, clusters, max_width, &config)
}

#[test]
fn push_in_aggregates_shrink_from_multiple_preceding_clusters() {
    let clusters = punctuation_line();
    let solution = break_lines(
        &clusters,
        vec![
            ShrinkOpportunity::new(5, 6, 8.0, ShrinkChannel::TrailingGlue),
            ShrinkOpportunity::new(10, 6, 8.0, ShrinkChannel::TrailingGlue),
        ],
        160.0,
    );

    assert_eq!(1, solution.lines.len());
    let line = &solution.lines[0];
    assert_eq!(IntRange::new(0, 10), line.cluster_range);
    assert_eq!(160.0, line.adjusted_width);
    let Some(RepairOption::PushIn {
        offender_cluster_index,
        total_shrink,
        total_available_capacity,
        allocations,
        ..
    }) = &line.repair
    else {
        panic!("expected PushIn repair")
    };
    assert_eq!(10, *offender_cluster_index);
    assert_eq!(16.0, *total_shrink);
    assert_eq!(16.0, *total_available_capacity);
    assert_eq!(
        vec![10, 5],
        allocations
            .iter()
            .map(|allocation| allocation.cluster_index)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        vec![8.0, 8.0],
        allocations
            .iter()
            .map(|allocation| allocation.shrink)
            .collect::<Vec<_>>()
    );
}

#[test]
fn push_in_rejects_when_line_wide_capacity_is_insufficient() {
    let clusters = punctuation_line();
    let solution = break_lines(
        &clusters,
        vec![ShrinkOpportunity::new(
            5,
            6,
            4.0,
            ShrinkChannel::TrailingGlue,
        )],
        160.0,
    );

    assert_eq!(2, solution.lines.len());
    assert_eq!(IntRange::new(0, 8), solution.lines[0].cluster_range);
    assert_eq!(IntRange::new(9, 10), solution.lines[1].cluster_range);
    assert!(matches!(
        solution.lines[1].repair,
        Some(RepairOption::CarryPrevious { .. })
    ));
    let rejected = solution.lines[1]
        .repair_candidates
        .iter()
        .find(|candidate| candidate.kind == "PushIn")
        .unwrap();
    assert!(!rejected.accepted);
    assert_eq!(
        Some("insufficient-capacity".to_owned()),
        rejected.rejection_reason
    );
    assert_eq!(4.0, rejected.available_capacity);
    assert_eq!(16.0, rejected.required_shrink);
}

#[test]
fn offender_only_capacity_remains_supported() {
    let clusters = vec![
        cluster(0, "中"),
        cluster(1, "文"),
        cluster(2, "中"),
        cluster(3, "。"),
    ];
    let solution = break_lines(
        &clusters,
        vec![ShrinkOpportunity::new(
            3,
            6,
            4.0,
            ShrinkChannel::TrailingGlue,
        )],
        60.0,
    );

    assert_eq!(1, solution.lines.len());
    let Some(RepairOption::PushIn {
        allocations,
        total_shrink,
        ..
    }) = &solution.lines[0].repair
    else {
        panic!("expected PushIn repair")
    };
    assert_eq!(
        vec![3],
        allocations
            .iter()
            .map(|allocation| allocation.cluster_index)
            .collect::<Vec<_>>()
    );
    assert_eq!(4.0, *total_shrink);
}

#[test]
fn push_in_merges_an_offender_that_fits_after_chained_repairs() {
    let clusters = vec![
        cluster(0, "中"),
        cluster(1, "中"),
        cluster(2, "中"),
        cluster(3, "」"),
        cluster(4, "。"),
        cluster(5, "中"),
        cluster(6, "中"),
        cluster(7, "中"),
        cluster(8, "、"),
        cluster(9, "中"),
    ];
    let solution = break_lines(
        &clusters,
        vec![
            ShrinkOpportunity::new(3, 6, 8.0, ShrinkChannel::TrailingGlue),
            ShrinkOpportunity::new(4, 6, 8.0, ShrinkChannel::TrailingGlue),
            ShrinkOpportunity::new(8, 6, 8.0, ShrinkChannel::TrailingGlue),
        ],
        64.0,
    );

    assert_eq!(3, solution.lines.len());
    assert_eq!(IntRange::new(0, 4), solution.lines[0].cluster_range);
    assert_eq!(IntRange::new(5, 8), solution.lines[1].cluster_range);
    assert_eq!(IntRange::new(9, 9), solution.lines[2].cluster_range);
    assert!(
        solution
            .lines
            .iter()
            .all(|line| clusters[line.cluster_range.first() as usize].text == "中")
    );
    let Some(RepairOption::PushIn {
        total_shrink,
        reason,
        ..
    }) = &solution.lines[1].repair
    else {
        panic!("expected zero-shrink PushIn repair")
    };
    assert_eq!(0.0, *total_shrink);
    assert!(reason.ends_with("fits-no-shrink"));
    assert_eq!(64.0, solution.lines[1].adjusted_width);
}

#[test]
fn carry_previous_refuses_to_split_an_unbreakable_span() {
    let clusters = vec![
        cluster(0, "中"),
        cluster(1, "中"),
        cluster(2, "王"),
        cluster(3, "小"),
        cluster(4, "明"),
        cluster(5, "。"),
    ];
    let mut config = LineBreakerConfig::default();
    config.shrink_opportunities = vec![ShrinkOpportunity::new(
        5,
        6,
        8.0,
        ShrinkChannel::TrailingGlue,
    )];
    config.unbreakable_ranges = vec![IntRange::new(2, 4)];
    let solution = GreedyLineBreaker::default().break_lines(&clusters, &clusters, 80.0, &config);

    assert_eq!(2, solution.lines.len());
    let Some(RepairOption::LeaveRagged { reason, .. }) = &solution.lines[1].repair else {
        panic!("expected LeaveRagged repair")
    };
    assert!(reason.ends_with("carry-would-split-mourning-span"));
    let carry = solution.lines[1]
        .repair_candidates
        .iter()
        .find(|candidate| candidate.kind == "CarryPrevious")
        .unwrap();
    assert_eq!(
        Some("carry-would-split-mourning-span".to_owned()),
        carry.rejection_reason
    );
}
