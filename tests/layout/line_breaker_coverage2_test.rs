use tiqian::common::{HashMap, HashSet};
use tiqian::core::geometry::{text_range};
use tiqian::core::int_range::IntRange;
use tiqian::core::layout_model::{Cluster, LineEndReason};
use tiqian::core::text::Text;
use tiqian::layout::line_breaker::{
    LineBreaker, LineBreakerConfig, LookaheadLineBreaker, ends_with_progressive_break,
    find_greedy_end, line_gap_count, rebuild_line,
};
use tiqian::layout::line_optimization::{LineCandidate, LineSolution};
use tiqian::layout::progressive_break_decisions::{
    ProgressiveBreakOpportunity, ProgressiveBreakTier,
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

#[test]
fn test_line_breaker_strategy_name_default() {
    assert_eq!("custom", CustomBreaker.strategy_name());
}

#[test]
fn test_lookahead_line_breaker_preconditions() {
    let clusters = han_clusters(2, 16.0);
    assert!(std::panic::catch_unwind(|| {
        LookaheadLineBreaker::default().break_lines(&han_clusters(1, 16.0), &clusters, 100.0, &LineBreakerConfig::default());
    })
    .is_err());
    assert!(std::panic::catch_unwind(|| {
        LookaheadLineBreaker::new(
            Box::new(tiqian::layout::kinsoku_rule::ClreqKinsokuRule::default()),
            -1,
            2,
            0.5,
            2,
            10,
            20,
            12.0,
        )
        .break_lines(&clusters, &clusters, 100.0, &LineBreakerConfig::default());
    })
    .is_err());
    assert!(std::panic::catch_unwind(|| {
        LookaheadLineBreaker::new(
            Box::new(tiqian::layout::kinsoku_rule::ClreqKinsokuRule::default()),
            2,
            -1,
            0.5,
            2,
            10,
            20,
            12.0,
        )
        .break_lines(&clusters, &clusters, 100.0, &LineBreakerConfig::default());
    })
    .is_err());
}

#[test]
fn test_lookahead_candidate_filtering_with_non_rendering_control_clusters() {
    let clusters = vec![cluster(0, "\u{200b}", 0.0), cluster(1, "A", 20.0), cluster(2, "B", 20.0)];
    let mut config = LineBreakerConfig::default();
    config.non_rendering_control_clusters = HashSet::from([0]);
    let solution = LookaheadLineBreaker::new(
        Box::new(tiqian::layout::kinsoku_rule::ClreqKinsokuRule::default()),
        2,
        2,
        0.5,
        2,
        10,
        20,
        12.0,
    )
    .break_lines(&clusters, &clusters, 25.0, &config);
    assert!(!solution.lines.is_empty());
    assert_eq!(IntRange::new(0, 1), solution.lines[0].cluster_range);
}

#[test]
fn test_lookahead_hard_break_at_end_and_middle() {
    let end_clusters = han_clusters(2, 16.0);
    let mut end_config = LineBreakerConfig::default();
    end_config.hard_break_after_clusters = HashSet::from([1]);
    let end_solution = LookaheadLineBreaker::default().break_lines(&end_clusters, &end_clusters, 20.0, &end_config);
    assert_eq!(2, end_solution.lines.len());
    assert_eq!(IntRange::new(0, 1), end_solution.lines[0].cluster_range);
    assert_eq!(LineEndReason::MandatoryBreak, end_solution.lines[0].end_reason);
    assert_eq!(IntRange::EMPTY, end_solution.lines[1].cluster_range);
    assert_eq!(LineEndReason::ParagraphEnd, end_solution.lines[1].end_reason);

    let middle_clusters = han_clusters(3, 16.0);
    let mut middle_config = LineBreakerConfig::default();
    middle_config.hard_break_after_clusters = HashSet::from([0]);
    let middle_solution = LookaheadLineBreaker::default().break_lines(&middle_clusters, &middle_clusters, 20.0, &middle_config);
    assert_eq!(3, middle_solution.lines.len());
    assert_eq!(IntRange::new(0, 0), middle_solution.lines[0].cluster_range);
    assert_eq!(LineEndReason::MandatoryBreak, middle_solution.lines[0].end_reason);

    let oversized = vec![cluster(0, "A", 50.0), cluster(1, "B", 10.0)];
    let mut oversized_config = LineBreakerConfig::default();
    oversized_config.hard_break_after_clusters = HashSet::from([0]);
    assert_eq!(2, LookaheadLineBreaker::default().break_lines(&oversized, &oversized, 20.0, &oversized_config).lines.len());
}

#[test]
fn test_line_candidate_ends_with_progressive_break() {
    let candidate = LineCandidate::new(IntRange::new(0, 1), text_range(0, 2), 32.0, 32.0);
    let opportunities = HashMap::from([(
        2,
        ProgressiveBreakOpportunity::new(ProgressiveBreakTier::Syllable, text_range(0, 4)),
    )]);
    assert!(ends_with_progressive_break(&candidate, &opportunities));

    let mut paragraph_end = candidate.clone();
    paragraph_end.end_reason = LineEndReason::ParagraphEnd;
    assert!(!ends_with_progressive_break(&paragraph_end, &opportunities));
    assert!(!ends_with_progressive_break(&LineCandidate::new(IntRange::EMPTY, text_range(0, 0), 0.0, 0.0), &opportunities));
    assert!(!ends_with_progressive_break(&candidate, &HashMap::new()));
}

#[test]
fn test_line_gap_count() {
    assert_eq!(0, line_gap_count(IntRange::EMPTY, &HashSet::from([0, 1])));
    assert_eq!(1, line_gap_count(IntRange::new(0, 2), &HashSet::from([1])));
    assert_eq!(0, line_gap_count(IntRange::new(0, 2), &HashSet::from([2])));
}

#[test]
fn test_rebuild_line_empty_range_throws() {
    let clusters = han_clusters(2, 16.0);
    assert!(std::panic::catch_unwind(|| {
        rebuild_line(IntRange::EMPTY, &clusters, &clusters, LineEndReason::AutoWrap, None, Vec::new());
    })
    .is_err());
}

#[test]
fn test_find_greedy_end_default_args() {
    let clusters = han_clusters(5, 10.0);
    assert_eq!(2, find_greedy_end(&clusters, 0, 25.0, clusters.len() as i32, &HashSet::new()));
}

#[test]
fn test_lookahead_orphan_and_synthetic_hyphen_runs() {
    let clusters = han_clusters(4, 20.0);
    let mut config = LineBreakerConfig::default();
    config.hyphen_break_clusters = HashSet::from([1, 2, 3]);
    let solution = LookaheadLineBreaker::new(
        Box::new(tiqian::layout::kinsoku_rule::ClreqKinsokuRule::default()),
        2,
        2,
        0.5,
        2,
        10,
        20,
        12.0,
    )
    .break_lines(&clusters, &clusters, 25.0, &config);
    assert_eq!(4, solution.lines.len());
}
