use tiqian::core::geometry::{text_range};
use tiqian::core::int_range::IntRange;
use tiqian::core::layout_model::Cluster;
use tiqian::core::text::Text;
use tiqian::layout::line_breaker::{LineBreaker, LineBreakerConfig};
use tiqian::layout::line_optimization::RepairOption;
use tiqian::layout::paragraph_dp_line_breaker::ParagraphDpLineBreaker;
use tiqian::layout::progressive_break_decisions::{
    ProgressiveBreakOpportunity, ProgressiveBreakTier, ShrinkChannel, ShrinkOpportunity,
};

fn cluster(index: i32, text: &str, advance: f32) -> Cluster {
    Cluster::new(
        text_range(index, index + 1),
        Text::from(text),
        "test".to_owned(),
        advance,
    )
}

fn han_clusters(count: i32) -> Vec<Cluster> {
    (0..count).map(|index| cluster(index, "中", 16.0)).collect()
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
fn foreign_span_candidate_survives_the_promotion_pool_purge() {
    let clusters = latin_clusters();
    let span = text_range(0, clusters.len() as i32);
    let mut config = LineBreakerConfig::default();
    config.shrink_opportunities = vec![ShrinkOpportunity::new(2, 2, 5.0, ShrinkChannel::RawAdvance)];
    config.line_adjustment_push_in = true;
    config.progressive_break_opportunities = tiqian::common::HashMap::from([
        (
            1,
            ProgressiveBreakOpportunity::new(
                ProgressiveBreakTier::Emergency,
                text_range(0, 1),
            ),
        ),
        (2, ProgressiveBreakOpportunity::new(ProgressiveBreakTier::Emergency, span)),
        (3, ProgressiveBreakOpportunity::new(ProgressiveBreakTier::Whitespace, span)),
    ]);

    let solution = ParagraphDpLineBreaker::default().break_lines(&clusters, &clusters, 80.0, &config);
    assert!(matches!(&solution.lines[0].repair, Some(RepairOption::PushIn { reason, .. }) if reason.starts_with("ProgressiveTechnicalTierPromotion")), "{:?}", solution.lines);
}

#[test]
fn committed_compressed_line_with_foreign_span_opportunities_keeps_plain_push_in_reason() {
    let clusters = han_clusters(4);
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
    config.progressive_break_opportunities = tiqian::common::HashMap::from([
        (
            2,
            ProgressiveBreakOpportunity::new(
                ProgressiveBreakTier::Emergency,
                text_range(0, 2),
            ),
        ),
        (
            3,
            ProgressiveBreakOpportunity::new(
                ProgressiveBreakTier::Whitespace,
                text_range(2, 4),
            ),
        ),
    ]);

    let solution = ParagraphDpLineBreaker::default().break_lines(&clusters, &clusters, 44.0, &config);
    let first = &solution.lines[0];
    assert_eq!(IntRange::new(0, 2), first.cluster_range, "{:?}", solution.lines);
    assert!(matches!(&first.repair, Some(RepairOption::PushIn { reason, .. }) if reason.starts_with("LineAdjustmentPushIn")), "{:?}", solution.lines);
}

#[test]
fn committed_compressed_end_without_opportunity_keeps_plain_push_in_reason() {
    let clusters = han_clusters(4);
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
    config.progressive_break_opportunities = tiqian::common::HashMap::from([(
        2,
        ProgressiveBreakOpportunity::new(
            ProgressiveBreakTier::Emergency,
            text_range(0, 2),
        ),
    )]);

    let solution = ParagraphDpLineBreaker::default().break_lines(&clusters, &clusters, 44.0, &config);
    let first = &solution.lines[0];
    assert_eq!(IntRange::new(0, 2), first.cluster_range, "{:?}", solution.lines);
    assert!(matches!(&first.repair, Some(RepairOption::PushIn { reason, .. }) if reason.starts_with("LineAdjustmentPushIn")), "{:?}", solution.lines);
}
