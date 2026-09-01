use tiqian::common::HashMap;
use tiqian::core::geometry::TextRange;
use tiqian::core::layout_model::Cluster;
use tiqian::core::text::Text;
use tiqian::layout::progressive_break_decisions::{
    ProgressiveBreakOpportunity, ProgressiveBreakTier, decide_progressive_break,
};

fn cluster(index: i32) -> Cluster {
    Cluster::new(
        TextRange::new(index, index + 1),
        Text::from("中"),
        "test".to_owned(),
        16.0,
    )
}

fn opportunities() -> HashMap<i32, ProgressiveBreakOpportunity> {
    let span = TextRange::new(0, 5);
    HashMap::from([
        (2, ProgressiveBreakOpportunity::new(ProgressiveBreakTier::Whitespace, span)),
        (4, ProgressiveBreakOpportunity::new(ProgressiveBreakTier::Emergency, span)),
    ])
}

#[test]
fn infinite_line_limit_with_clusters_admits_the_cleanest_tier() {
    let clusters: Vec<_> = (0..5).map(cluster).collect();
    assert_eq!(
        2,
        decide_progressive_break(
            0,
            4,
            &opportunities(),
            Some(&clusters),
            f32::INFINITY,
            &Default::default(),
            8.0,
            &Default::default(),
            0.0,
        )
    );
}

#[test]
fn infinite_stretch_ceiling_with_finite_line_limit_admits_the_cleanest_tier() {
    let clusters: Vec<_> = (0..5).map(cluster).collect();
    assert_eq!(
        2,
        decide_progressive_break(
            0,
            4,
            &opportunities(),
            Some(&clusters),
            200.0,
            &Default::default(),
            f32::INFINITY,
            &Default::default(),
            0.0,
        )
    );
}
