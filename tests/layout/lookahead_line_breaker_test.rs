use tiqian::common::HashSet;

use tiqian::core::geometry::TextRange;
use tiqian::core::int_range::IntRange;
use tiqian::core::layout_model::Cluster;
use tiqian::core::text::Text;
use tiqian::layout::line_breaker::{
    GreedyLineBreaker, LineBreaker, LineBreakerConfig, LookaheadLineBreaker,
};
use tiqian::layout::line_optimization::{LineCandidate, RepairOption};
use tiqian::layout::progressive_break_decisions::{
    ShrinkChannel, ShrinkOpportunity, UnbreakableRanges,
};

fn cluster(start: i32, text: &str, advance: f32) -> Cluster {
    Cluster::new(
        TextRange::new(start, start + 1),
        Text::from(text),
        "test".to_owned(),
        advance,
    )
}

fn break_lines(
    breaker: &dyn LineBreaker,
    clusters: &[Cluster],
    max_width: f32,
    config: LineBreakerConfig,
) -> tiqian::layout::line_optimization::LineSolution {
    breaker.break_lines(clusters, clusters, max_width, &config)
}

#[test]
fn hanging_tail_is_excluded_from_fill_density_geometry() {
    let mut line = LineCandidate::new(IntRange::new(0, 2), TextRange::new(0, 3), 48.0, 16.0);
    line.hanging_cluster_indices = HashSet::from([1, 2]);
    line.validate_hanging_suffix();

    assert_eq!(IntRange::new(0, 0), line.in_measure_cluster_range());
    assert_eq!(
        0,
        tiqian::layout::line_breaker::line_gap_count(
            line.in_measure_cluster_range(),
            &HashSet::from([1, 2])
        )
    );
    assert_eq!(
        0.0,
        tiqian::layout::line_breaker::line_adjustment_density(
            &line,
            48.0,
            false,
            &HashSet::from([1, 2])
        )
    );
}

#[test]
#[should_panic(expected = "Hanging clusters must be a trailing line suffix")]
fn hanging_clusters_must_be_a_contiguous_trailing_suffix() {
    let mut line = LineCandidate::new(IntRange::new(0, 2), TextRange::new(0, 3), 48.0, 32.0);
    line.hanging_cluster_indices = HashSet::from([1]);
    line.validate_hanging_suffix();
}

#[test]
fn compatibility_hanging_index_skips_a_trailing_mandatory_break_control() {
    let mut line = LineCandidate::new(IntRange::new(0, 2), TextRange::new(0, 3), 32.0, 16.0);
    line.repair = Some(RepairOption::Hang {
        penalty: 5,
        reason: "test".to_owned(),
        offender_cluster_index: 1,
    });
    line.hanging_cluster_indices = HashSet::from([1, 2]);
    line.validate_hanging_suffix();

    assert_eq!(Some(1), line.hanging_cluster_index());
}

#[test]
fn empty_input_produces_no_lines() {
    let solution = break_lines(
        &LookaheadLineBreaker::default(),
        &[],
        100.0,
        LineBreakerConfig::default(),
    );
    assert!(solution.lines.is_empty());
}

#[test]
fn lookahead_matches_greedy_when_shifting_earlier_gives_no_benefit() {
    let clusters: Vec<_> = (0..6).map(|index| cluster(index, "x", 16.0)).collect();
    let solution = break_lines(
        &LookaheadLineBreaker::default(),
        &clusters,
        64.0,
        LineBreakerConfig::default(),
    );

    assert_eq!(2, solution.lines.len());
    assert_eq!(IntRange::new(0, 3), solution.lines[0].cluster_range);
    assert_eq!(IntRange::new(4, 5), solution.lines[1].cluster_range);
    assert_eq!(0.0, solution.total_badness);
}

#[test]
fn lookahead_shifts_break_earlier_to_avoid_kinsoku_repair() {
    let clusters = vec![
        cluster(0, "中", 16.0),
        cluster(1, "文", 16.0),
        cluster(2, "中", 16.0),
        cluster(3, "文", 16.0),
        cluster(4, "中", 16.0),
        cluster(5, "文", 16.0),
        cluster(6, "。", 16.0),
    ];
    let greedy = break_lines(
        &GreedyLineBreaker::default(),
        &clusters,
        48.0,
        LineBreakerConfig::default(),
    );
    let lookahead = break_lines(
        &LookaheadLineBreaker::default(),
        &clusters,
        48.0,
        LineBreakerConfig::default(),
    );

    assert!(matches!(
        greedy.lines[2].repair,
        Some(RepairOption::CarryPrevious { .. })
    ));
    assert_eq!(10.0, greedy.total_badness);
    assert_eq!(
        vec![
            IntRange::new(0, 1),
            IntRange::new(2, 4),
            IntRange::new(5, 6)
        ],
        lookahead
            .lines
            .iter()
            .map(|line| line.cluster_range)
            .collect::<Vec<_>>()
    );
    assert!(lookahead.lines.iter().all(|line| line.repair.is_none()));
    assert_eq!(0.0, lookahead.total_badness);
}

#[test]
fn lookahead_keeps_greedy_break_when_push_in_glue_covers_repair() {
    let clusters = vec![
        cluster(0, "中", 16.0),
        cluster(1, "文", 16.0),
        cluster(2, "中", 16.0),
        cluster(3, "。", 16.0),
    ];
    let mut config = LineBreakerConfig::default();
    config.shrink_opportunities = vec![ShrinkOpportunity::new(
        3,
        6,
        4.0,
        ShrinkChannel::TrailingGlue,
    )];
    let solution = break_lines(&LookaheadLineBreaker::default(), &clusters, 60.0, config);

    assert_eq!(1, solution.lines.len());
    let line = &solution.lines[0];
    assert_eq!(IntRange::new(0, 3), line.cluster_range);
    assert_eq!(60.0, line.adjusted_width);
    assert!(matches!(line.repair, Some(RepairOption::PushIn { .. })));
    assert_eq!(2.0, solution.total_badness);
}

#[test]
fn lookahead_scores_future_push_in_before_choosing_earlier_break() {
    let clusters = vec![
        cluster(0, "中", 16.0),
        cluster(1, "文", 16.0),
        cluster(2, "中", 16.0),
        cluster(3, "文", 16.0),
        cluster(4, "中", 16.0),
        cluster(5, "文", 16.0),
        cluster(6, "。", 16.0),
    ];
    let mut config = LineBreakerConfig::default();
    config.shrink_opportunities = vec![ShrinkOpportunity::new(
        6,
        6,
        4.0,
        ShrinkChannel::TrailingGlue,
    )];
    let solution = break_lines(&LookaheadLineBreaker::default(), &clusters, 60.0, config);

    assert_eq!(
        vec![IntRange::new(0, 2), IntRange::new(3, 6)],
        solution
            .lines
            .iter()
            .map(|line| line.cluster_range)
            .collect::<Vec<_>>()
    );
    assert_eq!(60.0, solution.lines[1].adjusted_width);
    assert!(matches!(
        solution.lines[1].repair,
        Some(RepairOption::PushIn { .. })
    ));
    assert_eq!(2.0, solution.total_badness);
}

#[test]
fn lookahead_falls_back_to_greedy_when_alternatives_are_worse() {
    let clusters: Vec<_> = (0..9).map(|index| cluster(index, "x", 16.0)).collect();
    let solution = break_lines(
        &LookaheadLineBreaker::default(),
        &clusters,
        64.0,
        LineBreakerConfig::default(),
    );

    assert_eq!(3, solution.lines.len());
    assert_eq!(IntRange::new(0, 3), solution.lines[0].cluster_range);
    assert_eq!(IntRange::new(4, 7), solution.lines[1].cluster_range);
    assert_eq!(IntRange::new(8, 8), solution.lines[2].cluster_range);
}

#[test]
fn lookahead_avoids_consecutive_synthetic_hyphen_breaks() {
    let clusters: Vec<_> = (0..8).map(|index| cluster(index, "x", 10.0)).collect();
    let mut config = LineBreakerConfig::default();
    config.hyphen_break_clusters = HashSet::from([3, 6]);
    let no_penalty = LookaheadLineBreaker::new(
        Box::new(tiqian::layout::kinsoku_rule::ClreqKinsokuRule::default()),
        2,
        2,
        0.5,
        2,
        10,
        20,
        0.0,
    )
    .break_lines(&clusters, &clusters, 30.0, &config);
    let with_penalty = LookaheadLineBreaker::default().break_lines(&clusters, &clusters, 30.0, &config);

    assert_eq!(
        vec![IntRange::new(0, 2), IntRange::new(3, 5), IntRange::new(6, 7)],
        no_penalty.lines.iter().map(|line| line.cluster_range).collect::<Vec<_>>(),
    );
    assert_eq!(
        vec![IntRange::new(0, 1), IntRange::new(2, 4), IntRange::new(5, 7)],
        with_penalty.lines.iter().map(|line| line.cluster_range).collect::<Vec<_>>(),
    );
}

#[test]
fn lookahead_scores_kinsoku_repairs_with_unbreakable_ranges() {
    let mut clusters: Vec<_> = (0..8).map(|index| cluster(index, "中", 16.0)).collect();
    clusters.push(cluster(8, "。", 16.0));
    let mut config = LineBreakerConfig::default();
    config.unbreakable_ranges = UnbreakableRanges::new(vec![IntRange::new(6, 7)]);
    config.forbidden_line_start_clusters = Some(HashSet::from([8]));
    let breaker = LookaheadLineBreaker::new(
        Box::new(tiqian::layout::kinsoku_rule::ClreqKinsokuRule::default()),
        2,
        2,
        0.5,
        2,
        10,
        20,
        12.0,
    );
    let solution = break_lines(&breaker, &clusters, 64.0, config);

    assert_eq!(
        vec![
            IntRange::new(0, 1),
            IntRange::new(2, 5),
            IntRange::new(6, 8)
        ],
        solution
            .lines
            .iter()
            .map(|line| line.cluster_range)
            .collect::<Vec<_>>()
    );
    assert!(solution.lines.iter().all(|line| line.repair.is_none()));
    assert_eq!(0.0, solution.total_badness);
}

#[test]
fn window_zero_reduces_lookahead_to_greedy() {
    let clusters = vec![
        cluster(0, "中", 16.0),
        cluster(1, "文", 16.0),
        cluster(2, "中", 16.0),
        cluster(3, "文", 16.0),
        cluster(4, "中", 16.0),
        cluster(5, "文", 16.0),
        cluster(6, "。", 16.0),
    ];
    let breaker = LookaheadLineBreaker::new(
        Box::new(tiqian::layout::kinsoku_rule::ClreqKinsokuRule::default()),
        0,
        2,
        0.5,
        2,
        10,
        20,
        12.0,
    );
    let solution = break_lines(&breaker, &clusters, 48.0, LineBreakerConfig::default());

    assert!(matches!(
        solution.lines[2].repair,
        Some(RepairOption::CarryPrevious { .. })
    ));
}
