use tiqian::common::{HashMap, HashSet};
use tiqian::core::east_asian_spacing::{EastAsianSpacingEdges, EastAsianSpacingValue};
use tiqian::core::geometry::TextRange;
use tiqian::core::int_range::IntRange;
use tiqian::core::layout_model::Cluster;
use tiqian::core::text::Text;
use tiqian::core::text_model::{InlineObjectPreferredStretch, InlineObjectPreferredStretchKind};
use tiqian::font::font_policy::FontRole;
use tiqian::layout::justifier::{JustificationPlan, JustificationRequest, Justifier};
use tiqian::layout::line_optimization::PushInAllocation;
use tiqian::layout::progressive_break_decisions::{
    ProgressiveBreakTier, ShrinkChannel, ShrinkOpportunity,
};
use tiqian::layout::punctuation_model::GlueKind;

const EM: f32 = 16.0;

struct JustificationRequestConfig {
    skip: bool,
    skip_reason: Option<String>,
    allow_sino_western_gap_stretch: bool,
    cjk_latin_space_base_em: f32,
    cjk_latin_space_max_em: f32,
    no_stretch_boundary_clusters: HashSet<i32>,
    no_stretch_boundary_after_clusters: HashSet<i32>,
    western_bracket_cjk_inter_char_boundary_after_clusters: HashSet<i32>,
    attached_inline_physical_boundary_after_clusters: HashSet<i32>,
    attached_inline_virtual_boundary_after_clusters: HashMap<i32, i32>,
    attached_inline_virtual_sino_western_boundary_after_clusters: HashSet<i32>,
    uniform_inline_object_boundary_after_clusters: HashSet<i32>,
    preferred_inline_object_boundary_after_clusters: HashMap<i32, InlineObjectPreferredStretch>,
    technical_boundary_after_clusters: HashMap<i32, ProgressiveBreakTier>,
    emergency_tracking_boundary_after_clusters: HashMap<i32, String>,
    preferred_emergency_tracking_boundary_after_clusters: HashMap<i32, String>,
}

impl Default for JustificationRequestConfig {
    fn default() -> Self {
        Self {
            skip: false,
            skip_reason: None,
            allow_sino_western_gap_stretch: true,
            cjk_latin_space_base_em: 0.25,
            cjk_latin_space_max_em: 0.5,
            no_stretch_boundary_clusters: HashSet::new(),
            no_stretch_boundary_after_clusters: HashSet::new(),
            western_bracket_cjk_inter_char_boundary_after_clusters: HashSet::new(),
            attached_inline_physical_boundary_after_clusters: HashSet::new(),
            attached_inline_virtual_boundary_after_clusters: HashMap::new(),
            attached_inline_virtual_sino_western_boundary_after_clusters: HashSet::new(),
            uniform_inline_object_boundary_after_clusters: HashSet::new(),
            preferred_inline_object_boundary_after_clusters: HashMap::new(),
            technical_boundary_after_clusters: HashMap::new(),
            emergency_tracking_boundary_after_clusters: HashMap::new(),
            preferred_emergency_tracking_boundary_after_clusters: HashMap::new(),
        }
    }
}

fn c(text: &str, index: i32, advance: f32, font_key: &str) -> Cluster {
    Cluster::new(
        TextRange::new(index, index + text.encode_utf16().count() as i32),
        Text::from(text),
        font_key.to_owned(),
        advance,
    )
}

fn e(
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
    edges: &[EastAsianSpacingEdges],
    range: IntRange,
    max_width: f32,
    justifier: &Justifier,
    configure: impl FnOnce(&mut JustificationRequestConfig),
) -> JustificationPlan {
    let mut config = JustificationRequestConfig::default();
    configure(&mut config);
    justifier.justify(JustificationRequest {
        adjusted_clusters: clusters,
        cluster_roles: roles,
        east_asian_spacing_edges: edges,
        line_cluster_range: range,
        max_width,
        font_size: EM,
        skip: config.skip,
        skip_reason: config.skip_reason,
        allow_sino_western_gap_stretch: config.allow_sino_western_gap_stretch,
        cjk_latin_space_base_em: config.cjk_latin_space_base_em,
        cjk_latin_space_max_em: config.cjk_latin_space_max_em,
        no_stretch_boundary_clusters: &config.no_stretch_boundary_clusters,
        no_stretch_boundary_after_clusters: &config.no_stretch_boundary_after_clusters,
        western_bracket_cjk_inter_char_boundary_after_clusters: &config
            .western_bracket_cjk_inter_char_boundary_after_clusters,
        attached_inline_physical_boundary_after_clusters: &config
            .attached_inline_physical_boundary_after_clusters,
        attached_inline_virtual_boundary_after_clusters: &config
            .attached_inline_virtual_boundary_after_clusters,
        attached_inline_virtual_sino_western_boundary_after_clusters: &config
            .attached_inline_virtual_sino_western_boundary_after_clusters,
        uniform_inline_object_boundary_after_clusters: &config
            .uniform_inline_object_boundary_after_clusters,
        preferred_inline_object_boundary_after_clusters: &config
            .preferred_inline_object_boundary_after_clusters,
        technical_boundary_after_clusters: &config.technical_boundary_after_clusters,
        emergency_tracking_boundary_after_clusters: &config
            .emergency_tracking_boundary_after_clusters,
        preferred_emergency_tracking_boundary_after_clusters: &config
            .preferred_emergency_tracking_boundary_after_clusters,
    })
}

fn latin_space_latin(space_advance: f32, a_advance: f32, b_advance: f32) -> (Vec<Cluster>, Vec<FontRole>, Vec<EastAsianSpacingEdges>) {
    (
        vec![c("a", 0, a_advance, "lat"), c(" ", 1, space_advance, "lat"), c("b", 2, b_advance, "lat")],
        vec![FontRole::LatinText, FontRole::LatinText, FontRole::LatinText],
        vec![
            e(EastAsianSpacingValue::Narrow, EastAsianSpacingValue::Narrow, false),
            e(EastAsianSpacingValue::Narrow, EastAsianSpacingValue::Narrow, false),
            e(EastAsianSpacingValue::Narrow, EastAsianSpacingValue::Narrow, false),
        ],
    )
}

fn cjk_cjk() -> (Vec<Cluster>, Vec<FontRole>, Vec<EastAsianSpacingEdges>) {
    (
        vec![c("中", 0, EM, "k"), c("中", 1, EM, "k")],
        vec![FontRole::CjkText, FontRole::CjkText],
        vec![
            e(EastAsianSpacingValue::Wide, EastAsianSpacingValue::Wide, true),
            e(EastAsianSpacingValue::Wide, EastAsianSpacingValue::Wide, true),
        ],
    )
}

fn cjk_latin() -> (Vec<Cluster>, Vec<FontRole>, Vec<EastAsianSpacingEdges>) {
    (
        vec![c("中", 0, EM, "k"), c("a", 1, EM, "lat")],
        vec![FontRole::CjkText, FontRole::LatinText],
        vec![
            e(EastAsianSpacingValue::Wide, EastAsianSpacingValue::Wide, true),
            e(EastAsianSpacingValue::Narrow, EastAsianSpacingValue::Narrow, false),
        ],
    )
}

#[test]
fn misaligned_role_and_spacing_lists_are_rejected() {
    let (clusters, roles, edges) = cjk_cjk();
    let roles_with_extra = [roles.clone(), vec![FontRole::LatinText]].concat();
    assert!(std::panic::catch_unwind(|| {
        justify(&clusters, &roles_with_extra, &edges, IntRange::new(0, 1), 64.0, &Justifier::default(), |_| {});
    })
    .is_err());
    let edges_with_extra = [edges.clone(), vec![e(EastAsianSpacingValue::Other, EastAsianSpacingValue::Other, false)]].concat();
    assert!(std::panic::catch_unwind(|| {
        justify(&clusters, &roles, &edges_with_extra, IntRange::new(0, 1), 64.0, &Justifier::default(), |_| {});
    })
    .is_err());
}

#[test]
fn skip_keeps_the_deficit_and_records_the_reason() {
    let (clusters, roles, edges) = cjk_cjk();
    let plan = justify(&clusters, &roles, &edges, IntRange::new(0, 1), 64.0, &Justifier::default(), |request| {
        request.skip = true;
        request.skip_reason = Some("RaggedRight".to_owned());
    });

    assert_eq!(32.0, plan.deficit_before);
    assert_eq!(32.0, plan.unfilled_deficit);
    assert!(plan.allocations.is_empty());
    assert_eq!(Some("RaggedRight".to_owned()), plan.fallback_reason);
}

#[test]
fn zero_deficit_returns_an_empty_plan_without_reason() {
    let (clusters, roles, edges) = cjk_cjk();
    let plan = justify(&clusters, &roles, &edges, IntRange::new(0, 1), 32.0, &Justifier::default(), |_| {});

    assert_eq!(0.0, plan.deficit_before);
    assert_eq!(0.0, plan.unfilled_deficit);
    assert!(plan.allocations.is_empty());
    assert_eq!(None, plan.fallback_reason);
}

#[test]
fn technical_whitespace_stretch_fills_and_stops_the_tier_chain() {
    let (clusters, roles, edges) = latin_space_latin(2.0, EM, EM);
    let justifier = Justifier::new(0.5, 0.25);
    let plan = justify(&clusters, &roles, &edges, IntRange::new(0, 2), 38.0, &justifier, |request| {
        request.technical_boundary_after_clusters = HashMap::from([(1, ProgressiveBreakTier::Whitespace)]);
    });

    let [allocation] = plan.allocations.as_slice() else {
        panic!("expected exactly one allocation: {:?}", plan.allocations);
    };
    assert_eq!(1, allocation.target_cluster_index);
    assert_eq!(GlueKind::ProgressiveTechnical, allocation.kind);
    assert_eq!("ProgressiveTechnicalWhitespaceStretch", allocation.reason);
    assert_eq!(4.0, allocation.delta);
    assert_eq!(0.0, plan.unfilled_deficit);
}

#[test]
fn technical_whitespace_requires_the_whitespace_tier_and_a_source_space() {
    let (clusters, roles, edges) = latin_space_latin(4.0, EM, EM);
    let wrong_tier = justify(&clusters, &roles, &edges, IntRange::new(0, 2), 40.0, &Justifier::default(), |request| {
        request.technical_boundary_after_clusters = HashMap::from([(1, ProgressiveBreakTier::Structural)]);
    });
    assert_eq!(
        GlueKind::WordSpace,
        wrong_tier
            .allocations
            .as_slice()
            .first()
            .expect("expected exactly one wrong-tier allocation")
            .kind
    );
    assert_eq!(1, wrong_tier.allocations.len());

    let wrong_cluster = justify(&clusters, &roles, &edges, IntRange::new(0, 2), 40.0, &Justifier::default(), |request| {
        request.technical_boundary_after_clusters = HashMap::from([(0, ProgressiveBreakTier::Whitespace)]);
    });
    assert_eq!(
        GlueKind::WordSpace,
        wrong_cluster
            .allocations
            .as_slice()
            .first()
            .expect("expected exactly one wrong-cluster allocation")
            .kind
    );
    assert_eq!(1, wrong_cluster.allocations.len());
}

#[test]
fn zero_technical_stretch_capacity_produces_no_opportunity() {
    let (clusters, roles, edges) = latin_space_latin(4.0, EM, EM);
    let justifier = Justifier::new(0.5, 0.0);
    let plan = justify(&clusters, &roles, &edges, IntRange::new(0, 2), 40.0, &justifier, |request| {
        request.technical_boundary_after_clusters = HashMap::from([(1, ProgressiveBreakTier::Whitespace)]);
    });

    assert_eq!(GlueKind::WordSpace, plan.allocations.first().expect("expected exactly one allocation").kind);
    assert_eq!(1, plan.allocations.len());
}

#[test]
fn word_space_stretches_within_its_cap() {
    let (clusters, roles, edges) = latin_space_latin(4.0, EM, EM);
    let plan = justify(&clusters, &roles, &edges, IntRange::new(0, 2), 38.0, &Justifier::default(), |_| {});

    let [allocation] = plan.allocations.as_slice() else {
        panic!("expected exactly one allocation: {:?}", plan.allocations);
    };
    assert_eq!(GlueKind::WordSpace, allocation.kind);
    assert_eq!(1, allocation.target_cluster_index);
    assert_eq!(2.0, allocation.delta);
    assert_eq!("WordSpace", allocation.reason);
}

#[test]
fn word_space_at_the_cap_or_collapsed_is_skipped() {
    let at_cap = latin_space_latin(8.0, EM, EM);
    let at_cap_plan = justify(&at_cap.0, &at_cap.1, &at_cap.2, IntRange::new(0, 2), 48.0, &Justifier::default(), |_| {});
    assert!(at_cap_plan.allocations.is_empty());
    assert_eq!(Some("WesternDominantLineNaturalSpacing".to_owned()), at_cap_plan.fallback_reason);

    let collapsed = latin_space_latin(0.0, EM, EM);
    let collapsed_plan = justify(&collapsed.0, &collapsed.1, &collapsed.2, IntRange::new(0, 2), 40.0, &Justifier::default(), |_| {});
    assert!(collapsed_plan.allocations.is_empty());
}

#[test]
fn space_gap_protection_covers_all_four_disjuncts() {
    let base = latin_space_latin(4.0, EM, EM);
    let mut clusters = base.0;
    clusters.push(c("中", 3, EM, "k"));
    clusters.push(c("x", 4, EM, "lat"));
    let mut roles = base.1;
    roles.extend([FontRole::CjkText, FontRole::LatinText]);
    let mut edges = base.2;
    edges.extend([
        e(EastAsianSpacingValue::Wide, EastAsianSpacingValue::Wide, true),
        e(EastAsianSpacingValue::Narrow, EastAsianSpacingValue::Narrow, false),
    ]);

    for (after, protected_clusters) in [
        (HashSet::from([0]), HashSet::new()),
        (HashSet::from([1]), HashSet::new()),
        (HashSet::new(), HashSet::from([0])),
        (HashSet::new(), HashSet::from([2])),
    ] {
        let plan = justify(&clusters, &roles, &edges, IntRange::new(0, 4), 72.0, &Justifier::default(), |request| {
            request.no_stretch_boundary_after_clusters = after;
            request.no_stretch_boundary_clusters = protected_clusters;
        });
        assert!(plan.allocations.iter().all(|allocation| allocation.kind != GlueKind::WordSpace));
        assert_eq!(0.0, plan.unfilled_deficit);
    }
}

#[test]
fn virtual_sino_western_gap_skips_protected_and_typed_edges() {
    let typed_left_clusters = vec![c("中", 0, EM, "k"), c(" ", 1, 4.0, "lat"), c("a", 2, EM, "lat")];
    let typed_left_roles = vec![FontRole::CjkText, FontRole::LatinText, FontRole::LatinText];
    let typed_left_edges = vec![
        e(EastAsianSpacingValue::Wide, EastAsianSpacingValue::Wide, true),
        e(EastAsianSpacingValue::Other, EastAsianSpacingValue::Wide, false),
        e(EastAsianSpacingValue::Narrow, EastAsianSpacingValue::Narrow, false),
    ];
    let typed_left = justify(&typed_left_clusters, &typed_left_roles, &typed_left_edges, IntRange::new(0, 2), 40.0, &Justifier::default(), |_| {});
    let [typed_left_allocation] = typed_left.allocations.as_slice() else {
        panic!("expected exactly one typed-left allocation: {:?}", typed_left.allocations);
    };
    assert_eq!(GlueKind::CjkLatinSpace, typed_left_allocation.kind);
    assert_eq!(1, typed_left_allocation.target_cluster_index);

    let typed_right_clusters = vec![c("中", 0, EM, "k"), c(" a", 1, EM, "lat"), c("b", 2, EM, "lat")];
    let typed_right_roles = vec![FontRole::CjkText, FontRole::LatinText, FontRole::LatinText];
    let typed_right_edges = vec![
        e(EastAsianSpacingValue::Wide, EastAsianSpacingValue::Wide, true),
        e(EastAsianSpacingValue::Narrow, EastAsianSpacingValue::Other, false),
        e(EastAsianSpacingValue::Narrow, EastAsianSpacingValue::Narrow, false),
    ];
    let typed_right = justify(&typed_right_clusters, &typed_right_roles, &typed_right_edges, IntRange::new(0, 2), 52.0, &Justifier::default(), |_| {});
    assert!(typed_right.allocations.iter().all(|allocation| allocation.kind != GlueKind::CjkLatinSpace));

    let (clusters, roles, edges) = cjk_latin();
    let physical = justify(&clusters, &roles, &edges, IntRange::new(0, 1), 36.0, &Justifier::default(), |request| {
        request.attached_inline_physical_boundary_after_clusters = HashSet::from([0]);
    });
    assert!(physical.allocations.iter().all(|allocation| allocation.kind != GlueKind::CjkLatinSpace));
    let closed = justify(&clusters, &roles, &edges, IntRange::new(0, 1), 36.0, &Justifier::default(), |request| {
        request.no_stretch_boundary_after_clusters = HashSet::from([0]);
    });
    assert!(closed.allocations.iter().all(|allocation| allocation.kind != GlueKind::CjkLatinSpace));
    assert!(closed.unfilled_deficit > 0.0);
}

#[test]
fn attached_inline_virtual_auto_space_joins_tier_two() {
    let clusters = vec![c("中", 0, EM, "k"), c("", 1, 0.0, "obj"), c("a", 2, EM, "lat"), c("b", 3, EM, "lat")];
    let roles = vec![FontRole::CjkText, FontRole::Unknown, FontRole::LatinText, FontRole::LatinText];
    let edges = vec![
        e(EastAsianSpacingValue::Wide, EastAsianSpacingValue::Wide, true),
        e(EastAsianSpacingValue::Other, EastAsianSpacingValue::Other, false),
        e(EastAsianSpacingValue::Narrow, EastAsianSpacingValue::Narrow, false),
        e(EastAsianSpacingValue::Narrow, EastAsianSpacingValue::Narrow, false),
    ];
    let happy = justify(&clusters, &roles, &edges, IntRange::new(0, 3), 52.0, &Justifier::default(), |request| {
        request.attached_inline_virtual_boundary_after_clusters = HashMap::from([(2, 0)]);
        request.attached_inline_virtual_sino_western_boundary_after_clusters = HashSet::from([2]);
    });
    let [allocation] = happy.allocations.as_slice() else {
        panic!("expected exactly one allocation: {:?}", happy.allocations);
    };
    assert_eq!(GlueKind::CjkLatinSpace, allocation.kind);
    assert_eq!("AttachedInlineVirtualAutoSpace", allocation.reason);
    assert_eq!(2, allocation.target_cluster_index);
    assert_eq!(4.0, allocation.delta);

    let no_previous = justify(&clusters, &roles, &edges, IntRange::new(0, 3), 52.0, &Justifier::default(), |request| {
        request.attached_inline_virtual_sino_western_boundary_after_clusters = HashSet::from([2]);
    });
    assert!(no_previous.allocations.iter().all(|allocation| allocation.reason != "AttachedInlineVirtualAutoSpace"));

    let target_out_of_range = justify(&clusters, &roles, &edges, IntRange::new(0, 2), 36.0, &Justifier::default(), |request| {
        request.attached_inline_virtual_boundary_after_clusters = HashMap::from([(3, 0)]);
        request.attached_inline_virtual_sino_western_boundary_after_clusters = HashSet::from([3]);
    });
    assert!(target_out_of_range.allocations.iter().all(|allocation| allocation.reason != "AttachedInlineVirtualAutoSpace"));

    let next_out_of_range = justify(&clusters, &roles, &edges, IntRange::new(0, 2), 36.0, &Justifier::default(), |request| {
        request.attached_inline_virtual_boundary_after_clusters = HashMap::from([(2, 0)]);
        request.attached_inline_virtual_sino_western_boundary_after_clusters = HashSet::from([2]);
    });
    assert!(next_out_of_range.allocations.iter().all(|allocation| allocation.reason != "AttachedInlineVirtualAutoSpace"));

    for protected in [HashSet::from([0]), HashSet::from([3])] {
        let plan = justify(&clusters, &roles, &edges, IntRange::new(0, 3), 52.0, &Justifier::default(), |request| {
            request.attached_inline_virtual_boundary_after_clusters = HashMap::from([(2, 0)]);
            request.attached_inline_virtual_sino_western_boundary_after_clusters = HashSet::from([2]);
            request.no_stretch_boundary_clusters = protected;
        });
        assert!(plan.allocations.iter().all(|allocation| allocation.reason != "AttachedInlineVirtualAutoSpace"));
    }
}

#[test]
fn typed_sino_western_space_stretches_from_its_base() {
    let clusters = vec![c("中", 0, EM, "k"), c(" ", 1, 2.0, "k"), c("b", 2, EM, "lat")];
    let roles = vec![FontRole::CjkText, FontRole::LatinText, FontRole::LatinText];
    let edges = vec![
        e(EastAsianSpacingValue::Wide, EastAsianSpacingValue::Wide, true),
        e(EastAsianSpacingValue::Narrow, EastAsianSpacingValue::Narrow, false),
        e(EastAsianSpacingValue::Narrow, EastAsianSpacingValue::Narrow, false),
    ];
    let plan = justify(&clusters, &roles, &edges, IntRange::new(0, 2), 38.0, &Justifier::default(), |_| {});
    let [allocation] = plan.allocations.as_slice() else {
        panic!("expected exactly one allocation: {:?}", plan.allocations);
    };
    assert_eq!(GlueKind::CjkLatinSpace, allocation.kind);
    assert_eq!(1, allocation.target_cluster_index);
    assert_eq!(4.0, allocation.delta);
    assert_eq!(0.0, plan.unfilled_deficit);

    let at_cap = vec![c("中", 0, EM, "k"), c(" ", 1, 8.0, "k"), c("b", 2, EM, "lat")];
    let at_cap_plan = justify(&at_cap, &roles, &edges, IntRange::new(0, 2), 44.0, &Justifier::default(), |_| {});
    assert_eq!(0, at_cap_plan.allocations.iter().filter(|allocation| allocation.kind == GlueKind::CjkLatinSpace).count());
    assert_eq!(2, at_cap_plan.allocations.iter().filter(|allocation| allocation.kind == GlueKind::CjkInterChar).count());
    assert_eq!(0.0, at_cap_plan.unfilled_deficit);

    let collapsed = vec![c("中", 0, EM, "k"), c(" ", 1, 0.0, "k"), c("b", 2, EM, "lat")];
    let collapsed_plan = justify(&collapsed, &roles, &edges, IntRange::new(0, 2), 36.0, &Justifier::default(), |request| {
        request.cjk_latin_space_base_em = 0.25;
        request.cjk_latin_space_max_em = 0.25;
    });
    assert!(collapsed_plan.allocations.iter().all(|allocation| allocation.target_cluster_index != 1 || allocation.delta <= 0.0));
}

#[test]
fn typed_sino_western_space_needs_both_edges_to_pair() {
    let clusters = vec![c("中", 0, EM, "k"), c(" ", 1, 4.0, "k"), c("中", 2, EM, "k")];
    let roles = vec![FontRole::CjkText, FontRole::LatinText, FontRole::CjkText];
    let edges = vec![
        e(EastAsianSpacingValue::Wide, EastAsianSpacingValue::Wide, true),
        e(EastAsianSpacingValue::Narrow, EastAsianSpacingValue::Narrow, false),
        e(EastAsianSpacingValue::Wide, EastAsianSpacingValue::Wide, true),
    ];
    let plan = justify(&clusters, &roles, &edges, IntRange::new(0, 2), 40.0, &Justifier::default(), |_| {});

    assert!(plan.allocations.iter().all(|allocation| !matches!(allocation.kind, GlueKind::WordSpace | GlueKind::CjkLatinSpace)));
    assert_eq!(0.0, plan.unfilled_deficit);
}

#[test]
fn zero_capacity_sino_western_tier_defers_everything_downward() {
    let (clusters, roles, edges) = cjk_latin();
    let plan = justify(&clusters, &roles, &edges, IntRange::new(0, 1), 36.0, &Justifier::default(), |request| {
        request.cjk_latin_space_max_em = 0.25;
    });

    assert_eq!(0.0, plan.unfilled_deficit);
    let [allocation] = plan.allocations.as_slice() else {
        panic!("expected exactly one allocation: {:?}", plan.allocations);
    };
    assert_eq!(GlueKind::CjkInterChar, allocation.kind);
    assert_eq!(4.0, allocation.delta);
}

#[test]
fn mixed_capacity_sino_western_opps_skip_zero_capacity_in_overflow() {
    let clusters = vec![c("中", 0, EM, "k"), c(" ", 1, 2.0, "k"), c("a", 2, EM, "lat")];
    let roles = vec![FontRole::CjkText, FontRole::LatinText, FontRole::LatinText];
    let edges = vec![
        e(EastAsianSpacingValue::Wide, EastAsianSpacingValue::Wide, true),
        e(EastAsianSpacingValue::Narrow, EastAsianSpacingValue::Narrow, false),
        e(EastAsianSpacingValue::Narrow, EastAsianSpacingValue::Narrow, false),
    ];
    let plan = justify(&clusters, &roles, &edges, IntRange::new(0, 2), 40.0, &Justifier::default(), |request| {
        request.cjk_latin_space_max_em = 0.25;
    });
    let tier_two: Vec<_> = plan.allocations.iter().filter(|allocation| allocation.kind == GlueKind::CjkLatinSpace).collect();
    assert_eq!(vec![1], tier_two.iter().map(|allocation| allocation.target_cluster_index).collect::<Vec<_>>());
    assert_eq!(2.0, tier_two[0].delta);
    assert_eq!(0.0, plan.unfilled_deficit);
}

#[test]
fn sino_western_stretch_disabled_skips_tier_two_and_its_virtual_tracking() {
    let (clusters, roles, edges) = cjk_latin();
    let plan = justify(&clusters, &roles, &edges, IntRange::new(0, 1), 36.0, &Justifier::default(), |request| {
        request.allow_sino_western_gap_stretch = false;
    });

    assert!(plan.allocations.is_empty());
    assert_eq!(4.0, plan.unfilled_deficit);
}

#[test]
fn preferred_inline_object_stretch_runs_by_semantic_kind() {
    let clusters = vec![c("中", 0, EM, "k"), c("", 1, 0.0, "obj"), c("中", 2, EM, "k")];
    let roles = vec![FontRole::CjkText, FontRole::Unknown, FontRole::CjkText];
    let edges = vec![
        e(EastAsianSpacingValue::Wide, EastAsianSpacingValue::Wide, true),
        e(EastAsianSpacingValue::Other, EastAsianSpacingValue::Other, false),
        e(EastAsianSpacingValue::Wide, EastAsianSpacingValue::Wide, true),
    ];
    for (kind, reason, glue) in [
        (InlineObjectPreferredStretchKind::PunctuationTrailing, "InlineObjectPunctuationTrailing", GlueKind::InlineObjectPunctuationTrailing),
        (InlineObjectPreferredStretchKind::Relation, "InlineObjectRelation", GlueKind::InlineObjectRelation),
        (InlineObjectPreferredStretchKind::BinaryOperator, "InlineObjectBinaryOperator", GlueKind::InlineObjectBinaryOperator),
    ] {
        let plan = justify(&clusters, &roles, &edges, IntRange::new(0, 2), 36.0, &Justifier::default(), |request| {
            request.preferred_inline_object_boundary_after_clusters = HashMap::from([(1, InlineObjectPreferredStretch::new(kind, 4.0, 8.0))]);
        });
        let [allocation] = plan.allocations.as_slice() else {
            panic!("expected exactly one allocation: {:?}", plan.allocations);
        };
        assert_eq!(glue, allocation.kind);
        assert_eq!(reason, allocation.reason);
        assert_eq!(4.0, allocation.delta);
        assert_eq!(2, allocation.priority);
    }

    let at_end = justify(&clusters, &roles, &edges, IntRange::new(0, 1), 20.0, &Justifier::default(), |request| {
        request.preferred_inline_object_boundary_after_clusters = HashMap::from([(1, InlineObjectPreferredStretch::new(InlineObjectPreferredStretchKind::Relation, 4.0, 8.0))]);
    });
    assert!(at_end.allocations.is_empty());
    assert_eq!(4.0, at_end.unfilled_deficit);

    let closed = justify(&clusters, &roles, &edges, IntRange::new(0, 2), 36.0, &Justifier::default(), |request| {
        request.preferred_inline_object_boundary_after_clusters = HashMap::from([(1, InlineObjectPreferredStretch::new(InlineObjectPreferredStretchKind::Relation, 4.0, 8.0))]);
        request.no_stretch_boundary_after_clusters = HashSet::from([1]);
    });
    assert_eq!(4.0, closed.unfilled_deficit);
    assert!(closed.allocations.iter().all(|allocation| allocation.kind != GlueKind::InlineObjectRelation));
}

#[test]
fn preferred_inline_object_kinds_chain_until_filled() {
    let clusters = vec![c("中", 0, EM, "k"), c("", 1, 0.0, "obj"), c("中", 2, EM, "k")];
    let roles = vec![FontRole::CjkText, FontRole::Unknown, FontRole::CjkText];
    let edges = vec![
        e(EastAsianSpacingValue::Wide, EastAsianSpacingValue::Wide, true),
        e(EastAsianSpacingValue::Other, EastAsianSpacingValue::Other, false),
        e(EastAsianSpacingValue::Wide, EastAsianSpacingValue::Wide, true),
    ];
    let plan = justify(&clusters, &roles, &edges, IntRange::new(0, 2), 36.0, &Justifier::default(), |request| {
        request.preferred_inline_object_boundary_after_clusters = HashMap::from([
            (1, InlineObjectPreferredStretch::new(InlineObjectPreferredStretchKind::PunctuationTrailing, 4.0, 6.0)),
            (0, InlineObjectPreferredStretch::new(InlineObjectPreferredStretchKind::Relation, 4.0, 6.0)),
        ]);
    });
    assert_eq!(2, plan.allocations.len());
    assert_eq!(0.0, plan.unfilled_deficit);
    assert!(plan.allocations.iter().all(|allocation| matches!(allocation.kind, GlueKind::InlineObjectPunctuationTrailing | GlueKind::InlineObjectRelation)));
}

#[test]
fn western_dominant_line_stays_ragged() {
    let (clusters, roles, edges) = latin_space_latin(8.0, EM, EM);
    let plan = justify(&clusters, &roles, &edges, IntRange::new(0, 2), 64.0, &Justifier::default(), |_| {});
    assert_eq!(Some("WesternDominantLineNaturalSpacing".to_owned()), plan.fallback_reason);
    assert!(plan.unfilled_deficit > 0.0);

    let closed_object = justify(&clusters, &roles, &edges, IntRange::new(0, 2), 64.0, &Justifier::default(), |request| {
        request.uniform_inline_object_boundary_after_clusters = HashSet::from([0]);
        request.no_stretch_boundary_after_clusters = HashSet::from([0]);
    });
    assert_eq!(Some("WesternDominantLineNaturalSpacing".to_owned()), closed_object.fallback_reason);
}

#[test]
fn uniform_object_boundary_opens_the_gate_and_fills() {
    let (clusters, roles, edges) = latin_space_latin(8.0, EM, EM);
    let plan = justify(&clusters, &roles, &edges, IntRange::new(0, 2), 64.0, &Justifier::default(), |request| {
        request.uniform_inline_object_boundary_after_clusters = HashSet::from([0]);
    });

    assert_eq!(None, plan.fallback_reason);
    assert!(plan.allocations.iter().any(|allocation| allocation.kind == GlueKind::InlineObjectBoundary));
    assert_eq!(0.0, plan.unfilled_deficit);
}

#[test]
fn emergency_tracking_fills_the_residual_for_authorized_boundaries() {
    let clusters = vec![c("a", 0, EM, "lat"), c("b", 1, EM, "lat")];
    let roles = vec![FontRole::LatinText, FontRole::LatinText];
    let edges = vec![
        e(EastAsianSpacingValue::Narrow, EastAsianSpacingValue::Narrow, false),
        e(EastAsianSpacingValue::Narrow, EastAsianSpacingValue::Narrow, false),
    ];
    let plan = justify(&clusters, &roles, &edges, IntRange::new(0, 1), 36.0, &Justifier::default(), |request| {
        request.emergency_tracking_boundary_after_clusters = HashMap::from([(0, "token".to_owned())]);
    });
    let [allocation] = plan.allocations.as_slice() else {
        panic!("expected exactly one allocation: {:?}", plan.allocations);
    };
    assert_eq!(GlueKind::EmergencyGraphemeTracking, allocation.kind);
    assert_eq!("EmergencyGraphemeTracking:token", allocation.reason);
    assert_eq!(4.0, allocation.delta);
    assert_eq!(0.0, plan.unfilled_deficit);

    let preferred = justify(&clusters, &roles, &edges, IntRange::new(0, 1), 36.0, &Justifier::default(), |request| {
        request.emergency_tracking_boundary_after_clusters = HashMap::from([(0, "token".to_owned())]);
        request.preferred_emergency_tracking_boundary_after_clusters = HashMap::from([(0, "code".to_owned())]);
    });
    let [preferred_allocation] = preferred.allocations.as_slice() else {
        panic!("expected exactly one preferred allocation: {:?}", preferred.allocations);
    };
    assert_eq!("TerminalTechnicalEmergencyTracking:code", preferred_allocation.reason);
    assert_eq!(GlueKind::EmergencyGraphemeTracking, preferred_allocation.kind);
}

#[test]
fn cjk_line_with_no_opportunities_reports_unfilled_without_fallback() {
    let clusters = vec![c("中", 0, EM, "k")];
    let roles = vec![FontRole::CjkText];
    let edges = vec![e(EastAsianSpacingValue::Wide, EastAsianSpacingValue::Wide, true)];
    let plan = justify(&clusters, &roles, &edges, IntRange::new(0, 0), 20.0, &Justifier::default(), |_| {});

    assert!(plan.allocations.is_empty());
    assert_eq!(4.0, plan.unfilled_deficit);
    assert_eq!(None, plan.fallback_reason);
}

#[test]
fn uniform_text_boundaries_exclude_protected_classes() {
    let (clusters, roles, edges) = cjk_latin();
    let plain = justify(&clusters, &roles, &edges, IntRange::new(0, 1), 36.0, &Justifier::default(), |request| {
        request.cjk_latin_space_max_em = 0.25;
    });
    assert_eq!(GlueKind::CjkInterChar, plain.allocations.first().expect("expected plain allocation").kind);

    let bracket = justify(&clusters, &roles, &edges, IntRange::new(0, 1), 36.0, &Justifier::default(), |request| {
        request.cjk_latin_space_max_em = 0.25;
        request.western_bracket_cjk_inter_char_boundary_after_clusters = HashSet::from([0]);
    });
    assert_eq!("WesternBracketCjkInterChar", bracket.allocations.first().expect("expected bracket allocation").reason);

    let physical = justify(&clusters, &roles, &edges, IntRange::new(0, 1), 36.0, &Justifier::default(), |request| {
        request.cjk_latin_space_max_em = 0.25;
        request.attached_inline_physical_boundary_after_clusters = HashSet::from([0]);
    });
    assert_eq!(4.0, physical.unfilled_deficit);

    let virtual_owned = justify(&clusters, &roles, &edges, IntRange::new(0, 1), 36.0, &Justifier::default(), |request| {
        request.cjk_latin_space_max_em = 0.25;
        request.attached_inline_virtual_boundary_after_clusters = HashMap::from([(0, -1)]);
    });
    assert_eq!("AttachedInlineVirtualInterChar", virtual_owned.allocations.first().expect("expected virtual allocation").reason);

    let uniform_object = justify(&clusters, &roles, &edges, IntRange::new(0, 1), 36.0, &Justifier::default(), |request| {
        request.cjk_latin_space_max_em = 0.25;
        request.uniform_inline_object_boundary_after_clusters = HashSet::from([0]);
    });
    assert_eq!(GlueKind::InlineObjectBoundary, uniform_object.allocations.first().expect("expected object allocation").kind);

    let bracket_physical = justify(&clusters, &roles, &edges, IntRange::new(0, 1), 36.0, &Justifier::default(), |request| {
        request.cjk_latin_space_max_em = 0.25;
        request.western_bracket_cjk_inter_char_boundary_after_clusters = HashSet::from([0]);
        request.attached_inline_physical_boundary_after_clusters = HashSet::from([0]);
    });
    assert_eq!(4.0, bracket_physical.unfilled_deficit);
    let bracket_object = justify(&clusters, &roles, &edges, IntRange::new(0, 1), 36.0, &Justifier::default(), |request| {
        request.cjk_latin_space_max_em = 0.25;
        request.western_bracket_cjk_inter_char_boundary_after_clusters = HashSet::from([0]);
        request.uniform_inline_object_boundary_after_clusters = HashSet::from([0]);
    });
    assert_eq!(GlueKind::InlineObjectBoundary, bracket_object.allocations.first().expect("expected bracket object allocation").kind);
}

#[test]
fn attached_inline_virtual_inter_char_honours_no_stretch_protection() {
    let clusters = vec![c("a", 0, EM, "lat"), c("", 1, 0.0, "obj"), c("b", 2, EM, "lat"), c("中", 3, EM, "k")];
    let roles = vec![FontRole::LatinText, FontRole::Unknown, FontRole::LatinText, FontRole::CjkText];
    let edges = vec![
        e(EastAsianSpacingValue::Narrow, EastAsianSpacingValue::Narrow, false),
        e(EastAsianSpacingValue::Other, EastAsianSpacingValue::Other, false),
        e(EastAsianSpacingValue::Narrow, EastAsianSpacingValue::Narrow, false),
        e(EastAsianSpacingValue::Other, EastAsianSpacingValue::Wide, true),
    ];
    let happy = justify(&clusters, &roles, &edges, IntRange::new(0, 3), 60.0, &Justifier::default(), |request| {
        request.attached_inline_virtual_boundary_after_clusters = HashMap::from([(1, 0)]);
    });
    assert_eq!("AttachedInlineVirtualInterChar", happy.allocations.first().expect("expected attached allocation").reason);
    assert_eq!(0.0, happy.unfilled_deficit);

    for (no_stretch, no_stretch_after) in [
        (HashSet::from([0]), HashSet::new()),
        (HashSet::from([2]), HashSet::new()),
        (HashSet::new(), HashSet::from([0])),
    ] {
        let plan = justify(&clusters, &roles, &edges, IntRange::new(0, 3), 60.0, &Justifier::default(), |request| {
            request.attached_inline_virtual_boundary_after_clusters = HashMap::from([(1, 0)]);
            request.no_stretch_boundary_clusters = no_stretch;
            request.no_stretch_boundary_after_clusters = no_stretch_after;
        });
        assert!(plan.allocations.iter().all(|allocation| allocation.reason != "AttachedInlineVirtualInterChar"));
        assert!(plan.unfilled_deficit > 0.0);
    }

    let promoted = justify(&clusters, &roles, &edges, IntRange::new(0, 3), 60.0, &Justifier::default(), |request| {
        request.attached_inline_virtual_boundary_after_clusters = HashMap::from([(1, 0)]);
        request.uniform_inline_object_boundary_after_clusters = HashSet::from([1]);
    });
    assert_eq!(
        GlueKind::InlineObjectBoundary,
        promoted
            .allocations
            .iter()
            .find(|allocation| allocation.target_cluster_index == 1)
            .expect("expected promoted object allocation")
            .kind
    );
}

#[test]
fn attached_inline_virtual_sino_western_needs_stretch_enabled() {
    let clusters = vec![c("a", 0, EM, "lat"), c("", 1, 0.0, "obj"), c("b", 2, EM, "lat"), c("中", 3, EM, "k")];
    let roles = vec![FontRole::LatinText, FontRole::Unknown, FontRole::LatinText, FontRole::CjkText];
    let edges = vec![
        e(EastAsianSpacingValue::Narrow, EastAsianSpacingValue::Narrow, false),
        e(EastAsianSpacingValue::Other, EastAsianSpacingValue::Other, false),
        e(EastAsianSpacingValue::Narrow, EastAsianSpacingValue::Narrow, false),
        e(EastAsianSpacingValue::Other, EastAsianSpacingValue::Wide, true),
    ];
    let plan = justify(&clusters, &roles, &edges, IntRange::new(0, 3), 60.0, &Justifier::default(), |request| {
        request.attached_inline_virtual_boundary_after_clusters = HashMap::from([(1, 0)]);
        request.attached_inline_virtual_sino_western_boundary_after_clusters = HashSet::from([1]);
        request.allow_sino_western_gap_stretch = false;
    });
    assert!(plan.allocations.iter().all(|allocation| allocation.reason != "AttachedInlineVirtualInterChar"));
    assert!(plan.unfilled_deficit > 0.0);
}

#[test]
fn empty_cluster_range_defers_every_tier_loop() {
    let (clusters, roles, edges) = cjk_latin();
    let plan = justify(&clusters, &roles, &edges, IntRange::new(1, 0), 16.0, &Justifier::default(), |_| {});

    assert!(plan.allocations.is_empty());
    assert_eq!(16.0, plan.unfilled_deficit);
    assert_eq!(Some("WesternDominantLineNaturalSpacing".to_owned()), plan.fallback_reason);
}

#[test]
fn paragraph_edge_space_lines_cover_the_boundary_guards() {
    let leading = vec![c(" ", 0, 4.0, "k"), c("中", 1, EM, "k"), c("x", 2, EM, "lat")];
    let roles = vec![FontRole::LatinText, FontRole::CjkText, FontRole::LatinText];
    let edges = vec![
        e(EastAsianSpacingValue::Narrow, EastAsianSpacingValue::Narrow, false),
        e(EastAsianSpacingValue::Wide, EastAsianSpacingValue::Wide, true),
        e(EastAsianSpacingValue::Narrow, EastAsianSpacingValue::Narrow, false),
    ];
    let leading_plan = justify(&leading, &roles, &edges, IntRange::new(0, 2), 40.0, &Justifier::default(), |_| {});
    assert_eq!(0, leading_plan.allocations.iter().filter(|allocation| allocation.kind == GlueKind::WordSpace).count());
    let leading_gap = leading_plan.allocations.iter().find(|allocation| allocation.kind == GlueKind::CjkLatinSpace).expect("expected leading Sino-Western gap");
    assert_eq!(1, leading_gap.target_cluster_index);
    assert_eq!(0.0, leading_plan.unfilled_deficit);

    let trailing = vec![c("中", 0, EM, "k"), c("x", 1, EM, "lat"), c(" ", 2, 4.0, "k")];
    let trailing_plan = justify(&trailing, &roles, &edges, IntRange::new(0, 2), 40.0, &Justifier::default(), |_| {});
    assert_eq!(0, trailing_plan.allocations.iter().filter(|allocation| allocation.kind == GlueKind::WordSpace).count());
    let trailing_gap = trailing_plan.allocations.iter().find(|allocation| allocation.kind == GlueKind::CjkLatinSpace).expect("expected trailing Sino-Western gap");
    assert_eq!(0, trailing_gap.target_cluster_index);
    assert_eq!(0.0, trailing_plan.unfilled_deficit);
}

#[test]
fn compress_distributes_tier_by_tier() {
    let tier_one = ShrinkOpportunity::new(0, 1, 4.0, ShrinkChannel::TrailingGlue);
    let tier_two = ShrinkOpportunity::new(1, 2, 16.0, ShrinkChannel::LeadingGlue);
    let plan = Justifier::default().compress(12.0, &[tier_two, tier_one]);

    assert_eq!(0.0, plan.unfilled_surplus);
    assert_eq!(
        vec![
            PushInAllocation { cluster_index: 0, shrink: 4.0, available_capacity: 4.0, channel: ShrinkChannel::TrailingGlue },
            PushInAllocation { cluster_index: 1, shrink: 8.0, available_capacity: 16.0, channel: ShrinkChannel::LeadingGlue },
        ],
        plan.allocations
    );
}

#[test]
fn compress_early_exits_and_filters_degenerate_inputs() {
    let justifier = Justifier::default();
    let empty = justifier.compress(0.0, &[]);
    assert!(empty.allocations.is_empty());
    assert_eq!(0.0, empty.surplus_before);
    assert_eq!(0.0, empty.unfilled_surplus);

    let zero = ShrinkOpportunity::new(0, 1, 0.0, ShrinkChannel::TrailingGlue);
    let unfilled = justifier.compress(8.0, &[zero]);
    assert!(unfilled.allocations.is_empty());
    assert_eq!(8.0, unfilled.unfilled_surplus);

    let big = ShrinkOpportunity::new(0, 1, 16.0, ShrinkChannel::TrailingGlue);
    let other = ShrinkOpportunity::new(1, 2, 16.0, ShrinkChannel::TrailingGlue);
    let capped = justifier.compress(8.0, &[big, other]);
    assert_eq!(
        vec![PushInAllocation { cluster_index: 0, shrink: 8.0, available_capacity: 16.0, channel: ShrinkChannel::TrailingGlue }],
        capped.allocations
    );
    assert_eq!(0.0, capped.unfilled_surplus);
}
