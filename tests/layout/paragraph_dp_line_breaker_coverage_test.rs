use tiqian::core::geometry::{text_range};
use tiqian::core::int_range::IntRange;
use tiqian::core::layout_model::{Cluster, LineEndReason};
use tiqian::core::text::Text;
use tiqian::layout::line_optimization::{LineSolution, RepairOption};
use tiqian::layout::line_breaker::{LineBreaker, LineBreakerConfig};
use tiqian::layout::paragraph_dp_line_breaker::ParagraphDpLineBreaker;
use tiqian::layout::progressive_break_decisions::{
    ProgressiveBreakOpportunity, ProgressiveBreakTier, ShrinkChannel, ShrinkOpportunity,
};

fn cluster(index: i32, advance: f32) -> Cluster {
    Cluster::new(
        text_range(index, index + 1),
        Text::from("中"),
        "test".to_owned(),
        advance,
    )
}

fn han_clusters(count: i32, advance: f32) -> Vec<Cluster> {
    (0..count).map(|index| cluster(index, advance)).collect()
}

fn latin_clusters() -> Vec<Cluster> {
    vec![
        Cluster::new(text_range(0, 1), Text::from("a"), "test".to_owned(), 30.0),
        Cluster::new(text_range(1, 2), Text::from("/"), "test".to_owned(), 30.0),
        Cluster::new(text_range(2, 3), Text::from("b"), "test".to_owned(), 25.0),
        Cluster::new(text_range(3, 4), Text::from("c"), "test".to_owned(), 30.0),
        Cluster::new(text_range(4, 5), Text::from("d"), "test".to_owned(), 30.0),
    ]
}

#[test]
fn empty_clusters_return_an_empty_solution() {
    let solution = ParagraphDpLineBreaker::default().break_lines(
        &[],
        &[],
        100.0,
        &LineBreakerConfig::default(),
    );
    assert!(solution.lines.is_empty());
}

#[test]
fn mismatched_natural_and_adjusted_sizes_are_rejected() {
    let error = std::panic::catch_unwind(|| {
        ParagraphDpLineBreaker::default().break_lines(
            &han_clusters(2, 16.0),
            &han_clusters(1, 16.0),
            100.0,
            &LineBreakerConfig::default(),
        );
    })
    .expect_err("expected natural/adjusted alignment rejection");
    let message = error
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| error.downcast_ref::<&str>().copied())
        .expect("panic message");
    assert!(message.contains("cluster-for-cluster"), "{message}");
}

#[test]
fn negative_candidate_window_is_rejected() {
    let error = std::panic::catch_unwind(|| {
        let mut breaker = ParagraphDpLineBreaker::default();
        breaker.candidate_window = -1;
        breaker.break_lines(
            &han_clusters(2, 16.0),
            &han_clusters(2, 16.0),
            100.0,
            &LineBreakerConfig::default(),
        );
    })
    .expect_err("expected negative candidate window rejection");
    let message = error
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| error.downcast_ref::<&str>().copied())
        .expect("panic message");
    assert!(message.contains("non-negative"), "{message}");
}

#[test]
fn shrink_prefix_skips_non_positive_and_out_of_range_opportunities() {
    let clusters = han_clusters(4, 16.0);
    let mut config = LineBreakerConfig::default();
    config.shrink_opportunities = vec![
        ShrinkOpportunity::new(1, 2, 0.0, ShrinkChannel::RawAdvance),
        ShrinkOpportunity::new(4, 2, 8.0, ShrinkChannel::RawAdvance),
        ShrinkOpportunity::new(1, 2, 8.0, ShrinkChannel::RawAdvance),
    ];
    let solution = ParagraphDpLineBreaker::default().break_lines(&clusters, &clusters, 100.0, &config);

    assert_eq!(1, solution.lines.len(), "{:?}", solution.lines);
    assert_eq!(IntRange::new(0, 3), solution.lines[0].cluster_range);
}

#[test]
fn line_end_only_capacity_feeds_the_compressed_edge_at_the_line_end() {
    let clusters = han_clusters(4, 16.0);
    let mut config = LineBreakerConfig::default();
    config.shrink_opportunities = vec![ShrinkOpportunity::with_line_end_only(
        2,
        1,
        4.0,
        ShrinkChannel::TrailingGlue,
        true,
    )];
    config.cjk_inter_char_boundaries = tiqian::common::HashSet::from([1]);
    config.line_adjustment_push_in = true;
    let solution = ParagraphDpLineBreaker::default().break_lines(&clusters, &clusters, 44.0, &config);

    let first = &solution.lines[0];
    assert_eq!(IntRange::new(0, 2), first.cluster_range, "{:?}", solution.lines);
    assert!(matches!(&first.repair, Some(RepairOption::PushIn { reason, .. }) if reason.starts_with("LineAdjustmentPushIn")), "{:?}", solution.lines);
}

#[test]
fn compressed_ends_may_reach_the_segment_end() {
    let clusters = han_clusters(3, 16.0);
    let mut config = LineBreakerConfig::default();
    config.shrink_opportunities = vec![ShrinkOpportunity::new(1, 2, 12.0, ShrinkChannel::RawAdvance)];
    config.cjk_inter_char_boundaries = tiqian::common::HashSet::from([1]);
    config.line_adjustment_push_in = true;
    let solution = ParagraphDpLineBreaker::default().break_lines(&clusters, &clusters, 44.0, &config);

    let first = &solution.lines[0];
    assert_eq!(IntRange::new(0, 2), first.cluster_range, "{:?}", solution.lines);
    assert!(matches!(&first.repair, Some(RepairOption::PushIn { reason, .. }) if reason.starts_with("LineAdjustmentPushIn")), "{:?}", solution.lines);
    assert_eq!(LineEndReason::ParagraphEnd, solution.lines.last().unwrap().end_reason);
}

#[test]
fn compressed_final_mandatory_line_uses_the_compressed_commit_branch() {
    let clusters = han_clusters(4, 16.0);
    let mut config = LineBreakerConfig::default();
    config.shrink_opportunities = vec![ShrinkOpportunity::with_line_end_only(
        2,
        1,
        4.0,
        ShrinkChannel::TrailingGlue,
        true,
    )];
    config.hard_break_after_clusters = tiqian::common::HashSet::from([2]);
    config.line_adjustment_push_in = true;
    let solution = ParagraphDpLineBreaker::default().break_lines(&clusters, &clusters, 44.0, &config);

    assert_eq!(IntRange::new(0, 2), solution.lines[0].cluster_range, "{:?}", solution.lines);
    assert_eq!(LineEndReason::MandatoryBreak, solution.lines[0].end_reason);
    assert!(matches!(&solution.lines[0].repair, Some(RepairOption::PushIn { reason, .. }) if reason.starts_with("LineAdjustmentPushIn")), "{:?}", solution.lines);
}

#[test]
fn tier_promotion_routes_the_repair_reason_through_the_promotion_code() {
    let clusters = latin_clusters();
    let span = text_range(0, clusters.len() as i32);
    let mut config = LineBreakerConfig::default();
    config.shrink_opportunities = vec![ShrinkOpportunity::new(2, 2, 5.0, ShrinkChannel::RawAdvance)];
    config.line_adjustment_push_in = true;
    config.progressive_break_opportunities = tiqian::common::HashMap::from([
        (2, ProgressiveBreakOpportunity::new(ProgressiveBreakTier::Emergency, span)),
        (3, ProgressiveBreakOpportunity::new(ProgressiveBreakTier::Whitespace, span)),
    ]);
    let solution = ParagraphDpLineBreaker::default().break_lines(&clusters, &clusters, 80.0, &config);

    let first = &solution.lines[0];
    assert_eq!(IntRange::new(0, 2), first.cluster_range, "{:?}", solution.lines);
    assert!(matches!(&first.repair, Some(RepairOption::PushIn { reason, .. }) if reason.starts_with("ProgressiveTechnicalTierPromotion")), "{:?}", solution.lines);
}

#[test]
fn promotion_check_returns_false_when_the_candidate_end_has_no_opportunity() {
    let clusters = latin_clusters();
    let span = text_range(0, clusters.len() as i32);
    let mut config = LineBreakerConfig::default();
    config.shrink_opportunities = vec![ShrinkOpportunity::new(2, 2, 5.0, ShrinkChannel::RawAdvance)];
    config.line_adjustment_push_in = true;
    config.progressive_break_opportunities = tiqian::common::HashMap::from([(
        2,
        ProgressiveBreakOpportunity::new(ProgressiveBreakTier::Emergency, span),
    )]);
    let solution = ParagraphDpLineBreaker::default().break_lines(&clusters, &clusters, 80.0, &config);

    assert_eq!(IntRange::new(0, 1), solution.lines[0].cluster_range, "{:?}", solution.lines);
    assert!(solution.lines[0].repair.is_none(), "{:?}", solution.lines);
}

#[test]
fn mandatory_segment_filters_the_control_boundary_from_candidates() {
    let clusters = han_clusters(6, 16.0);
    let mut config = LineBreakerConfig::default();
    config.hard_break_after_clusters = tiqian::common::HashSet::from([2]);
    let solution = ParagraphDpLineBreaker::default().break_lines(&clusters, &clusters, 32.0, &config);

    assert_eq!(IntRange::new(0, 2), solution.lines[0].cluster_range, "{:?}", solution.lines);
    assert_eq!(LineEndReason::MandatoryBreak, solution.lines[0].end_reason);
    assert_eq!(5, solution.lines.last().unwrap().cluster_range.last(), "{:?}", solution.lines);
}

#[test]
fn narrow_windows_drop_ends_at_or_below_the_line_start() {
    let clusters = han_clusters(4, 16.0);
    let solution = ParagraphDpLineBreaker::default().break_lines(
        &clusters,
        &clusters,
        20.0,
        &LineBreakerConfig::default(),
    );

    assert_eq!(4, solution.lines.len(), "{:?}", solution.lines);
    assert!(solution
        .lines
        .iter()
        .all(|line| line.cluster_range.first() == line.cluster_range.last()), "{:?}", solution.lines);
}

#[test]
fn interface_default_strategy_name_is_custom() {
    struct CustomBreaker;

    impl LineBreaker for CustomBreaker {
        fn break_lines(
            &self,
            _: &[Cluster],
            _: &[Cluster],
            _: f32,
            _: &LineBreakerConfig,
        ) -> LineSolution {
            LineSolution::new(Vec::new())
        }
    }

    assert_eq!("custom", CustomBreaker.strategy_name());
}
