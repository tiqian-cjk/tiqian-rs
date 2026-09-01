use tiqian::common::{HashMap, HashSet};
use tiqian::core::east_asian_spacing::{EastAsianSpacingEdges, EastAsianSpacingValue};
use tiqian::core::geometry::TextRange;
use tiqian::core::int_range::IntRange;
use tiqian::core::layout_model::Cluster;
use tiqian::core::text::Text;
use tiqian::core::text_model::{InlineObjectPreferredStretch, InlineObjectPreferredStretchKind};
use tiqian::font::font_policy::FontRole;
use tiqian::layout::justifier::{JustificationRequest, Justifier};
use tiqian::layout::punctuation_model::GlueKind;
use tiqian::layout::progressive_break_decisions::{ShrinkChannel, ShrinkOpportunity};

const EM: f32 = 16.0;

fn cluster(index: i32, text: &str, advance: f32) -> Cluster {
    Cluster::new(
        TextRange::new(index, index + text.encode_utf16().count() as i32),
        Text::from(text),
        "k".to_owned(),
        advance,
    )
}

fn edges(
    leading: EastAsianSpacingValue,
    trailing: EastAsianSpacingValue,
    contains_wide: bool,
) -> EastAsianSpacingEdges {
    EastAsianSpacingEdges {
        leading,
        trailing,
        contains_wide,
    }
}

fn justify(
    clusters: &[Cluster],
    roles: &[FontRole],
    spacing: &[EastAsianSpacingEdges],
    range: IntRange,
    max_width: f32,
    configure: impl FnOnce(&mut JustificationRequest<'_>),
) -> tiqian::layout::justifier::JustificationPlan {
    let mut request = JustificationRequest::new(clusters, roles, spacing, range, max_width, EM, 0.25, 0.5);
    configure(&mut request);
    Justifier::default().justify(request)
}

#[test]
fn attached_inline_virtual_sino_western_boundary_out_of_bounds() {
    let clusters = vec![cluster(0, "中", EM), cluster(1, "文", EM), cluster(2, "a", EM)];
    let roles = vec![FontRole::CjkText, FontRole::CjkText, FontRole::LatinText];
    let spacing = vec![
        edges(EastAsianSpacingValue::Wide, EastAsianSpacingValue::Wide, true),
        edges(EastAsianSpacingValue::Wide, EastAsianSpacingValue::Wide, true),
        edges(EastAsianSpacingValue::Narrow, EastAsianSpacingValue::Narrow, false),
    ];
    let plan = justify(&clusters, &roles, &spacing, IntRange::new(0, 2), 60.0, |request| {
        request.attached_inline_virtual_boundary_after_clusters = HashMap::from([(-1, -2), (2, 1), (5, 4)]);
        request.attached_inline_virtual_sino_western_boundary_after_clusters = HashSet::from([-1, 2, 5]);
    });

    assert!(!plan.allocations.is_empty());
}

#[test]
fn preferred_inline_object_boundary_out_of_bounds() {
    let clusters = vec![cluster(0, "中", EM), cluster(1, "文", EM), cluster(2, "字", EM)];
    let roles = vec![FontRole::CjkText, FontRole::CjkText, FontRole::CjkText];
    let spacing = vec![edges(EastAsianSpacingValue::Wide, EastAsianSpacingValue::Wide, true); 3];
    let relation = InlineObjectPreferredStretch::new(InlineObjectPreferredStretchKind::Relation, 0.0, 4.0);
    let plan = justify(&clusters, &roles, &spacing, IntRange::new(0, 2), 60.0, |request| {
        request.preferred_inline_object_boundary_after_clusters = HashMap::from([(-1, relation), (2, relation), (5, relation)]);
    });

    assert!(plan.allocations.iter().all(|allocation| allocation.kind != GlueKind::InlineObjectRelation));
    assert!(!plan.allocations.is_empty());
}

#[test]
fn closed_space_gap_in_typed_sino_western_and_uniform_space() {
    let clusters = vec![cluster(0, "中", EM), cluster(1, " ", 4.0), cluster(2, "a", EM)];
    let roles = vec![FontRole::CjkText, FontRole::LatinText, FontRole::LatinText];
    let spacing = vec![
        edges(EastAsianSpacingValue::Wide, EastAsianSpacingValue::Wide, true),
        edges(EastAsianSpacingValue::Other, EastAsianSpacingValue::Other, false),
        edges(EastAsianSpacingValue::Narrow, EastAsianSpacingValue::Narrow, false),
    ];
    let plan = justify(&clusters, &roles, &spacing, IntRange::new(0, 2), 60.0, |request| {
        request.no_stretch_boundary_clusters = HashSet::from([2]);
    });

    assert!(plan.allocations.iter().all(|allocation| allocation.target_cluster_index != 1));
}

#[test]
fn closed_space_gap_in_uniform_space_when_word_space() {
    let clusters = vec![cluster(0, "a", EM), cluster(1, " ", 4.0), cluster(2, "b", EM)];
    let roles = vec![FontRole::LatinText, FontRole::LatinText, FontRole::LatinText];
    let spacing = vec![
        edges(EastAsianSpacingValue::Narrow, EastAsianSpacingValue::Narrow, false),
        edges(EastAsianSpacingValue::Other, EastAsianSpacingValue::Other, false),
        edges(EastAsianSpacingValue::Narrow, EastAsianSpacingValue::Narrow, false),
    ];
    let plan = justify(&clusters, &roles, &spacing, IntRange::new(0, 2), 60.0, |request| {
        request.no_stretch_boundary_clusters = HashSet::from([0]);
    });

    assert!(plan.allocations.iter().all(|allocation| allocation.target_cluster_index != 1));
}

#[test]
fn virtual_sino_western_gap_when_allow_sino_western_gap_stretch_is_false() {
    let clusters = vec![
        cluster(0, "中", EM),
        cluster(1, "[", EM),
        cluster(2, "1", EM),
        cluster(3, "]", EM),
        cluster(4, "a", EM),
    ];
    let roles = vec![FontRole::CjkText, FontRole::CjkText, FontRole::CjkText, FontRole::CjkText, FontRole::LatinText];
    let spacing = vec![
        edges(EastAsianSpacingValue::Wide, EastAsianSpacingValue::Wide, true),
        edges(EastAsianSpacingValue::Other, EastAsianSpacingValue::Other, false),
        edges(EastAsianSpacingValue::Other, EastAsianSpacingValue::Other, false),
        edges(EastAsianSpacingValue::Other, EastAsianSpacingValue::Other, false),
        edges(EastAsianSpacingValue::Narrow, EastAsianSpacingValue::Narrow, false),
    ];
    let plan = justify(&clusters, &roles, &spacing, IntRange::new(0, 4), 100.0, |request| {
        request.allow_sino_western_gap_stretch = false;
        request.attached_inline_virtual_boundary_after_clusters = HashMap::from([(3, 0)]);
        request.attached_inline_virtual_sino_western_boundary_after_clusters = HashSet::from([3]);
    });

    assert!(plan.allocations.iter().all(|allocation| allocation.target_cluster_index != 3));
}

#[test]
fn single_cluster_range_produces_no_opportunities() {
    let clusters = vec![cluster(0, "中", EM)];
    let roles = vec![FontRole::CjkText];
    let spacing = vec![edges(EastAsianSpacingValue::Wide, EastAsianSpacingValue::Wide, true)];
    let plan = justify(&clusters, &roles, &spacing, IntRange::new(0, 0), 30.0, |_| {});

    assert!(plan.allocations.is_empty());
}

#[test]
fn zero_cjk_latin_headroom_produces_no_opportunities() {
    let clusters = vec![cluster(0, "中", EM), cluster(1, "a", EM)];
    let roles = vec![FontRole::CjkText, FontRole::LatinText];
    let spacing = vec![
        edges(EastAsianSpacingValue::Wide, EastAsianSpacingValue::Wide, true),
        edges(EastAsianSpacingValue::Narrow, EastAsianSpacingValue::Narrow, false),
    ];
    let plan = justify(&clusters, &roles, &spacing, IntRange::new(0, 1), 40.0, |request| {
        request.cjk_latin_space_base_em = 0.5;
        request.cjk_latin_space_max_em = 0.5;
    });

    assert!(plan.allocations.iter().all(|allocation| allocation.kind == GlueKind::CjkInterChar));
}

#[test]
fn typed_space_and_word_space_predicate_edge_conditions() {
    let clusters = vec![
        cluster(0, "", EM),
        cluster(1, " ", 4.0),
        cluster(2, " ", 4.0),
        cluster(3, "abc", EM),
        cluster(4, "xyz", EM),
        cluster(5, "字", EM),
    ];
    let roles = vec![
        FontRole::LatinText,
        FontRole::LatinText,
        FontRole::LatinText,
        FontRole::LatinText,
        FontRole::LatinText,
        FontRole::CjkText,
    ];
    let spacing = vec![
        edges(EastAsianSpacingValue::Other, EastAsianSpacingValue::Other, false),
        edges(EastAsianSpacingValue::Other, EastAsianSpacingValue::Other, false),
        edges(EastAsianSpacingValue::Other, EastAsianSpacingValue::Other, false),
        edges(EastAsianSpacingValue::Narrow, EastAsianSpacingValue::Narrow, false),
        edges(EastAsianSpacingValue::Narrow, EastAsianSpacingValue::Narrow, false),
        edges(EastAsianSpacingValue::Wide, EastAsianSpacingValue::Wide, true),
    ];
    let plan = justify(&clusters, &roles, &spacing, IntRange::new(0, 5), 150.0, |_| {});

    assert!(!plan.allocations.is_empty());
}

#[test]
fn compression_with_zero_surplus_and_zero_capacity() {
    let justifier = Justifier::default();
    let empty_plan = justifier.compress(0.0, &[]);
    assert_eq!(0.0, empty_plan.surplus_before);
    assert_eq!(0.0, empty_plan.unfilled_surplus);
    assert!(empty_plan.allocations.is_empty());

    let zero_capacity = justifier.compress(
        10.0,
        &[ShrinkOpportunity::new(0, 1, 0.0, ShrinkChannel::TrailingGlue)],
    );
    assert_eq!(10.0, zero_capacity.surplus_before);
    assert_eq!(10.0, zero_capacity.unfilled_surplus);
    assert!(zero_capacity.allocations.is_empty());
}

#[test]
fn virtual_non_sino_western_boundary_when_allow_sino_western_gap_stretch_is_false() {
    let clusters = vec![
        cluster(0, "中", EM),
        cluster(1, "[", EM),
        cluster(2, "1", EM),
        cluster(3, "]", EM),
        cluster(4, "文", EM),
    ];
    let roles = vec![FontRole::CjkText; 5];
    let spacing = vec![
        edges(EastAsianSpacingValue::Wide, EastAsianSpacingValue::Wide, true),
        edges(EastAsianSpacingValue::Other, EastAsianSpacingValue::Other, false),
        edges(EastAsianSpacingValue::Other, EastAsianSpacingValue::Other, false),
        edges(EastAsianSpacingValue::Other, EastAsianSpacingValue::Other, false),
        edges(EastAsianSpacingValue::Wide, EastAsianSpacingValue::Wide, true),
    ];
    let plan = justify(&clusters, &roles, &spacing, IntRange::new(0, 4), 100.0, |request| {
        request.allow_sino_western_gap_stretch = false;
        request.attached_inline_virtual_boundary_after_clusters = HashMap::from([(3, 0)]);
    });

    assert!(plan.allocations.iter().any(|allocation| {
        allocation.target_cluster_index == 3 && allocation.reason == "AttachedInlineVirtualInterChar"
    }));
}

#[test]
fn empty_line_cluster_range_skips_uniform_space_loop() {
    let clusters = vec![cluster(0, "中", EM), cluster(1, "文", EM)];
    let roles = vec![FontRole::CjkText, FontRole::CjkText];
    let spacing = vec![edges(EastAsianSpacingValue::Wide, EastAsianSpacingValue::Wide, true); 2];
    let plan = justify(&clusters, &roles, &spacing, IntRange::new(1, 0), 50.0, |_| {});

    assert!(plan.allocations.is_empty());
    assert_eq!(50.0, plan.unfilled_deficit);
}

#[test]
fn compress_subnormal_underflow_shrink_zero() {
    let plan = Justifier::default().compress(
        0.0,
        &[ShrinkOpportunity::new(0, 1, f32::INFINITY, ShrinkChannel::TrailingGlue)],
    );

    assert!(plan.allocations.is_empty());
    assert_eq!(0.0, plan.surplus_before);
}

#[test]
fn attached_inline_virtual_sino_western_zero_headroom_in_allocate() {
    let clusters = vec![cluster(0, "中", EM), cluster(1, "文", EM), cluster(2, "a", EM)];
    let roles = vec![FontRole::CjkText, FontRole::CjkText, FontRole::LatinText];
    let spacing = vec![
        edges(EastAsianSpacingValue::Wide, EastAsianSpacingValue::Wide, true),
        edges(EastAsianSpacingValue::Wide, EastAsianSpacingValue::Wide, true),
        edges(EastAsianSpacingValue::Narrow, EastAsianSpacingValue::Narrow, false),
    ];
    let plan = justify(&clusters, &roles, &spacing, IntRange::new(0, 2), 60.0, |request| {
        request.cjk_latin_space_base_em = 0.5;
        request.cjk_latin_space_max_em = 0.5;
        request.attached_inline_virtual_boundary_after_clusters = HashMap::from([(1, 0)]);
        request.attached_inline_virtual_sino_western_boundary_after_clusters = HashSet::from([1]);
    });

    assert!(plan.allocations.iter().all(|allocation| allocation.kind != GlueKind::CjkLatinSpace));
    assert!(plan.allocations.iter().any(|allocation| allocation.kind == GlueKind::CjkInterChar));
}

#[test]
fn cjk_latin_mixed_zero_and_positive_capacity_allocation() {
    let clusters = vec![
        cluster(0, "中", EM),
        cluster(1, " ", 2.0),
        cluster(2, "a", EM),
        cluster(3, "b", EM),
    ];
    let roles = vec![FontRole::CjkText, FontRole::LatinText, FontRole::LatinText, FontRole::LatinText];
    let spacing = vec![
        edges(EastAsianSpacingValue::Wide, EastAsianSpacingValue::Wide, true),
        edges(EastAsianSpacingValue::Other, EastAsianSpacingValue::Other, false),
        edges(EastAsianSpacingValue::Narrow, EastAsianSpacingValue::Narrow, false),
        edges(EastAsianSpacingValue::Narrow, EastAsianSpacingValue::Narrow, false),
    ];
    let configure = |request: &mut JustificationRequest<'_>| {
        request.cjk_latin_space_base_em = 0.5;
        request.cjk_latin_space_max_em = 0.5;
        request.attached_inline_virtual_boundary_after_clusters = HashMap::from([(2, 0)]);
        request.attached_inline_virtual_sino_western_boundary_after_clusters = HashSet::from([2]);
    };
    let exact = justify(&clusters, &roles, &spacing, IntRange::new(0, 3), 54.0, configure);
    let exact_allocations: Vec<_> = exact
        .allocations
        .iter()
        .filter(|allocation| allocation.kind == GlueKind::CjkLatinSpace)
        .collect();
    assert_eq!(1, exact_allocations.len());
    assert_eq!(1, exact_allocations[0].target_cluster_index);
    assert_eq!(4.0, exact_allocations[0].delta);

    let under = justify(&clusters, &roles, &spacing, IntRange::new(0, 3), 60.0, |request| {
        request.cjk_latin_space_base_em = 0.5;
        request.cjk_latin_space_max_em = 0.5;
        request.attached_inline_virtual_boundary_after_clusters = HashMap::from([(2, 0)]);
        request.attached_inline_virtual_sino_western_boundary_after_clusters = HashSet::from([2]);
    });
    let under_allocations: Vec<_> = under
        .allocations
        .iter()
        .filter(|allocation| allocation.kind == GlueKind::CjkLatinSpace)
        .collect();
    assert_eq!(1, under_allocations.len());
    assert_eq!(1, under_allocations[0].target_cluster_index);
    assert_eq!(6.0, under_allocations[0].delta);
}
