use tiqian::common::HashSet;

use tiqian::core::east_asian_spacing::unicode_east_asian_spacing;
use tiqian::core::geometry::TextRange;
use tiqian::core::int_range::IntRange;
use tiqian::core::layout_model::Cluster;
use tiqian::core::text::Text;
use tiqian::font::font_policy::FontRole;
use tiqian::layout::justifier::{JustificationRequest, Justifier};
use tiqian::layout::punctuation_model::GlueKind;

const EM: f32 = 16.0;

fn cluster(start: i32, end: i32, text: &str, advance: f32, font_key: &str) -> Cluster {
    Cluster::new(
        TextRange::new(start, end),
        Text::from(text),
        font_key.to_owned(),
        advance,
    )
}

fn edges(clusters: &[Cluster]) -> Vec<tiqian::core::east_asian_spacing::EastAsianSpacingEdges> {
    clusters
        .iter()
        .map(|cluster| unicode_east_asian_spacing::resolved_edges(&cluster.text, "zh-Hans"))
        .collect()
}

fn justify(
    clusters: &[Cluster],
    roles: &[FontRole],
    max_width: f32,
    configure: impl FnOnce(&mut JustificationRequest<'_>),
) -> tiqian::layout::justifier::JustificationPlan {
    let spacing = edges(clusters);
    let mut request = JustificationRequest::new(
        clusters,
        roles,
        &spacing,
        IntRange::new(0, clusters.len() as i32 - 1),
        max_width,
        EM,
        0.25,
        0.5,
    );
    configure(&mut request);
    Justifier::default().justify(request)
}

#[test]
fn western_dominant_line_does_not_stretch_around_cjk_punctuation() {
    let clusters = vec![
        cluster(0, 2, "Hi", 48.0, "latin"),
        cluster(2, 3, "（", 16.0, "cjk"),
        cluster(3, 5, "Hi", 48.0, "latin"),
        cluster(5, 6, "）", 16.0, "cjk"),
        cluster(6, 7, "、", 16.0, "cjk"),
        cluster(7, 9, "Hi", 48.0, "latin"),
    ];
    let roles = vec![
        FontRole::LatinText,
        FontRole::CjkPunctuation,
        FontRole::LatinText,
        FontRole::CjkPunctuation,
        FontRole::CjkPunctuation,
        FontRole::LatinText,
    ];
    let natural: f32 = clusters.iter().map(|cluster| cluster.advance).sum();
    let plan = justify(&clusters, &roles, natural + 2.0 * EM, |_| {});

    assert!(
        plan.allocations
            .iter()
            .all(|allocation| allocation.kind != GlueKind::CjkInterChar)
    );
    assert_eq!(2.0 * EM, plan.unfilled_deficit);
    assert_eq!(
        Some("WesternDominantLineNaturalSpacing".to_owned()),
        plan.fallback_reason
    );
}

#[test]
fn typed_sino_western_space_stretches_in_tier_two() {
    let clusters = vec![
        cluster(0, 1, "中", EM, "cjk"),
        cluster(1, 2, " ", 0.25 * EM, "latin"),
        cluster(2, 4, "Hi", 2.0 * EM, "latin"),
    ];
    let roles = vec![FontRole::CjkText, FontRole::LatinText, FontRole::LatinText];
    let natural: f32 = clusters.iter().map(|cluster| cluster.advance).sum();
    let plan = justify(&clusters, &roles, natural + 0.2 * EM, |_| {});

    assert_eq!(0.0, plan.unfilled_deficit);
    assert_eq!(1, plan.allocations.len());
    assert_eq!(1, plan.allocations[0].target_cluster_index);
    assert_eq!(GlueKind::CjkLatinSpace, plan.allocations[0].kind);
    assert!((plan.allocations[0].delta - 0.2 * EM).abs() < 0.001);
}

#[test]
fn typed_sino_western_space_is_capped_before_final_uniform_spacing() {
    let clusters = vec![
        cluster(0, 1, "中", EM, "cjk"),
        cluster(1, 2, " ", 0.25 * EM, "latin"),
        cluster(2, 4, "Hi", 2.0 * EM, "latin"),
        cluster(4, 5, "中", EM, "cjk"),
        cluster(5, 6, "中", EM, "cjk"),
    ];
    let roles = vec![
        FontRole::CjkText,
        FontRole::LatinText,
        FontRole::LatinText,
        FontRole::CjkText,
        FontRole::CjkText,
    ];
    let natural: f32 = clusters.iter().map(|cluster| cluster.advance).sum();
    let plan = justify(&clusters, &roles, natural + 2.0 * EM, |_| {});

    let sino: Vec<_> = plan
        .allocations
        .iter()
        .filter(|allocation| allocation.kind == GlueKind::CjkLatinSpace)
        .collect();
    assert_eq!(
        HashSet::from([1, 2]),
        sino.iter()
            .map(|allocation| allocation.target_cluster_index)
            .collect::<HashSet<_>>()
    );
    assert!(
        sino.iter()
            .all(|allocation| (allocation.delta - 0.25 * EM).abs() < 0.001)
    );
    let uniform: Vec<_> = plan
        .allocations
        .iter()
        .filter(|allocation| allocation.kind == GlueKind::CjkInterChar)
        .collect();
    assert_eq!(
        HashSet::from([1, 2, 3]),
        uniform
            .iter()
            .map(|allocation| allocation.target_cluster_index)
            .collect::<HashSet<_>>()
    );
    assert!(
        uniform
            .iter()
            .all(|allocation| (allocation.delta - 0.5 * EM).abs() < 0.001)
    );
    assert_eq!(0.0, plan.unfilled_deficit);
}

#[test]
fn inseparable_number_symbol_boundary_never_stretches() {
    let clusters = vec![
        cluster(0, 1, "中", EM, "cjk"),
        cluster(1, 3, "50", 2.0 * EM, "latin"),
        cluster(3, 4, "%", EM, "cjk"),
        cluster(4, 5, "中", EM, "cjk"),
        cluster(5, 6, "中", EM, "cjk"),
    ];
    let roles = vec![
        FontRole::CjkText,
        FontRole::LatinText,
        FontRole::CjkPunctuation,
        FontRole::CjkText,
        FontRole::CjkText,
    ];
    let natural: f32 = clusters.iter().map(|cluster| cluster.advance).sum();
    let plan = justify(&clusters, &roles, natural + EM, |request| {
        request.no_stretch_boundary_after_clusters = HashSet::from([1]);
    });

    assert!(
        plan.allocations
            .iter()
            .all(|allocation| !(allocation.target_cluster_index == 1
                && matches!(
                    allocation.kind,
                    GlueKind::CjkLatinSpace | GlueKind::CjkInterChar
                )))
    );
    assert_eq!(0.0, plan.unfilled_deficit);
}

#[test]
fn fixed_sino_western_gap_does_not_join_final_uniform_spacing() {
    let clusters = vec![
        cluster(0, 1, "中", EM, "cjk"),
        cluster(1, 3, "Hi", 2.0 * EM, "latin"),
        cluster(3, 4, "中", EM, "cjk"),
        cluster(4, 5, "中", EM, "cjk"),
    ];
    let roles = vec![
        FontRole::CjkText,
        FontRole::LatinText,
        FontRole::CjkText,
        FontRole::CjkText,
    ];
    let natural: f32 = clusters.iter().map(|cluster| cluster.advance).sum();
    let plan = justify(&clusters, &roles, natural + EM, |request| {
        request.allow_sino_western_gap_stretch = false;
    });

    assert_eq!(1, plan.allocations.len());
    assert_eq!(GlueKind::CjkInterChar, plan.allocations[0].kind);
    assert_eq!(2, plan.allocations[0].target_cluster_index);
    assert_eq!(EM, plan.allocations[0].delta);
    assert_eq!(0.0, plan.unfilled_deficit);
}
