use std::collections::{HashMap, HashSet};

use tiqian::org::tiqian::core::Geometry::TextRange;
use tiqian::org::tiqian::core::LayoutModel::Cluster;
use tiqian::org::tiqian::layout::ProgressiveBreakDecisions::{
    ProgressiveBreakOpportunity, ProgressiveBreakTier, decide_progressive_break,
    progressive_candidate_allowed,
};

fn cluster(index: i32, text: &str, advance: f32) -> Cluster {
    Cluster::new(
        TextRange::new(index, index + 1),
        text.to_owned(),
        "test".to_owned(),
        advance,
    )
}

#[test]
fn source_whitespace_capacity_keeps_structural_tier_ahead_of_syllable() {
    let span = TextRange::new(0, 6);
    let clusters = vec![
        cluster(0, "a", 20.0),
        cluster(1, " ", 4.0),
        cluster(2, "b", 28.0),
        cluster(3, "/", 28.0),
        cluster(4, "c", 2.0),
        cluster(5, "d", 20.0),
    ];
    let opportunities = HashMap::from([
        (
            2,
            ProgressiveBreakOpportunity::with_preceding_whitespace_stretch_capacity(
                ProgressiveBreakTier::Whitespace,
                span,
                4.0,
            ),
        ),
        (
            4,
            ProgressiveBreakOpportunity::new(ProgressiveBreakTier::Structural, span),
        ),
        (
            5,
            ProgressiveBreakOpportunity::new(ProgressiveBreakTier::Syllable, span),
        ),
    ]);

    assert_eq!(
        4,
        decide_progressive_break(
            0,
            5,
            &opportunities,
            Some(&clusters),
            84.0,
            &HashSet::new(),
            8.0,
            &HashSet::new(),
            8.0,
        ),
    );
}

#[test]
fn lookahead_cannot_replace_selected_emergency_boundary_with_earlier_same_tier_cut() {
    let span = TextRange::new(0, 5);
    let clusters: Vec<_> = (0..5).map(|index| cluster(index, "a", 20.0)).collect();
    let opportunities = HashMap::from([
        (
            3,
            ProgressiveBreakOpportunity::new(ProgressiveBreakTier::Emergency, span),
        ),
        (
            4,
            ProgressiveBreakOpportunity::new(ProgressiveBreakTier::Emergency, span),
        ),
    ]);
    let allowed = |candidate_end| {
        progressive_candidate_allowed(
            0,
            4,
            candidate_end,
            &opportunities,
            Some(&clusters),
            90.0,
            &HashSet::new(),
            8.0,
            &HashSet::new(),
            8.0,
        )
    };

    assert!(!allowed(3));
    assert!(allowed(4));
}
