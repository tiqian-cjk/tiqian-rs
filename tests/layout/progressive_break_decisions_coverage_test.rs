use tiqian::common::{HashMap, HashSet};
use tiqian::core::geometry::TextRange;
use tiqian::core::layout_model::Cluster;
use tiqian::core::text::Text;
use tiqian::layout::progressive_break_decisions::{
    ProgressiveBreakOpportunity, ProgressiveBreakTier, decide_hyphen_break, decide_progressive_break,
    progressive_candidate_allowed,
};

fn cluster(index: i32, text: &str, advance: f32) -> Cluster {
    Cluster::new(
        TextRange::new(index, index + 1),
        Text::from(text),
        "test".to_owned(),
        advance,
    )
}

fn decide(
    line_start: i32,
    overflow_at: i32,
    opportunities: &HashMap<i32, ProgressiveBreakOpportunity>,
    clusters: Option<&[Cluster]>,
    line_limit: f32,
    cjk_inter_char_boundaries: &HashSet<i32>,
    max_cjk_stretch_per_gap: f32,
) -> i32 {
    decide_progressive_break(
        line_start,
        overflow_at,
        opportunities,
        clusters,
        line_limit,
        cjk_inter_char_boundaries,
        max_cjk_stretch_per_gap,
        &HashSet::new(),
        0.0,
    )
}

fn allowed(
    line_start: i32,
    raw_greedy: i32,
    candidate_end: i32,
    opportunities: &HashMap<i32, ProgressiveBreakOpportunity>,
    clusters: Option<&[Cluster]>,
) -> bool {
    progressive_candidate_allowed(
        line_start,
        raw_greedy,
        candidate_end,
        opportunities,
        clusters,
        f32::INFINITY,
        &HashSet::new(),
        f32::INFINITY,
        &HashSet::new(),
        0.0,
    )
}

#[test]
fn defaults_admit_the_clean_tier_without_geometry_inputs() {
    let span = TextRange::new(0, 5);
    let opportunities = HashMap::from([
        (1, ProgressiveBreakOpportunity::new(ProgressiveBreakTier::Whitespace, span)),
        (2, ProgressiveBreakOpportunity::new(ProgressiveBreakTier::Emergency, span)),
    ]);

    assert_eq!(1, decide(0, 2, &opportunities, None, f32::INFINITY, &HashSet::new(), f32::INFINITY));
    assert!(allowed(0, 2, 3, &opportunities, None));
}

#[test]
fn line_start_at_the_overflow_boundary_scans_an_empty_range() {
    let span = TextRange::new(0, 5);
    let opportunities = HashMap::from([(
        2,
        ProgressiveBreakOpportunity::new(ProgressiveBreakTier::Emergency, span),
    )]);

    assert_eq!(2, decide(2, 2, &opportunities, None, f32::INFINITY, &HashSet::new(), f32::INFINITY));
}

#[test]
fn two_same_tier_boundaries_pick_the_rightmost() {
    let span = TextRange::new(0, 5);
    let clusters: Vec<_> = (0..5).map(|index| cluster(index, "中", 16.0)).collect();
    let opportunities = HashMap::from([
        (2, ProgressiveBreakOpportunity::new(ProgressiveBreakTier::Whitespace, span)),
        (4, ProgressiveBreakOpportunity::new(ProgressiveBreakTier::Whitespace, span)),
    ]);

    assert_eq!(4, decide(0, 4, &opportunities, Some(&clusters), 64.0, &HashSet::new(), 8.0));
}

#[test]
fn visibly_loose_clean_tiers_fall_through_to_emergency() {
    let span = TextRange::new(0, 5);
    let clusters: Vec<_> = (0..5).map(|index| cluster(index, "中", 16.0)).collect();
    let opportunities = HashMap::from([
        (2, ProgressiveBreakOpportunity::new(ProgressiveBreakTier::Whitespace, span)),
        (4, ProgressiveBreakOpportunity::new(ProgressiveBreakTier::Emergency, span)),
    ]);

    assert_eq!(4, decide(0, 4, &opportunities, Some(&clusters), 200.0, &HashSet::new(), 8.0));
}

#[test]
fn a_leftward_emergency_boundary_keeps_the_best_clean_tier() {
    let span = TextRange::new(0, 5);
    let clusters: Vec<_> = (0..5).map(|index| cluster(index, "中", 16.0)).collect();
    let opportunities = HashMap::from([
        (2, ProgressiveBreakOpportunity::new(ProgressiveBreakTier::Emergency, span)),
        (4, ProgressiveBreakOpportunity::new(ProgressiveBreakTier::Whitespace, span)),
    ]);

    assert_eq!(4, decide(0, 4, &opportunities, Some(&clusters), 200.0, &HashSet::new(), 8.0));
}

#[test]
fn span_edge_and_whitespace_clusters_do_not_count_as_technical_units() {
    let clusters = vec![
        cluster(0, "中", 16.0),
        cluster(1, " ", 16.0),
        cluster(2, "a", 16.0),
        cluster(3, "b", 16.0),
    ];
    let span = TextRange::new(1, 4);
    let opportunities = HashMap::from([
        (2, ProgressiveBreakOpportunity::new(ProgressiveBreakTier::Whitespace, span)),
        (3, ProgressiveBreakOpportunity::new(ProgressiveBreakTier::Emergency, span)),
    ]);

    assert_eq!(3, decide(0, 3, &opportunities, Some(&clusters), 200.0, &HashSet::new(), 8.0));
}

#[test]
fn single_technical_unit_falls_back_to_the_cjk_gap_density() {
    let clusters = vec![cluster(0, "a", 16.0), cluster(1, "b", 16.0), cluster(2, "c", 16.0)];
    let span = TextRange::new(0, 1);
    let opportunities = HashMap::from([
        (1, ProgressiveBreakOpportunity::new(ProgressiveBreakTier::Whitespace, span)),
        (2, ProgressiveBreakOpportunity::new(ProgressiveBreakTier::Emergency, span)),
    ]);

    assert_eq!(2, decide(0, 2, &opportunities, Some(&clusters), 200.0, &HashSet::from([1]), 8.0));
}

#[test]
fn candidate_outside_the_cluster_list_is_allowed() {
    let span = TextRange::new(0, 5);
    let opportunities = HashMap::from([(
        1,
        ProgressiveBreakOpportunity::new(ProgressiveBreakTier::Emergency, span),
    )]);

    assert!(allowed(0, 1, 5, &opportunities, Some(&[cluster(0, "中", 16.0)])));
}

#[test]
fn candidates_outside_the_active_span_are_allowed() {
    let clusters: Vec<_> = (0..4).map(|index| cluster(index, "中", 16.0)).collect();
    let leading_span = HashMap::from([(
        1,
        ProgressiveBreakOpportunity::new(ProgressiveBreakTier::Emergency, TextRange::new(5, 10)),
    )]);
    assert!(allowed(0, 1, 2, &leading_span, Some(&clusters)));

    let trailing_span = HashMap::from([(
        1,
        ProgressiveBreakOpportunity::new(ProgressiveBreakTier::Emergency, TextRange::new(0, 2)),
    )]);
    assert!(allowed(0, 1, 2, &trailing_span, Some(&clusters)));

    let inner_span = HashMap::from([(
        1,
        ProgressiveBreakOpportunity::new(ProgressiveBreakTier::Emergency, TextRange::new(0, 4)),
    )]);
    assert!(!allowed(0, 1, 2, &inner_span, Some(&clusters)));
}

#[test]
fn candidates_of_a_different_span_are_allowed() {
    let opportunities = HashMap::from([
        (1, ProgressiveBreakOpportunity::new(ProgressiveBreakTier::Emergency, TextRange::new(0, 2))),
        (3, ProgressiveBreakOpportunity::new(ProgressiveBreakTier::Whitespace, TextRange::new(2, 6))),
    ]);

    assert!(allowed(0, 1, 3, &opportunities, None));
}

#[test]
fn same_tier_past_the_raw_greedy_is_allowed_and_worse_tiers_are_not() {
    let span = TextRange::new(0, 5);
    let opportunities = HashMap::from([
        (2, ProgressiveBreakOpportunity::new(ProgressiveBreakTier::Whitespace, span)),
        (3, ProgressiveBreakOpportunity::new(ProgressiveBreakTier::Whitespace, span)),
        (4, ProgressiveBreakOpportunity::new(ProgressiveBreakTier::Emergency, span)),
    ]);

    assert!(allowed(0, 2, 3, &opportunities, None));
    assert!(!allowed(0, 2, 4, &opportunities, None));
}

#[test]
fn candidates_before_the_raw_greedy_must_match_the_selected_boundary() {
    let span = TextRange::new(0, 5);
    let opportunities = HashMap::from([
        (1, ProgressiveBreakOpportunity::new(ProgressiveBreakTier::Whitespace, span)),
        (2, ProgressiveBreakOpportunity::new(ProgressiveBreakTier::Whitespace, span)),
        (3, ProgressiveBreakOpportunity::new(ProgressiveBreakTier::Emergency, span)),
    ]);

    assert!(allowed(0, 3, 2, &opportunities, None));
    assert!(!allowed(0, 3, 1, &opportunities, None));
}

#[test]
fn hyphen_break_returns_overflow_at_plain_word_boundaries() {
    let clusters: Vec<_> = (0..3).map(|index| cluster(index, "中", 16.0)).collect();

    assert_eq!(
        1,
        decide_hyphen_break(0, 1, &clusters, 16.0, &HashSet::new(), &HashSet::new(), 8.0, &HashSet::new(), 0.0)
    );
}

#[test]
fn over_long_words_must_hyphenate_from_the_line_start() {
    let clusters: Vec<_> = (0..3).map(|index| cluster(index, "中", 16.0)).collect();

    assert_eq!(
        2,
        decide_hyphen_break(0, 2, &clusters, 48.0, &HashSet::from([0, 1, 2]), &HashSet::new(), 8.0, &HashSet::new(), 0.0)
    );
}

#[test]
fn a_fitting_whole_word_breaks_there() {
    let clusters: Vec<_> = (0..3).map(|index| cluster(index, "中", 16.0)).collect();

    assert_eq!(
        1,
        decide_hyphen_break(0, 2, &clusters, 16.0, &HashSet::from([2]), &HashSet::new(), 8.0, &HashSet::new(), 0.0)
    );
}

#[test]
fn sino_western_gaps_absorbing_the_deficit_keep_the_whole_word() {
    let clusters: Vec<_> = (0..4).map(|index| cluster(index, "中", 16.0)).collect();

    assert_eq!(
        2,
        decide_hyphen_break(0, 3, &clusters, 40.0, &HashSet::from([3]), &HashSet::from([1]), 8.0, &HashSet::from([1]), 8.0)
    );
}

#[test]
fn gapless_or_too_loose_lines_hyphenate_instead() {
    let clusters: Vec<_> = (0..4).map(|index| cluster(index, "中", 16.0)).collect();
    assert_eq!(
        3,
        decide_hyphen_break(0, 3, &clusters, 60.0, &HashSet::from([3]), &HashSet::from([2]), 8.0, &HashSet::new(), 0.0)
    );
    assert_eq!(
        3,
        decide_hyphen_break(0, 3, &clusters, 100.0, &HashSet::from([3]), &HashSet::from([1]), 8.0, &HashSet::new(), 0.0)
    );
    assert_eq!(
        2,
        decide_hyphen_break(0, 3, &clusters, 36.0, &HashSet::from([3]), &HashSet::from([1]), 8.0, &HashSet::new(), 0.0)
    );
}
