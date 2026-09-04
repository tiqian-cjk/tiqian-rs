use tiqian::clreq::clreq_profile::{
    AutoSpacePolicy, KinsokuLevel, PunctuationGluePlacement, PunctuationWidthPolicy,
};
use tiqian::common::HashSet;
use tiqian::core::east_asian_spacing::{EastAsianSpacingEdges, EastAsianSpacingValue};
use tiqian::core::geometry::{text_range, Rect};
use tiqian::core::layout_model::{Cluster, Glyph};
use tiqian::core::text::Text;
use tiqian::core::text_model::{InlineAttachment, InlineBoxSpan};
use tiqian::font::font_policy::FontRole;
use tiqian::layout::punctuation_geometry_ledger::PunctuationGeometryLedger;
use tiqian::layout::punctuation_geometry_stage::{
    apply_auto_space_policy, apply_inline_box_spans, attached_ascii_point_mark_kinsoku,
    inline_object_attached_kinsoku, inline_object_attached_marks, is_attached_ascii_point_mark_at,
    is_east_asian_spacing_boundary_at, punctuation_atoms, InlineObjectAttachedMark,
};
use tiqian::layout::punctuation_model::{
    PunctuationAtomBuilder, PunctuationInkInput, PunctuationSpacingAdjustment,
    PunctuationSpacingCompressionResult, PunctuationSpacingCompressor,
};

const EM: f32 = 16.0;

fn cluster(text: &str, start: i32, advance: f32, font_key: &str) -> Cluster {
    Cluster::new(
        text_range(start, start + text.chars().count() as i32),
        Text::from(text),
        font_key.to_owned(),
        advance,
    )
}

fn display_cluster(text: &str, display: &str, start: i32, advance: f32, font_key: &str) -> Cluster {
    Cluster::with_display_text(
        text_range(start, start + text.chars().count() as i32),
        Text::from(text),
        Text::from(display),
        font_key.to_owned(),
        advance,
    )
}

fn atoms(clusters: &[Cluster]) -> Vec<tiqian::layout::punctuation_model::PunctuationAtom> {
    let builder = PunctuationAtomBuilder::default();
    clusters
        .iter()
        .flat_map(|cluster| punctuation_atoms(
            cluster, EM, &builder, &[], PunctuationGluePlacement::MainlandSimplified,
            PunctuationWidthPolicy::default(),
        ))
        .collect()
}

fn ledger(clusters: Vec<Cluster>, atoms: &[tiqian::layout::punctuation_model::PunctuationAtom]) -> PunctuationGeometryLedger {
    PunctuationGeometryLedger::from(
        clusters,
        atoms,
        &PunctuationSpacingCompressor.compress(atoms, EM),
    )
}

fn edge(value: EastAsianSpacingValue) -> EastAsianSpacingEdges {
    EastAsianSpacingEdges { leading: value, trailing: value, contains_wide: value == EastAsianSpacingValue::Wide }
}

#[test]
fn halt_advance_is_rejected_at_zero_and_at_full_width() {
    let builder = PunctuationAtomBuilder::default();
    let zero = builder.build('，', text_range(0, 1), EM, Some(PunctuationInkInput::builder(16.0).halt_advance(Some(0.0)).build()), PunctuationGluePlacement::MainlandSimplified, PunctuationWidthPolicy::default()).unwrap();
    assert_eq!(None, zero.halt_advance);
    assert_eq!("ProfileGlueFallbackWithoutFontGeometry", zero.geometry_source);
    let full = builder.build('，', text_range(0, 1), EM, Some(PunctuationInkInput::builder(16.0).halt_advance(Some(16.0)).build()), PunctuationGluePlacement::MainlandSimplified, PunctuationWidthPolicy::default()).unwrap();
    assert_eq!(None, full.halt_advance);
    assert_eq!("ProfileGlueFallbackWithoutFontGeometry", full.geometry_source);
}

#[test]
fn non_finite_halt_placement_is_ignored() {
    let builder = PunctuationAtomBuilder::default();
    let ink = builder.build('·', text_range(0, 1), EM, Some(PunctuationInkInput::builder(16.0).ink_bounds(Some(Rect { left: 8.0, top: 4.0, right: 16.0, bottom: 12.0 })).halt_advance(Some(8.0)).halt_placement_x(Some(f32::NAN)).build()), PunctuationGluePlacement::MainlandSimplified, PunctuationWidthPolicy::default()).unwrap();
    assert_eq!("FontHaltAdvanceWithInkBoundsFittedPlacement", ink.geometry_source);
    let no_ink = builder.build('，', text_range(0, 1), EM, Some(PunctuationInkInput::builder(16.0).halt_advance(Some(8.0)).halt_placement_x(Some(f32::NAN)).build()), PunctuationGluePlacement::MainlandSimplified, PunctuationWidthPolicy::default()).unwrap();
    assert_eq!("FontHaltAdvanceWithProfileFallback", no_ink.geometry_source);
    assert_eq!(8.0, no_ink.trailing_glue.natural);
}

#[test]
fn union_ignores_glyphs_without_bounds() {
    let mark = cluster("，", 0, EM, "cjk");
    let glyphs = [
        Glyph::builder(1, text_range(0, 1), 8.0).bounds(Some(Rect { left: 0.0, top: 0.0, right: 8.0, bottom: 16.0 })).build(),
        Glyph::builder(2, text_range(0, 1), 6.0).x(8.0).build(),
    ];
    let atom = punctuation_atoms(&mark, EM, &PunctuationAtomBuilder::default(), &glyphs, PunctuationGluePlacement::MainlandSimplified, PunctuationWidthPolicy::default()).remove(0);
    assert_eq!(8.0, atom.ink_bounds.unwrap().width());
    assert_eq!(16.0, atom.advance);
}

#[test]
fn attached_mark_walk_stops_mid_run_at_a_gap() {
    let rule = tiqian::layout::kinsoku_rule::ClreqKinsokuRule::default();
    let gapped = vec![display_cluster("x", "", 0, 8.0, "inline-object"), cluster(" ", 1, EM, "latin"), cluster(" ", 2, EM, "latin"), cluster("，", 4, EM, "cjk")];
    assert!(inline_object_attached_marks(&gapped, &[FontRole::Unknown, FontRole::LatinText, FontRole::LatinText, FontRole::CjkPunctuation], KinsokuLevel::Basic, &rule).is_empty());
    let contiguous = vec![display_cluster("x", "", 0, 8.0, "inline-object"), cluster(" ", 1, EM, "latin"), cluster(" ", 2, EM, "latin"), cluster("，", 3, EM, "cjk")];
    let mark = inline_object_attached_marks(&contiguous, &[FontRole::Unknown, FontRole::LatinText, FontRole::LatinText, FontRole::CjkPunctuation], KinsokuLevel::Basic, &rule);
    assert_eq!(vec![1, 2], mark[0].separator_cluster_indices);
    assert_eq!(3, mark[0].mark_cluster_index);
}

#[test]
fn empty_text_clusters_cannot_be_attached_marks() {
    let clusters = vec![display_cluster("x", "", 0, 8.0, "inline-object"), cluster("", 1, EM, "latin")];
    assert!(inline_object_attached_marks(&clusters, &[FontRole::Unknown, FontRole::LatinText], KinsokuLevel::Basic, &tiqian::layout::kinsoku_rule::ClreqKinsokuRule::default()).is_empty());
    let attachments = [InlineObjectAttachedMark { object_cluster_index: 0, separator_cluster_indices: Vec::new(), mark_cluster_index: 1 }];
    let result = inline_object_attached_kinsoku(&clusters, &attachments, &clusters, KinsokuLevel::Basic, 10.0, 10.0);
    assert!(result.extendable_hang_ranges.is_empty());
    assert!(result.impossible_measure_hang_eligible_clusters.is_empty());
    assert_eq!(1, result.decisions.len());
}

#[test]
fn ascii_point_mark_kinsoku_skips_empty_text_clusters() {
    let kinsoku = |clusters: &[Cluster]| attached_ascii_point_mark_kinsoku(clusters, &vec![FontRole::LatinText; clusters.len()], clusters, KinsokuLevel::Basic, 100.0, 100.0);
    let empty_mark = vec![cluster("中", 0, EM, "cjk"), display_cluster("", "x", 1, EM, "latin")];
    assert!(kinsoku(&empty_mark).decisions.is_empty());
    let empty_previous = vec![display_cluster("", "x", 0, EM, "latin"), cluster(",", 0, EM, "latin")];
    assert!(kinsoku(&empty_previous).decisions.is_empty());
    let empty_next = vec![cluster("中", 0, EM, "cjk"), cluster(",", 1, 8.0, "latin"), display_cluster("", "x", 2, EM, "latin")];
    let roles = [FontRole::CjkText, FontRole::LatinText, FontRole::LatinText];
    let result = attached_ascii_point_mark_kinsoku(&empty_next, &roles, &empty_next, KinsokuLevel::Basic, 100.0, 100.0);
    assert_eq!(vec![(0, 1)], result.unbreakable_ranges);
    assert_eq!(1, result.decisions.len());
    let gapped = vec![cluster("中", 0, EM, "cjk"), cluster(",", 1, 8.0, "latin"), cluster(",", 3, 8.0, "latin")];
    let result = attached_ascii_point_mark_kinsoku(&gapped, &roles, &gapped, KinsokuLevel::Basic, 100.0, 100.0);
    assert_eq!(vec![(0, 1)], result.unbreakable_ranges);
    assert_eq!(1, result.decisions.len());
}

#[test]
fn attached_run_at_paragraph_end_emits_no_auto_space() {
    let clusters = [cluster("中", 0, EM, "cjk"), cluster("r", 1, EM, "latin")];
    let result = apply_auto_space_policy(&clusters, &[edge(EastAsianSpacingValue::Wide), edge(EastAsianSpacingValue::Other)], &[InlineAttachment::None, InlineAttachment::Previous], AutoSpacePolicy::default_policy(), EM, &HashSet::new(), &HashSet::new());
    assert!(result.decisions.is_empty());
    assert_eq!(16.0, result.clusters[1].advance);
}

#[test]
fn virtual_gap_with_empty_previous_text_has_no_narrow_character() {
    let attachments = [InlineAttachment::None, InlineAttachment::Previous, InlineAttachment::None];
    let clusters = [display_cluster("", "y", 0, EM, "latin"), cluster("r", 1, EM, "latin"), cluster("中", 2, EM, "cjk")];
    assert!(apply_auto_space_policy(&clusters, &[edge(EastAsianSpacingValue::Narrow), EastAsianSpacingEdges { leading: EastAsianSpacingValue::Narrow, trailing: EastAsianSpacingValue::Other, contains_wide: false }, edge(EastAsianSpacingValue::Wide)], &attachments, AutoSpacePolicy::default_policy(), EM, &HashSet::new(), &HashSet::new()).decisions.is_empty());
    let both_narrow = [cluster("a", 0, 8.0, "latin"), cluster("r", 1, EM, "latin"), cluster("a", 2, 8.0, "latin")];
    assert!(apply_auto_space_policy(&both_narrow, &[edge(EastAsianSpacingValue::Narrow), EastAsianSpacingEdges { leading: EastAsianSpacingValue::Narrow, trailing: EastAsianSpacingValue::Other, contains_wide: false }, edge(EastAsianSpacingValue::Narrow)], &attachments, AutoSpacePolicy::default_policy(), EM, &HashSet::new(), &HashSet::new()).decisions.is_empty());
}

#[test]
fn typed_space_with_empty_text_neighbours_keeps_its_width() {
    let attachments = [InlineAttachment::None; 3];
    let leading = [cluster("中", 0, EM, "cjk"), cluster(" ", 1, EM, "latin"), display_cluster("", "y", 2, EM, "latin")];
    assert!(apply_auto_space_policy(&leading, &[edge(EastAsianSpacingValue::Wide), edge(EastAsianSpacingValue::Other), edge(EastAsianSpacingValue::Narrow)], &attachments, AutoSpacePolicy::default_policy(), EM, &HashSet::new(), &HashSet::new()).decisions.is_empty());
    let trailing = [display_cluster("", "y", 0, EM, "latin"), cluster(" ", 1, EM, "latin"), cluster("中", 2, EM, "cjk")];
    assert!(apply_auto_space_policy(&trailing, &[edge(EastAsianSpacingValue::Narrow), edge(EastAsianSpacingValue::Other), edge(EastAsianSpacingValue::Wide)], &attachments, AutoSpacePolicy::default_policy(), EM, &HashSet::new(), &HashSet::new()).decisions.is_empty());
    let wide = [cluster("中", 0, EM, "cjk"), cluster(" ", 1, EM, "latin"), cluster("中", 2, EM, "cjk")];
    let result = apply_auto_space_policy(&wide, &[edge(EastAsianSpacingValue::Wide), edge(EastAsianSpacingValue::Other), edge(EastAsianSpacingValue::Wide)], &attachments, AutoSpacePolicy::default_policy(), EM, &HashSet::new(), &HashSet::new());
    assert!(result.decisions.is_empty());
    assert_eq!(16.0, result.clusters[1].advance);
}

#[test]
fn spacing_boundaries_at_list_edges_are_false() {
    let trailing = [cluster("中", 0, EM, "cjk"), cluster(" ", 1, EM, "latin")];
    assert!(!is_east_asian_spacing_boundary_at(1, &trailing, &[edge(EastAsianSpacingValue::Wide), edge(EastAsianSpacingValue::Other)]));
    let leading = [cluster(" ", 0, EM, "latin"), cluster("中", 1, EM, "cjk")];
    assert!(!is_east_asian_spacing_boundary_at(1, &leading, &[edge(EastAsianSpacingValue::Other), edge(EastAsianSpacingValue::Wide)]));
}

#[test]
fn attached_ascii_point_mark_check_skips_empty_previous_text() {
    let clusters = [display_cluster("", "x", 0, EM, "latin"), cluster(",", 0, EM, "latin")];
    assert!(!is_attached_ascii_point_mark_at(&clusters, 1));
}

#[test]
fn inline_box_span_with_zero_net_structural_edge_still_applies_leading() {
    let clusters = [cluster("a", 0, 8.0, "latin")];
    let result = apply_inline_box_spans(&clusters, &[
        InlineBoxSpan::with_edges(text_range(0, 1), 2.0, 0.0),
        InlineBoxSpan::with_edges(text_range(0, 1), 0.0, -2.0),
    ]);
    assert_eq!(8.0, result.clusters[0].advance);
    assert_eq!(2.0, result.clusters[0].leading_layout_advance);
    assert!(result.advance_by_cluster.is_empty());
    assert_eq!(2, result.decisions.len());
}

#[test]
fn resolve_clusters_applies_glyph_shift_with_unchanged_advance() {
    let mark = cluster("「", 0, EM, "cjk");
    let glyph = Glyph::builder(1, text_range(0, 1), 8.0).build();
    let atoms = punctuation_atoms(&mark, EM, &PunctuationAtomBuilder::default(), &[glyph], PunctuationGluePlacement::MainlandSimplified, PunctuationWidthPolicy::default());
    let resolved = ledger(vec![mark], &atoms).resolve_clusters();
    assert_eq!(16.0, resolved[0].advance);
    assert_eq!(8.0, resolved[0].glyph_inline_shift);
}

#[test]
fn glue_capacities_mark_centred_frames_as_paired() {
    let clusters = vec![cluster("，", 0, EM, "cjk")];
    let builder = PunctuationAtomBuilder::default();
    let atoms = punctuation_atoms(&clusters[0], EM, &builder, &[], PunctuationGluePlacement::Traditional, PunctuationWidthPolicy::default());
    let capacity = ledger(clusters, &atoms).glue_capacities()[&0];
    assert!(capacity.paired);
    assert_eq!(4.0, capacity.leading);
    assert_eq!(4.0, capacity.trailing);
}

#[test]
fn attached_boundary_with_plain_previous_cluster_keeps_the_right_budget() {
    let clusters = vec![cluster("中", 0, EM, "cjk"), cluster("r", 1, EM, "latin"), cluster("「", 2, EM, "cjk")];
    let atoms = atoms(&clusters);
    let result = ledger(clusters, &atoms).resolve_attached_inline_punctuation_boundaries(&[InlineAttachment::None, InlineAttachment::Previous, InlineAttachment::None], &atoms, EM);
    assert!(result.decisions.is_empty());
    assert!(result.trailing_glue_by_cluster.is_empty());
    assert_eq!(16.0, result.geometry.resolve_clusters()[2].advance);
}

#[test]
fn attached_trailing_glue_widens_a_budgeted_end_cluster() {
    let clusters = vec![cluster("」", 0, EM, "cjk"), cluster("」", 1, EM, "cjk"), cluster("「", 2, EM, "cjk")];
    let mut atoms = atoms(&clusters);
    atoms[0].trailing_glue.natural = 12.0;
    atoms[0].trailing_glue.max = 12.0;
    let result = PunctuationGeometryLedger::from(clusters, &atoms, &PunctuationSpacingCompressionResult::new(Vec::new())).resolve_attached_inline_punctuation_boundaries(&[InlineAttachment::None, InlineAttachment::Previous, InlineAttachment::None], &atoms, EM);
    assert_eq!(4.0, result.trailing_glue_by_cluster[&1]);
    assert_eq!(20.0, result.geometry.resolve_clusters()[1].advance);
}

#[test]
fn spacing_plan_ignores_targets_outside_the_budgets() {
    let clusters = vec![cluster("中", 0, EM, "cjk"), cluster("。", 1, EM, "cjk")];
    let atoms = atoms(&clusters);
    let stray = vec![
        PunctuationSpacingAdjustment { range: text_range(0, 2), reduction_target_range: text_range(0, 1), left_char: '中', right_char: '。', natural_inner_glue: 8.0, adjusted_inner_glue: 0.0, reduction: 8.0, reason: "test-stray".to_owned() },
        PunctuationSpacingAdjustment { range: text_range(8, 9), reduction_target_range: text_range(8, 9), left_char: '中', right_char: '。', natural_inner_glue: 8.0, adjusted_inner_glue: 0.0, reduction: 8.0, reason: "test-past-end".to_owned() },
    ];
    let capacity = PunctuationGeometryLedger::from(clusters, &atoms, &PunctuationSpacingCompressionResult::new(stray)).glue_capacities();
    assert!(!capacity.contains_key(&0));
    assert_eq!(8.0, capacity[&1].trailing);
}

#[test]
fn centred_adjacency_consumes_both_sides_equally() {
    let clusters = vec![cluster("，", 0, EM, "cjk"), cluster("，", 1, EM, "cjk")];
    let builder = PunctuationAtomBuilder::default();
    let atoms: Vec<_> = clusters.iter().flat_map(|cluster| punctuation_atoms(cluster, EM, &builder, &[], PunctuationGluePlacement::Traditional, PunctuationWidthPolicy::default())).collect();
    let ledger = ledger(clusters, &atoms);
    assert!(!ledger.glue_capacities().contains_key(&0));
    assert_eq!(4.0, ledger.glue_capacities()[&1].leading);
    let resolved = ledger.resolve_clusters();
    assert_eq!(8.0, resolved[0].advance);
    assert_eq!(16.0, resolved[1].advance);
}

#[test]
fn attached_boundary_reason_falls_back_to_natural_without_left_atom() {
    let clusters = vec![cluster("」", 0, EM, "cjk"), cluster("r", 1, EM, "latin"), cluster("「", 2, EM, "cjk")];
    let atoms = atoms(&clusters);
    let result = ledger(clusters, &atoms).resolve_attached_inline_punctuation_boundaries(&[InlineAttachment::None, InlineAttachment::Previous, InlineAttachment::None], &[], EM);
    assert_eq!("AttachedInlineVirtualPunctuationBoundary:natural", result.decisions[0].reason);
    assert_eq!('」', result.decisions[0].left_char);
    assert_eq!(8.0, result.trailing_glue_by_cluster[&1]);
}
