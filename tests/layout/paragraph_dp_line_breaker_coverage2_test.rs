use tiqian::core::geometry::{text_range};
use tiqian::core::int_range::IntRange;
use tiqian::core::layout_model::{Cluster, LineEndReason};
use tiqian::core::text::Text;
use tiqian::layout::line_breaker::{LineBreaker, LineBreakerConfig};
use tiqian::layout::paragraph_dp_line_breaker::ParagraphDpLineBreaker;
use tiqian::layout::progressive_break_decisions::{
    ProgressiveBreakOpportunity, ProgressiveBreakTier, ShrinkChannel, ShrinkOpportunity,
    UnbreakableRanges,
};

fn cluster(index: i32, text: &str, advance: f32) -> Cluster {
    Cluster::new(
        text_range(index, index + 1),
        Text::from(text),
        "test".to_owned(),
        advance,
    )
}

fn han_clusters(count: i32, advance: f32) -> Vec<Cluster> {
    (0..count).map(|index| cluster(index, "中", advance)).collect()
}

fn latin_clusters() -> Vec<Cluster> {
    vec![
        cluster(0, "a", 30.0),
        cluster(1, "/", 30.0),
        cluster(2, "b", 25.0),
        cluster(3, "c", 30.0),
        cluster(4, "d", 30.0),
    ]
}

#[test]
fn test_shrink_opportunities_negative_and_out_of_range() {
    let clusters = han_clusters(3, 16.0);
    let mut config = LineBreakerConfig::default();
    config.shrink_opportunities = vec![
        ShrinkOpportunity::new(-1, 1, 10.0, ShrinkChannel::RawAdvance),
        ShrinkOpportunity::new(0, 1, -5.0, ShrinkChannel::RawAdvance),
        ShrinkOpportunity::new(5, 1, 10.0, ShrinkChannel::RawAdvance),
        ShrinkOpportunity::new(1, 1, 4.0, ShrinkChannel::RawAdvance),
        ShrinkOpportunity::with_line_end_only(2, 1, 4.0, ShrinkChannel::TrailingGlue, true),
    ];

    let solution = ParagraphDpLineBreaker::default().break_lines(&clusters, &clusters, 100.0, &config);
    assert_eq!(1, solution.lines.len());
}

#[test]
fn test_candidate_window_bounds_compression_edges() {
    let clusters = han_clusters(4, 20.0);
    let mut breaker = ParagraphDpLineBreaker::default();
    breaker.candidate_window = 1;
    let mut config = LineBreakerConfig::default();
    config.shrink_opportunities = vec![
        ShrinkOpportunity::new(0, 1, 10.0, ShrinkChannel::RawAdvance),
        ShrinkOpportunity::new(1, 1, 10.0, ShrinkChannel::RawAdvance),
        ShrinkOpportunity::new(2, 1, 10.0, ShrinkChannel::RawAdvance),
    ];
    config.line_adjustment_push_in = true;

    let solution = breaker.break_lines(&clusters, &clusters, 25.0, &config);
    assert!(!solution.lines.is_empty());
}

#[test]
fn test_progressive_tier_promotion_branches() {
    let clusters = latin_clusters();
    let span = text_range(0, clusters.len() as i32);
    let other_span = text_range(1, 3);

    let mut no_promotion_config = LineBreakerConfig::default();
    no_promotion_config.shrink_opportunities = vec![ShrinkOpportunity::new(
        2,
        2,
        5.0,
        ShrinkChannel::RawAdvance,
    )];
    no_promotion_config.line_adjustment_push_in = true;
    no_promotion_config.progressive_break_opportunities = tiqian::common::HashMap::from([
        (2, ProgressiveBreakOpportunity::new(ProgressiveBreakTier::Whitespace, span)),
        (3, ProgressiveBreakOpportunity::new(ProgressiveBreakTier::Emergency, span)),
    ]);
    let no_promotion_solution = ParagraphDpLineBreaker::default().break_lines(
        &clusters,
        &clusters,
        80.0,
        &no_promotion_config,
    );
    assert!(!no_promotion_solution.lines.is_empty());

    let mut different_span_config = no_promotion_config.clone();
    different_span_config.progressive_break_opportunities = tiqian::common::HashMap::from([
        (2, ProgressiveBreakOpportunity::new(ProgressiveBreakTier::Emergency, span)),
        (3, ProgressiveBreakOpportunity::new(ProgressiveBreakTier::Whitespace, other_span)),
    ]);
    let different_span_solution = ParagraphDpLineBreaker::default().break_lines(
        &clusters,
        &clusters,
        80.0,
        &different_span_config,
    );
    assert!(!different_span_solution.lines.is_empty());

    let mut breaker = ParagraphDpLineBreaker::default();
    breaker.candidate_window = 4;
    let mut mixed_promotion_config = LineBreakerConfig::default();
    mixed_promotion_config.shrink_opportunities = vec![ShrinkOpportunity::new(
        2,
        2,
        5.0,
        ShrinkChannel::RawAdvance,
    )];
    mixed_promotion_config.line_adjustment_push_in = true;
    mixed_promotion_config.progressive_break_opportunities = tiqian::common::HashMap::from([
        (2, ProgressiveBreakOpportunity::new(ProgressiveBreakTier::Emergency, span)),
        (3, ProgressiveBreakOpportunity::new(ProgressiveBreakTier::Whitespace, span)),
    ]);
    let mixed_promotion_solution = breaker.break_lines(
        &clusters,
        &clusters,
        80.0,
        &mixed_promotion_config,
    );
    assert!(!mixed_promotion_solution.lines.is_empty());
}

#[test]
fn test_commit_segment_original_break_not_null_resulting_break_null() {
    let clusters = latin_clusters();
    let span = text_range(0, clusters.len() as i32);
    let mut config = LineBreakerConfig::default();
    config.shrink_opportunities = vec![ShrinkOpportunity::new(2, 2, 5.0, ShrinkChannel::RawAdvance)];
    config.line_adjustment_push_in = true;
    config.progressive_break_opportunities = tiqian::common::HashMap::from([(
        2,
        ProgressiveBreakOpportunity::new(ProgressiveBreakTier::Whitespace, span),
    )]);

    let solution = ParagraphDpLineBreaker::default().break_lines(&clusters, &clusters, 80.0, &config);
    assert!(!solution.lines.is_empty());
}

#[test]
fn test_tier_preferred_pool_empty_fallback() {
    let clusters = han_clusters(4, 20.0);
    let mut config = LineBreakerConfig::default();
    config.unbreakable_ranges = UnbreakableRanges::new(vec![IntRange::new(0, 3)]);

    let solution = ParagraphDpLineBreaker::default().break_lines(&clusters, &clusters, 30.0, &config);

    assert!(!solution.lines.is_empty());
}

#[test]
fn test_hard_break_after_clusters_in_dp_commit() {
    let clusters = han_clusters(4, 20.0);
    let mut config = LineBreakerConfig::default();
    config.hard_break_after_clusters = tiqian::common::HashSet::from([1]);

    let solution = ParagraphDpLineBreaker::default().break_lines(&clusters, &clusters, 50.0, &config);
    assert_eq!(2, solution.lines.len());
    assert_eq!(LineEndReason::MandatoryBreak, solution.lines[0].end_reason);
    assert_eq!(LineEndReason::ParagraphEnd, solution.lines[1].end_reason);
}

#[test]
fn test_candidate_ends_window_below_line_start() {
    let clusters = han_clusters(3, 20.0);
    let mut breaker = ParagraphDpLineBreaker::default();
    breaker.candidate_window = 5;

    let solution = breaker.break_lines(&clusters, &clusters, 25.0, &LineBreakerConfig::default());
    assert_eq!(3, solution.lines.len());
}
