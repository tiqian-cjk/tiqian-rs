use tiqian::common::{HashMap, HashSet};

use tiqian::core::east_asian_spacing::unicode_east_asian_spacing;
use tiqian::core::geometry::TextRange;
use tiqian::core::int_range::IntRange;
use tiqian::core::layout_model::Cluster;
use tiqian::core::text::Text;
use tiqian::core::text_model::{InlineObjectPreferredStretch, InlineObjectPreferredStretchKind};
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

#[test]
fn explicit_inline_object_boundaries_share_uniform_stretch_on_formula_only_line() {
    let clusters = vec![
        cluster(0, 2, "a+", 2.0 * EM, "inline-object"),
        cluster(2, 4, "b=", 2.0 * EM, "inline-object"),
        cluster(4, 5, "c", 2.0 * EM, "inline-object"),
    ];
    let roles = vec![FontRole::Unknown; 3];
    let natural: f32 = clusters.iter().map(|cluster| cluster.advance).sum();
    let plan = justify(&clusters, &roles, natural + EM, |request| {
        request.uniform_inline_object_boundary_after_clusters = HashSet::from([0, 1]);
    });

    assert_eq!(0.0, plan.unfilled_deficit);
    assert_eq!(
        HashSet::from([0, 1]),
        plan.allocations
            .iter()
            .map(|allocation| allocation.target_cluster_index)
            .collect::<HashSet<i32>>()
    );
    assert!(plan.allocations.iter().all(|allocation| allocation.kind == GlueKind::InlineObjectBoundary));
    assert!(plan.allocations.iter().all(|allocation| (allocation.delta - 0.5 * EM).abs() < 0.001));
}

#[test]
fn formula_boundaries_stretch_punctuation_then_relations_then_binary_operators() {
    let clusters: Vec<_> = ["a", ",", "b", "=", "c", "+", "d"]
        .into_iter()
        .enumerate()
        .map(|(index, text)| cluster(index as i32, index as i32 + 1, text, 2.0 * EM, "inline-object"))
        .collect();
    let roles = vec![FontRole::Unknown; clusters.len()];
    let natural: f32 = clusters.iter().map(|cluster| cluster.advance).sum();
    let preferred = HashMap::from([
        (1, InlineObjectPreferredStretch::new(InlineObjectPreferredStretchKind::PunctuationTrailing, 1.0, 8.0)),
        (2, InlineObjectPreferredStretch::new(InlineObjectPreferredStretchKind::Relation, 2.0, 8.0)),
        (3, InlineObjectPreferredStretch::new(InlineObjectPreferredStretchKind::Relation, 2.0, 8.0)),
        (4, InlineObjectPreferredStretch::new(InlineObjectPreferredStretchKind::BinaryOperator, 3.0, 8.0)),
        (5, InlineObjectPreferredStretch::new(InlineObjectPreferredStretchKind::BinaryOperator, 3.0, 8.0)),
    ]);
    let preferred_only = justify(&clusters, &roles, natural + 24.0, |request| {
        request.preferred_inline_object_boundary_after_clusters = preferred.clone();
    });

    assert_eq!(
        vec![GlueKind::InlineObjectPunctuationTrailing, GlueKind::InlineObjectRelation, GlueKind::InlineObjectRelation, GlueKind::InlineObjectBinaryOperator, GlueKind::InlineObjectBinaryOperator],
        preferred_only.allocations.iter().map(|allocation| allocation.kind).collect::<Vec<_>>(),
    );
    assert_eq!(vec![7.0, 6.0, 6.0, 2.5, 2.5], preferred_only.allocations.iter().map(|allocation| allocation.delta).collect::<Vec<_>>());
    assert_eq!(0.0, preferred_only.unfilled_deficit);

    let with_final_uniform = justify(&clusters, &roles, natural + 34.0, |request| {
        request.preferred_inline_object_boundary_after_clusters = preferred.clone();
        request.uniform_inline_object_boundary_after_clusters = preferred.keys().copied().collect();
    });
    assert_eq!(29.0, with_final_uniform.allocations.iter().take(5).map(|allocation| allocation.delta).sum::<f32>());
    let uniform: Vec<_> = with_final_uniform.allocations.iter().filter(|allocation| allocation.kind == GlueKind::InlineObjectBoundary).collect();
    assert_eq!(preferred.keys().copied().collect::<HashSet<_>>(), uniform.iter().map(|allocation| allocation.target_cluster_index).collect());
    assert!(uniform.iter().all(|allocation| (allocation.delta - 1.0).abs() < 0.001));
    assert_eq!(0.0, with_final_uniform.unfilled_deficit);
}

#[test]
fn mixed_cjk_line_still_stretches_punctuation_western_boundary() {
    let clusters = vec![cluster(0, 2, "Hi", 2.0 * EM, "latin"), cluster(2, 3, "（", EM, "cjk"), cluster(3, 4, "中", EM, "cjk")];
    let roles = vec![FontRole::LatinText, FontRole::CjkPunctuation, FontRole::CjkText];
    let natural: f32 = clusters.iter().map(|cluster| cluster.advance).sum();
    let plan = justify(&clusters, &roles, natural + 0.5 * EM, |_| {});

    assert!(plan.allocations.iter().any(|allocation| allocation.kind == GlueKind::CjkInterChar && allocation.target_cluster_index == 0));
    assert_eq!(0.0, plan.unfilled_deficit);
    assert_eq!(None, plan.fallback_reason);
}

#[test]
fn final_uniform_spacing_includes_word_and_sino_western_gaps_once_each() {
    let clusters = vec![
        cluster(0, 1, "中", EM, "cjk"),
        cluster(1, 3, "Hi", 2.0 * EM, "latin"),
        cluster(3, 4, " ", 0.25 * EM, "latin"),
        cluster(4, 6, "Hi", 2.0 * EM, "latin"),
        cluster(6, 7, "中", EM, "cjk"),
        cluster(7, 8, "中", EM, "cjk"),
    ];
    let roles = vec![
        FontRole::CjkText,
        FontRole::LatinText,
        FontRole::LatinText,
        FontRole::LatinText,
        FontRole::CjkText,
        FontRole::CjkText,
    ];
    let natural: f32 = clusters.iter().map(|cluster| cluster.advance).sum();
    let plan = justify(&clusters, &roles, natural + 2.25 * EM, |_| {});

    let word: Vec<_> = plan.allocations.iter().filter(|allocation| allocation.kind == GlueKind::WordSpace).collect();
    assert_eq!(vec![2], word.iter().map(|allocation| allocation.target_cluster_index).collect::<Vec<_>>());
    assert!((word[0].delta - 0.25 * EM).abs() < 0.001);
    let sino: Vec<_> = plan.allocations.iter().filter(|allocation| allocation.kind == GlueKind::CjkLatinSpace).collect();
    assert_eq!(vec![0, 3], sino.iter().map(|allocation| allocation.target_cluster_index).collect::<Vec<_>>());
    assert!(sino.iter().all(|allocation| (allocation.delta - 0.25 * EM).abs() < 0.001));
    let uniform: Vec<_> = plan.allocations.iter().filter(|allocation| allocation.kind == GlueKind::CjkInterChar).collect();
    assert_eq!(vec![0, 3, 4, 2], uniform.iter().map(|allocation| allocation.target_cluster_index).collect::<Vec<_>>());
    assert!(uniform.iter().all(|allocation| (allocation.delta - 0.375 * EM).abs() < 0.001));
    assert_eq!(0.0, plan.unfilled_deficit);
}

#[test]
fn western_brackets_touching_cjk_share_tier_three_stretch() {
    let clusters = vec![
        cluster(0, 1, "中", EM, "cjk"),
        cluster(1, 2, "(", 0.5 * EM, "latin"),
        cluster(2, 3, "中", EM, "cjk"),
        cluster(3, 4, ")", 0.5 * EM, "latin"),
        cluster(4, 5, "中", EM, "cjk"),
    ];
    let roles = vec![FontRole::CjkText, FontRole::LatinText, FontRole::CjkText, FontRole::LatinText, FontRole::CjkText];
    let natural: f32 = clusters.iter().map(|cluster| cluster.advance).sum();
    let plan = justify(&clusters, &roles, natural + EM, |request| {
        request.western_bracket_cjk_inter_char_boundary_after_clusters = HashSet::from([0, 1, 2, 3]);
    });

    let allocations: Vec<_> = plan.allocations.iter().filter(|allocation| allocation.kind == GlueKind::CjkInterChar).collect();
    assert_eq!(HashSet::from([0, 1, 2, 3]), allocations.iter().map(|allocation| allocation.target_cluster_index).collect::<HashSet<i32>>());
    assert!(allocations.iter().all(|allocation| allocation.reason == "WesternBracketCjkInterChar" && (allocation.delta - 0.25 * EM).abs() < 0.001));
    assert_eq!(0.0, plan.unfilled_deficit);
}

#[test]
fn attached_reference_uses_the_virtual_prose_boundary_for_stretching() {
    let clusters = vec![
        cluster(0, 1, "中", EM, "cjk"),
        cluster(1, 2, "[", 0.5 * EM, "latin"),
        cluster(2, 4, "Hi", EM, "latin"),
        cluster(4, 5, "]", 0.5 * EM, "latin"),
        cluster(5, 6, "中", EM, "cjk"),
    ];
    let roles = vec![FontRole::CjkText, FontRole::LatinText, FontRole::LatinText, FontRole::LatinText, FontRole::CjkText];
    let natural: f32 = clusters.iter().map(|cluster| cluster.advance).sum();
    let middle = justify(&clusters, &roles, natural + EM, |request| {
        request.attached_inline_physical_boundary_after_clusters = HashSet::from([0, 3]);
        request.attached_inline_virtual_boundary_after_clusters = HashMap::from([(3, 0)]);
    });

    assert_eq!(1, middle.allocations.len());
    assert_eq!(3, middle.allocations[0].target_cluster_index);
    assert_eq!("AttachedInlineVirtualInterChar", middle.allocations[0].reason);
    assert_eq!(EM, middle.allocations[0].delta);

    let line_end_width: f32 = clusters[..4].iter().map(|cluster| cluster.advance).sum();
    let at_line_end = justify(&clusters, &roles, line_end_width + EM, |request| {
        request.line_cluster_range = IntRange::new(0, 3);
        request.attached_inline_physical_boundary_after_clusters = HashSet::from([0, 3]);
        request.attached_inline_virtual_boundary_after_clusters = HashMap::from([(3, 0)]);
    });
    assert!(at_line_end.allocations.is_empty());
}

#[test]
fn virtual_sino_western_stretch_requires_alpha_numeric_boundary_char() {
    let clusters = vec![
        cluster(0, 1, "中", EM, "cjk"),
        cluster(1, 4, "/Hi", 2.0 * EM, "latin"),
        cluster(4, 5, "中", EM, "cjk"),
    ];
    let roles = vec![FontRole::CjkText, FontRole::LatinText, FontRole::CjkText];
    let natural: f32 = clusters.iter().map(|cluster| cluster.advance).sum();
    let plan = justify(&clusters, &roles, natural + 0.2 * EM, |_| {});

    let sino: Vec<_> = plan.allocations.iter().filter(|allocation| allocation.kind == GlueKind::CjkLatinSpace).collect();
    assert_eq!(vec![1], sino.iter().map(|allocation| allocation.target_cluster_index).collect::<Vec<_>>());
}

#[test]
fn typed_space_before_slash_led_latin_run_is_not_sino_western_gap() {
    let clusters = vec![
        cluster(0, 1, "中", EM, "cjk"),
        cluster(1, 2, " ", 0.25 * EM, "latin"),
        cluster(2, 5, "/Hi", 2.0 * EM, "latin"),
    ];
    let roles = vec![FontRole::CjkText, FontRole::LatinText, FontRole::LatinText];
    let natural: f32 = clusters.iter().map(|cluster| cluster.advance).sum();
    let plan = justify(&clusters, &roles, natural + 0.2 * EM, |_| {});

    assert!(plan.allocations.iter().all(|allocation| allocation.kind != GlueKind::CjkLatinSpace));
}

#[test]
fn sino_western_stretch_respects_third_em_cap_when_style_sets_it() {
    let clusters = vec![
        cluster(0, 1, "中", EM, "cjk"),
        cluster(1, 2, " ", 0.25 * EM, "latin"),
        cluster(2, 4, "Hi", 2.0 * EM, "latin"),
    ];
    let roles = vec![FontRole::CjkText, FontRole::LatinText, FontRole::LatinText];
    let natural: f32 = clusters.iter().map(|cluster| cluster.advance).sum();
    let plan = justify(&clusters, &roles, natural + EM, |request| {
        request.cjk_latin_space_max_em = 1.0 / 3.0;
    });

    let sino = plan.allocations.iter().find(|allocation| allocation.kind == GlueKind::CjkLatinSpace).expect("expected CJK-Latin allocation");
    assert!((sino.delta - (1.0 / 3.0 - 0.25) * EM).abs() < 0.001);
}
