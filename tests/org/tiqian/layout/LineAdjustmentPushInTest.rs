use tiqian::common::{HashMap, HashSet};

use tiqian::core::Geometry::TextRange;
use tiqian::core::IntRange::IntRange;
use tiqian::core::LayoutModel::Cluster;
use tiqian::core::Text::Text;
use tiqian::layout::LineBreaker::rebuild_line;
use tiqian::layout::LineOptimization::{LineCandidate, RepairOption};
use tiqian::layout::LineRepair::apply_fill_push_in;
use tiqian::layout::ProgressiveBreakDecisions::{
    ProgressiveBreakOpportunity, ProgressiveBreakTier, ShrinkChannel, ShrinkOpportunity,
};

fn cluster(index: i32, text: &str, advance: f32) -> Cluster {
    Cluster::new(
        TextRange::new(index, index + 1),
        Text::from(text),
        "test".to_owned(),
        advance,
    )
}

fn lines(clusters: &[Cluster], ranges: &[(i32, i32)]) -> Vec<LineCandidate> {
    ranges
        .iter()
        .map(|(first, last)| {
            rebuild_line(
                IntRange::new(*first, *last),
                clusters,
                clusters,
                tiqian::core::LayoutModel::LineEndReason::AutoWrap,
                None,
                Vec::new(),
            )
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn fill(
    lines: &[LineCandidate],
    clusters: &[Cluster],
    max_width: f32,
    opportunities: &[ShrinkOpportunity],
    forbidden_start: Option<&HashSet<i32>>,
    forbidden_end: &HashSet<i32>,
    progressive: &HashMap<i32, ProgressiveBreakOpportunity>,
) -> Vec<LineCandidate> {
    apply_fill_push_in(
        lines,
        clusters,
        clusters,
        max_width,
        opportunities,
        0.0,
        1_000_000.0,
        forbidden_start,
        forbidden_end,
        &[],
        2,
        &HashSet::from([0, 1, 2, 3, 4]),
        progressive,
    )
}

fn push_in(line: &LineCandidate) -> &RepairOption {
    line.repair.as_ref().expect("expected PushIn repair")
}

#[test]
fn no_shrink_push_in_continues_until_line_is_no_longer_loose() {
    let clusters = vec![
        cluster(0, "甲", 30.0),
        cluster(1, "乙", 30.0),
        cluster(2, "丙", 20.0),
        cluster(3, "丁", 20.0),
        cluster(4, "戊", 20.0),
        cluster(5, "己", 20.0),
    ];
    let output = fill(
        &lines(&clusters, &[(0, 1), (2, 5)]),
        &clusters,
        100.0,
        &[],
        None,
        &HashSet::new(),
        &HashMap::new(),
    );

    assert_eq!(IntRange::new(0, 3), output[0].cluster_range);
    assert_eq!(100.0, output[0].adjusted_width);
    assert_eq!(IntRange::new(4, 5), output[1].cluster_range);
    let RepairOption::PushIn { total_shrink, .. } = push_in(&output[0]) else {
        unreachable!()
    };
    assert_eq!(0.0, *total_shrink);
}

#[test]
fn push_in_pulls_minimal_group_to_avoid_forbidden_next_head() {
    let clusters = vec![
        cluster(0, "甲", 30.0),
        cluster(1, "乙", 30.0),
        cluster(2, "势", 20.0),
        cluster(3, "。", 10.0),
        cluster(4, "后", 50.0),
    ];
    let forbidden = HashSet::from([3]);
    let output = fill(
        &lines(&clusters, &[(0, 1), (2, 4)]),
        &clusters,
        100.0,
        &[],
        Some(&forbidden),
        &HashSet::new(),
        &HashMap::new(),
    );

    assert_eq!(IntRange::new(0, 3), output[0].cluster_range);
    assert_eq!(90.0, output[0].adjusted_width);
    assert_eq!(IntRange::new(4, 4), output[1].cluster_range);
    let RepairOption::PushIn {
        offender_cluster_index,
        total_shrink,
        ..
    } = push_in(&output[0])
    else {
        unreachable!()
    };
    assert_eq!(3, *offender_cluster_index);
    assert_eq!(0.0, *total_shrink);
}

#[test]
fn push_in_extends_past_forbidden_line_end_head() {
    let clusters = vec![
        cluster(0, "甲", 30.0),
        cluster(1, "乙", 30.0),
        cluster(2, "「", 10.0),
        cluster(3, "安", 20.0),
        cluster(4, "装", 20.0),
    ];
    let output = fill(
        &lines(&clusters, &[(0, 1), (2, 4)]),
        &clusters,
        100.0,
        &[],
        None,
        &HashSet::from([2]),
        &HashMap::new(),
    );

    assert_eq!(IntRange::new(0, 3), output[0].cluster_range);
    assert_eq!(90.0, output[0].adjusted_width);
    assert_eq!(IntRange::new(4, 4), output[1].cluster_range);
    let RepairOption::PushIn {
        offender_cluster_index,
        total_shrink,
        ..
    } = push_in(&output[0])
    else {
        unreachable!()
    };
    assert_eq!(3, *offender_cluster_index);
    assert_eq!(0.0, *total_shrink);
}

#[test]
fn source_space_compression_promotes_emergency_break_to_syllable() {
    let clusters = vec![
        cluster(0, "a", 20.0),
        cluster(1, " ", 20.0),
        cluster(2, "R", 30.0),
        cluster(3, "e", 15.0),
        cluster(4, "l", 15.0),
    ];
    let span = TextRange::new(0, 5);
    let progressive = HashMap::from([
        (
            1,
            ProgressiveBreakOpportunity::new(ProgressiveBreakTier::Structural, span),
        ),
        (
            3,
            ProgressiveBreakOpportunity::new(ProgressiveBreakTier::Emergency, span),
        ),
        (
            4,
            ProgressiveBreakOpportunity::new(ProgressiveBreakTier::Syllable, span),
        ),
    ]);
    let output = fill(
        &lines(&clusters, &[(0, 2), (3, 4)]),
        &clusters,
        80.0,
        &[ShrinkOpportunity::new(
            1,
            2,
            10.0,
            ShrinkChannel::RawAdvance,
        )],
        None,
        &HashSet::new(),
        &progressive,
    );

    assert_eq!(IntRange::new(0, 3), output[0].cluster_range);
    assert_eq!(80.0, output[0].adjusted_width);
    let RepairOption::PushIn {
        reason,
        total_shrink,
        allocations,
        ..
    } = push_in(&output[0])
    else {
        unreachable!()
    };
    assert!(reason.starts_with("ProgressiveTechnicalTierPromotion"));
    assert_eq!(5.0, *total_shrink);
    assert_eq!(1, allocations.len());
    assert_eq!(1, allocations[0].cluster_index);
    assert_eq!(5.0, allocations[0].shrink);
}

#[test]
fn cleaner_boundary_that_still_leaves_deficit_does_not_promote() {
    let clusters = vec![
        cluster(0, "a", 20.0),
        cluster(1, " ", 20.0),
        cluster(2, "R", 30.0),
        cluster(3, "e", 15.0),
        cluster(4, "l", 15.0),
    ];
    let span = TextRange::new(0, 5);
    let progressive = HashMap::from([
        (
            3,
            ProgressiveBreakOpportunity::new(ProgressiveBreakTier::Emergency, span),
        ),
        (
            4,
            ProgressiveBreakOpportunity::new(ProgressiveBreakTier::Syllable, span),
        ),
    ]);
    let output = fill(
        &lines(&clusters, &[(0, 2), (3, 4)]),
        &clusters,
        100.0,
        &[],
        None,
        &HashSet::new(),
        &progressive,
    );

    assert_eq!(IntRange::new(0, 2), output[0].cluster_range);
    assert_eq!(IntRange::new(3, 4), output[1].cluster_range);
    assert_eq!(None, output[0].repair);
}

#[test]
fn selected_tier_refills_across_intermediate_cleaner_boundary() {
    let clusters = vec![
        cluster(0, "a", 20.0),
        cluster(1, " ", 20.0),
        cluster(2, "R", 30.0),
        cluster(3, "e", 15.0),
        cluster(4, "l", 15.0),
    ];
    let span = TextRange::new(0, 5);
    let progressive = HashMap::from([
        (
            3,
            ProgressiveBreakOpportunity::new(ProgressiveBreakTier::Emergency, span),
        ),
        (
            4,
            ProgressiveBreakOpportunity::new(ProgressiveBreakTier::Syllable, span),
        ),
        (
            5,
            ProgressiveBreakOpportunity::new(ProgressiveBreakTier::Emergency, span),
        ),
    ]);
    let output = fill(
        &lines(&clusters, &[(0, 2), (3, 4)]),
        &clusters,
        100.0,
        &[],
        None,
        &HashSet::new(),
        &progressive,
    );

    assert_eq!(1, output.len());
    assert_eq!(IntRange::new(0, 4), output[0].cluster_range);
    assert_eq!(100.0, output[0].adjusted_width);
    let RepairOption::PushIn { reason, .. } = push_in(&output[0]) else {
        unreachable!()
    };
    assert!(reason.starts_with("LineAdjustmentPushIn"));
}
