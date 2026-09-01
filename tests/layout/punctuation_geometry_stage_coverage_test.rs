use tiqian::clreq::clreq_profile::{
    AutoSpaceMode, AutoSpacePolicy, KinsokuLevel, PunctuationGluePlacement, PunctuationWidthPolicy,
};
use tiqian::common::HashSet;
use tiqian::core::east_asian_spacing::{EastAsianSpacingEdges, EastAsianSpacingValue};
use tiqian::core::geometry::{Rect, TextRange};
use tiqian::core::layout_model::{Cluster, Glyph};
use tiqian::core::text::Text;
use tiqian::core::text_model::{InlineAttachment, InlineBoxSpan};
use tiqian::font::font_policy::FontRole;
use tiqian::layout::kinsoku_rule::ClreqKinsokuRule;
use tiqian::layout::punctuation_geometry_stage::{
    apply_auto_space_policy, apply_inline_box_spans, attached_ascii_point_mark_kinsoku,
    inline_object_attached_kinsoku, inline_object_attached_marks, is_attached_ascii_point_mark_at,
    is_east_asian_spacing_boundary_at, punctuation_atoms, InlineObjectAttachedMark,
};
use tiqian::layout::punctuation_model::PunctuationAtomBuilder;

const EM: f32 = 16.0;

fn cluster(text: &str, start: i32, advance: f32, font_key: &str) -> Cluster {
    Cluster::new(
        TextRange::new(start, start + text.encode_utf16().count() as i32),
        Text::from(text),
        font_key.to_owned(),
        advance,
    )
}

fn display_cluster(text: &str, display: &str, start: i32, advance: f32, font_key: &str) -> Cluster {
    Cluster::with_display_text(
        TextRange::new(start, start + text.encode_utf16().count() as i32),
        Text::from(text),
        Text::from(display),
        font_key.to_owned(),
        advance,
    )
}

fn glyph(id: u32, advance: f32, x: f32, bounds: Option<Rect>) -> Glyph {
    Glyph::builder(id, TextRange::new(0, 1), advance)
        .x(x)
        .bounds(bounds)
        .build()
}

fn atoms(cluster: &Cluster, glyphs: &[Glyph]) -> Vec<tiqian::layout::punctuation_model::PunctuationAtom> {
    punctuation_atoms(
        cluster,
        EM,
        &PunctuationAtomBuilder::default(),
        glyphs,
        PunctuationGluePlacement::MainlandSimplified,
        PunctuationWidthPolicy::default(),
    )
}

fn edges(leading: EastAsianSpacingValue, trailing: EastAsianSpacingValue) -> EastAsianSpacingEdges {
    EastAsianSpacingEdges {
        leading,
        trailing,
        contains_wide: leading == EastAsianSpacingValue::Wide,
    }
}

fn inline_object(start: i32) -> Cluster {
    display_cluster("x", "", start, 8.0, "inline-object")
}

#[test]
fn multiple_glyphs_for_one_character_union_into_a_single_ink_box() {
    let mark = cluster("，", 0, EM, "cjk");
    let atom = atoms(
        &mark,
        &[
            glyph(1, 8.0, 0.0, Some(Rect { left: 0.0, top: 0.0, right: 8.0, bottom: 16.0 })),
            glyph(2, 6.0, 8.0, Some(Rect { left: 0.0, top: 0.0, right: 6.0, bottom: 16.0 })),
        ],
    )[0]
        .clone();
    assert_eq!(14.0, atom.ink_bounds.unwrap().width());
    assert_eq!(16.0, atom.ink_bounds.unwrap().bottom);
    assert_eq!(16.0, atom.advance);
    assert_eq!(None, atom.ink_bounds_fallback);
}

#[test]
fn union_without_bounds_falls_back_to_the_first_glyph() {
    let mark = cluster("，", 0, EM, "cjk");
    let atom = atoms(&mark, &[glyph(1, 8.0, 0.0, None), glyph(2, 6.0, 8.0, None)])[0].clone();
    assert_eq!(Some("shaper-no-ink-bounds".to_owned()), atom.ink_bounds_fallback);
    assert_eq!(16.0, atom.advance);
    assert_eq!(8.0, atom.body_width);
    assert_eq!(8.0, atom.trailing_glue.natural);
}

#[test]
fn glyphless_clusters_use_the_pure_policy_path() {
    let atom = atoms(&cluster("，", 0, EM, "cjk"), &[])[0].clone();
    assert_eq!("ProfileGlueFallbackWithoutFontGeometry", atom.geometry_source);
    assert_eq!(None, atom.ink_bounds_fallback);
    assert_eq!(8.0, atom.trailing_glue.natural);
}

#[test]
fn unmatched_glyph_counts_record_the_ambiguous_fallback() {
    let mark = cluster("。，", 0, EM, "cjk");
    let output = atoms(&mark, &[glyph(1, 8.0, 0.0, None), glyph(2, 8.0, 0.0, None), glyph(3, 8.0, 0.0, None)]);
    assert_eq!(2, output.len());
    assert!(output.iter().all(|atom| atom.ink_bounds_fallback.as_deref() == Some("glyph-cluster-mapping-ambiguous")));
    assert!(output.iter().all(|atom| atom.ink_bounds.is_none()));
    assert_eq!(16.0, output[0].advance);
}

#[test]
fn per_character_ink_subtracts_preceding_glyph_pens() {
    let mark = cluster("。，", 0, EM, "cjk");
    let output = atoms(
        &mark,
        &[
            glyph(1, 16.0, 0.0, Some(Rect { left: 2.0, top: 0.0, right: 14.0, bottom: 16.0 })),
            glyph(2, 16.0, 16.0, Some(Rect { left: 2.0, top: 0.0, right: 14.0, bottom: 16.0 })),
        ],
    );
    assert_eq!(2, output.len());
    assert_eq!(TextRange::new(0, 1), output[0].range);
    assert_eq!(TextRange::new(1, 2), output[1].range);
    assert_eq!(12.0, output[0].ink_bounds.unwrap().width());
    assert_eq!(12.0, output[1].ink_bounds.unwrap().width());
    assert!(output.iter().all(|atom| atom.ink_bounds_fallback.is_none()));
}

#[test]
fn empty_display_text_produces_no_atoms() {
    assert!(atoms(&display_cluster("\n", "", 0, 0.0, "mandatory-break"), &[]).is_empty());
}

#[test]
fn attached_marks_collapse_separator_space_before_the_mark() {
    let clusters = vec![inline_object(0), cluster(" ", 1, EM, "latin"), cluster("，", 2, EM, "cjk")];
    let roles = vec![FontRole::Unknown, FontRole::LatinText, FontRole::CjkPunctuation];
    let marks = inline_object_attached_marks(&clusters, &roles, KinsokuLevel::Basic, &ClreqKinsokuRule::default());
    assert_eq!(1, marks.len());
    assert_eq!(0, marks[0].object_cluster_index);
    assert_eq!(vec![1], marks[0].separator_cluster_indices);
    assert_eq!(2, marks[0].mark_cluster_index);
    assert!(inline_object_attached_marks(&clusters, &roles, KinsokuLevel::None, &ClreqKinsokuRule::default()).is_empty());
}

#[test]
fn attached_marks_accept_ascii_point_marks_after_objects() {
    let clusters = vec![inline_object(0), cluster(",", 1, EM, "latin")];
    let roles = vec![FontRole::Unknown, FontRole::LatinText];
    let marks = inline_object_attached_marks(&clusters, &roles, KinsokuLevel::Basic, &ClreqKinsokuRule::default());
    assert_eq!(1, marks[0].mark_cluster_index);
    assert!(marks[0].separator_cluster_indices.is_empty());
}

#[test]
fn attached_marks_reject_missing_objects_and_gapped_ranges() {
    let rule = ClreqKinsokuRule::default();
    assert!(inline_object_attached_marks(
        &[cluster("中", 0, EM, "cjk"), cluster("，", 1, EM, "cjk")],
        &[FontRole::CjkText, FontRole::CjkPunctuation], KinsokuLevel::Basic, &rule,
    ).is_empty());
    assert!(inline_object_attached_marks(
        &[cluster(" ", 0, EM, "latin"), cluster("，", 1, EM, "cjk")],
        &[FontRole::LatinText, FontRole::CjkPunctuation], KinsokuLevel::Basic, &rule,
    ).is_empty());
    assert!(inline_object_attached_marks(
        &[inline_object(0), cluster(" ", 2, EM, "latin"), cluster("，", 3, EM, "cjk")],
        &[FontRole::Unknown, FontRole::LatinText, FontRole::CjkPunctuation], KinsokuLevel::Basic, &rule,
    ).is_empty());
    assert!(inline_object_attached_marks(
        &[cluster("中", 0, EM, "cjk"), cluster("中", 1, EM, "cjk")],
        &[FontRole::CjkText, FontRole::CjkText], KinsokuLevel::Basic, &rule,
    ).is_empty());
}

#[test]
fn inline_object_kinsoku_protects_or_hangs_attached_marks() {
    let clusters = vec![inline_object(0), cluster("，", 1, EM, "cjk")];
    let attachments = vec![InlineObjectAttachedMark { object_cluster_index: 0, separator_cluster_indices: Vec::new(), mark_cluster_index: 1 }];
    assert!(std::panic::catch_unwind(|| {
        inline_object_attached_kinsoku(&clusters, &attachments, &clusters[1..], KinsokuLevel::Basic, 100.0, 100.0)
    }).is_err());
    assert!(inline_object_attached_kinsoku(&clusters, &attachments, &clusters, KinsokuLevel::None, 100.0, 100.0).unbreakable_ranges.is_empty());
    let fits = inline_object_attached_kinsoku(&clusters, &attachments, &clusters, KinsokuLevel::Basic, 100.0, 100.0);
    assert_eq!(vec![(0, 1)], fits.unbreakable_ranges);
    assert_eq!(HashSet::from([1]), fits.forbidden_line_start_clusters);
    assert_eq!("InlineObjectAttachedKinsoku", fits.decisions[0].reason);
    let hangs = inline_object_attached_kinsoku(&clusters, &attachments, &clusters, KinsokuLevel::Basic, 10.0, 10.0);
    assert!(hangs.unbreakable_ranges.is_empty());
    assert_eq!(HashSet::from([1]), hangs.impossible_measure_hang_eligible_clusters);
    assert_eq!(vec![(0, 1)], hangs.extendable_hang_ranges);
    let colon = vec![inline_object(0), cluster("：", 1, EM, "cjk")];
    let blocked = inline_object_attached_kinsoku(&colon, &attachments, &colon, KinsokuLevel::Basic, 10.0, 10.0);
    assert!(blocked.extendable_hang_ranges.is_empty());
    let pair = vec![inline_object(0), cluster("，。", 1, EM, "cjk")];
    assert!(inline_object_attached_kinsoku(&pair, &attachments, &pair, KinsokuLevel::Basic, 10.0, 10.0).extendable_hang_ranges.is_empty());
    assert_eq!(vec![(0, 1)], inline_object_attached_kinsoku(&clusters, &attachments, &clusters, KinsokuLevel::Basic, 5.0, 100.0).unbreakable_ranges);
    let separated = vec![inline_object(0), cluster(" ", 1, EM, "latin"), cluster("，", 2, EM, "cjk")];
    let separated_attachments = vec![InlineObjectAttachedMark { object_cluster_index: 0, separator_cluster_indices: vec![1], mark_cluster_index: 2 }];
    let result = inline_object_attached_kinsoku(&separated, &separated_attachments, &separated, KinsokuLevel::Basic, 100.0, 100.0);
    assert_eq!(HashSet::from([1, 2]), result.forbidden_line_start_clusters);
    assert_eq!("InlineObjectAttachedKinsokuAcrossCollapsedSeparatorSpace", result.decisions[0].reason);
}

#[test]
fn attached_ascii_point_mark_kinsoku_protects_runs() {
    let clusters = vec![cluster("中", 0, EM, "cjk"), cluster(",", 1, 8.0, "latin"), cluster(",", 2, 8.0, "latin")];
    let roles = vec![FontRole::CjkText, FontRole::LatinText, FontRole::LatinText];
    assert!(std::panic::catch_unwind(|| {
        attached_ascii_point_mark_kinsoku(&clusters, &roles, &clusters[1..], KinsokuLevel::Basic, 100.0, 100.0)
    }).is_err());
    assert!(attached_ascii_point_mark_kinsoku(&clusters, &roles, &clusters, KinsokuLevel::None, 100.0, 100.0).unbreakable_ranges.is_empty());
    let fits = attached_ascii_point_mark_kinsoku(&clusters, &roles, &clusters, KinsokuLevel::Basic, 10.0, 100.0);
    assert_eq!(vec![(0, 2)], fits.unbreakable_ranges);
    assert_eq!(HashSet::from([1, 2]), fits.forbidden_line_start_clusters);
    assert_eq!(2, fits.decisions.len());
    assert!(fits.decisions.iter().all(|decision| decision.reason == "AttachedAsciiPointMarkKinsoku"));
    let hangs = attached_ascii_point_mark_kinsoku(&clusters, &roles, &clusters, KinsokuLevel::Basic, 10.0, 5.0);
    assert_eq!(HashSet::from([1, 2]), hangs.impossible_measure_hang_eligible_clusters);
    assert_eq!(vec![(0, 2)], hangs.extendable_hang_ranges);
    let bounded = vec![cluster("中", 0, EM, "cjk"), cluster(",", 1, 8.0, "latin"), cluster("a", 2, 8.0, "latin")];
    let bounded_result = attached_ascii_point_mark_kinsoku(&bounded, &roles, &bounded, KinsokuLevel::Basic, 10.0, 100.0);
    assert_eq!(vec![(0, 1)], bounded_result.unbreakable_ranges);
    let middle = vec![cluster("中", 0, EM, "cjk"), cluster("中", 1, EM, "cjk"), cluster(",", 2, 8.0, "latin")];
    let middle_roles = vec![FontRole::CjkText, FontRole::CjkText, FontRole::LatinText];
    assert_eq!(vec![(1, 2)], attached_ascii_point_mark_kinsoku(&middle, &middle_roles, &middle, KinsokuLevel::Basic, 100.0, 5.0).unbreakable_ranges);
}

#[test]
fn attached_ascii_point_mark_kinsoku_rejects_detached_runs() {
    let kinsoku = |clusters: &[Cluster], roles: &[FontRole]| {
        attached_ascii_point_mark_kinsoku(clusters, roles, clusters, KinsokuLevel::Basic, 100.0, 100.0)
    };
    let after_space = vec![cluster("中", 0, EM, "cjk"), cluster(" ", 1, EM, "latin"), cluster(",", 2, EM, "latin")];
    assert!(kinsoku(&after_space, &[FontRole::CjkText, FontRole::LatinText, FontRole::LatinText]).decisions.is_empty());
    let gapped = vec![cluster("中", 0, EM, "cjk"), cluster(",", 2, EM, "latin")];
    assert!(kinsoku(&gapped, &[FontRole::CjkText, FontRole::LatinText]).decisions.is_empty());
    let object_base = vec![inline_object(0), cluster(",", 1, EM, "latin")];
    assert!(kinsoku(&object_base, &[FontRole::Unknown, FontRole::LatinText]).decisions.is_empty());
    let plain = vec![cluster("中", 0, EM, "cjk"), cluster("a", 1, EM, "latin")];
    assert!(kinsoku(&plain, &[FontRole::CjkText, FontRole::LatinText]).decisions.is_empty());
    let cjk = vec![cluster("中", 0, EM, "cjk"), cluster("，", 1, EM, "cjk")];
    assert!(kinsoku(&cjk, &[FontRole::CjkText, FontRole::CjkPunctuation]).decisions.is_empty());
}

#[test]
fn typed_space_between_wide_and_narrow_is_replaced_by_the_gap() {
    let clusters = vec![cluster("中", 0, EM, "cjk"), cluster(" ", 1, EM, "latin"), cluster("a", 2, 8.0, "latin")];
    let result = apply_auto_space_policy(
        &clusters,
        &[edges(EastAsianSpacingValue::Wide, EastAsianSpacingValue::Wide), edges(EastAsianSpacingValue::Other, EastAsianSpacingValue::Other), edges(EastAsianSpacingValue::Narrow, EastAsianSpacingValue::Narrow)],
        &[InlineAttachment::None; 3],
        AutoSpacePolicy::builder()
            .cjk_latin(AutoSpaceMode::Replace)
            .cjk_digit(AutoSpaceMode::Replace)
            .gap_em(0.125)
            .build(),
        EM, &HashSet::new(), &HashSet::new(),
    );
    let decision = &result.decisions[0];
    assert_eq!("gap", decision.side);
    assert_eq!("Replace", decision.mode);
    assert_eq!("EastAsianSpacing.Wide", decision.boundary_role);
    assert_eq!("TextAutoSpaceReplace:east-asian-spacing-W-space-N", decision.reason);
    assert_eq!(1, decision.characters_affected);
    assert_eq!(14.0, decision.reduction_per_char);
    assert_eq!(14.0, decision.total_reduction);
    assert_eq!(2.0, result.clusters[1].advance);
}

#[test]
fn space_replacement_skips_disabled_mode_null_boundaries_and_exact_widths() {
    let clusters = vec![cluster("中", 0, EM, "cjk"), cluster(" ", 1, EM, "latin"), cluster("a", 2, 8.0, "latin")];
    let spacing = [edges(EastAsianSpacingValue::Wide, EastAsianSpacingValue::Wide), edges(EastAsianSpacingValue::Other, EastAsianSpacingValue::Other), edges(EastAsianSpacingValue::Narrow, EastAsianSpacingValue::Narrow)];
    let disabled = apply_auto_space_policy(&clusters, &spacing, &[InlineAttachment::None; 3], AutoSpacePolicy::disabled(), EM, &HashSet::new(), &HashSet::new());
    assert!(disabled.decisions.is_empty());
    assert_eq!(16.0, disabled.clusters[1].advance);
    let exact = vec![cluster("中", 0, EM, "cjk"), cluster(" ", 1, 2.0, "latin"), cluster("a", 2, 8.0, "latin")];
    let replace = AutoSpacePolicy::builder()
        .cjk_latin(AutoSpaceMode::Replace)
        .cjk_digit(AutoSpaceMode::Replace)
        .gap_em(0.125)
        .build();
    assert!(apply_auto_space_policy(&exact, &spacing, &[InlineAttachment::None; 3], replace, EM, &HashSet::new(), &HashSet::new()).decisions.is_empty());
    let lone = [cluster(" ", 0, EM, "latin")];
    assert!(apply_auto_space_policy(&lone, &[edges(EastAsianSpacingValue::Other, EastAsianSpacingValue::Other)], &[InlineAttachment::None], replace, EM, &HashSet::new(), &HashSet::new()).decisions.is_empty());
    assert!(std::panic::catch_unwind(|| apply_auto_space_policy(&clusters, &[edges(EastAsianSpacingValue::Wide, EastAsianSpacingValue::Wide)], &[InlineAttachment::None; 3], replace, EM, &HashSet::new(), &HashSet::new())).is_err());
    assert!(std::panic::catch_unwind(|| apply_auto_space_policy(&clusters, &spacing, &[InlineAttachment::None], replace, EM, &HashSet::new(), &HashSet::new())).is_err());
    assert!(apply_auto_space_policy(&[], &[], &[], replace, EM, &HashSet::new(), &HashSet::new()).clusters.is_empty());
}

#[test]
fn wide_to_narrow_boundaries_insert_leading_and_trailing_gaps() {
    let insert = AutoSpacePolicy::default();
    let leading = vec![cluster("中", 0, EM, "cjk"), cluster("a", 1, 8.0, "latin")];
    let leading_result = apply_auto_space_policy(&leading, &[edges(EastAsianSpacingValue::Wide, EastAsianSpacingValue::Wide), edges(EastAsianSpacingValue::Narrow, EastAsianSpacingValue::Narrow)], &[InlineAttachment::None; 2], insert, EM, &HashSet::new(), &HashSet::new());
    assert_eq!("leading", leading_result.decisions[0].side);
    assert_eq!("EastAsianSpacing.Wide", leading_result.decisions[0].boundary_role);
    assert_eq!("TextAutoSpaceInsert:east-asian-spacing-W-N", leading_result.decisions[0].reason);
    assert_eq!(-2.0, leading_result.decisions[0].total_reduction);
    assert_eq!(10.0, leading_result.clusters[1].advance);
    let trailing = vec![cluster("a", 0, 8.0, "latin"), cluster("中", 1, EM, "cjk")];
    let trailing_result = apply_auto_space_policy(&trailing, &[edges(EastAsianSpacingValue::Narrow, EastAsianSpacingValue::Narrow), edges(EastAsianSpacingValue::Wide, EastAsianSpacingValue::Wide)], &[InlineAttachment::None; 2], insert, EM, &HashSet::new(), &HashSet::new());
    assert_eq!("trailing", trailing_result.decisions[0].side);
    assert_eq!(10.0, trailing_result.clusters[0].advance);
}

#[test]
fn narrow_inline_boxes_own_their_outer_auto_space() {
    let clusters = vec![cluster("中", 0, EM, "cjk"), cluster("a", 1, 8.0, "latin")];
    let result = apply_auto_space_policy(&clusters, &[edges(EastAsianSpacingValue::Wide, EastAsianSpacingValue::Wide), edges(EastAsianSpacingValue::Narrow, EastAsianSpacingValue::Narrow)], &[InlineAttachment::None; 2], AutoSpacePolicy::default(), EM, &HashSet::from([1]), &HashSet::new());
    assert_eq!("InlineBox.Narrow", result.decisions[0].boundary_role);
    assert_eq!("InlineBoxOuterAutoSpace:leading-W-N", result.decisions[0].reason);
    let trailing = vec![cluster("a", 0, 8.0, "latin"), cluster("中", 1, EM, "cjk")];
    let trailing_result = apply_auto_space_policy(&trailing, &[edges(EastAsianSpacingValue::Narrow, EastAsianSpacingValue::Narrow), edges(EastAsianSpacingValue::Wide, EastAsianSpacingValue::Wide)], &[InlineAttachment::None; 2], AutoSpacePolicy::default(), EM, &HashSet::new(), &HashSet::from([0]));
    assert_eq!("InlineBox.Narrow", trailing_result.decisions[0].boundary_role);
    assert_eq!("InlineBoxOuterAutoSpace:trailing-N-W", trailing_result.decisions[0].reason);
}

#[test]
fn attached_runs_own_one_virtual_gap_at_their_trailing_edge() {
    let clusters = vec![cluster("中", 0, EM, "cjk"), cluster("ref", 1, EM, "latin"), cluster("a", 4, 8.0, "latin")];
    let result = apply_auto_space_policy(&clusters, &[edges(EastAsianSpacingValue::Wide, EastAsianSpacingValue::Wide), edges(EastAsianSpacingValue::Other, EastAsianSpacingValue::Wide), edges(EastAsianSpacingValue::Narrow, EastAsianSpacingValue::Narrow)], &[InlineAttachment::None, InlineAttachment::Previous, InlineAttachment::None], AutoSpacePolicy::default(), EM, &HashSet::new(), &HashSet::new());
    let decision = &result.decisions[0];
    assert_eq!("trailing", decision.side);
    assert_eq!("InlineAttachment.Previous", decision.boundary_role);
    assert_eq!("AttachedInlineVirtualAutoSpace:east-asian-spacing-W-N", decision.reason);
    assert_eq!(18.0, result.clusters[1].advance);
    assert_eq!(8.0, result.clusters[2].advance);
}

#[test]
fn virtual_gaps_respect_narrow_to_wide_edges_and_their_neighbours() {
    let attachments = [InlineAttachment::None, InlineAttachment::Previous, InlineAttachment::None];
    let reversed = vec![cluster("a", 0, 8.0, "latin"), cluster("ref", 1, EM, "latin"), cluster("中", 4, EM, "cjk")];
    let reversed_result = apply_auto_space_policy(&reversed, &[edges(EastAsianSpacingValue::Narrow, EastAsianSpacingValue::Narrow), edges(EastAsianSpacingValue::Narrow, EastAsianSpacingValue::Other), edges(EastAsianSpacingValue::Wide, EastAsianSpacingValue::Wide)], &attachments, AutoSpacePolicy::default(), EM, &HashSet::new(), &HashSet::new());
    assert_eq!(1, reversed_result.decisions.len());
    assert_eq!("AttachedInlineVirtualAutoSpace:east-asian-spacing-W-N", reversed_result.decisions[0].reason);
    let space_after = vec![cluster("中", 0, EM, "cjk"), cluster("ref", 1, EM, "latin"), cluster(" ", 4, EM, "latin")];
    assert!(apply_auto_space_policy(&space_after, &[edges(EastAsianSpacingValue::Wide, EastAsianSpacingValue::Wide), edges(EastAsianSpacingValue::Other, EastAsianSpacingValue::Other), edges(EastAsianSpacingValue::Other, EastAsianSpacingValue::Other)], &attachments, AutoSpacePolicy::default(), EM, &HashSet::new(), &HashSet::new()).decisions.is_empty());
    let break_after = vec![cluster("中", 0, EM, "cjk"), cluster("ref", 1, EM, "latin"), display_cluster("\n", "", 4, 0.0, "mandatory-break")];
    assert!(apply_auto_space_policy(&break_after, &[edges(EastAsianSpacingValue::Wide, EastAsianSpacingValue::Wide), edges(EastAsianSpacingValue::Other, EastAsianSpacingValue::Other), edges(EastAsianSpacingValue::Other, EastAsianSpacingValue::Other)], &attachments, AutoSpacePolicy::default(), EM, &HashSet::new(), &HashSet::new()).decisions.is_empty());
    let cjk_after = vec![cluster("中", 0, EM, "cjk"), cluster("ref", 1, EM, "latin"), cluster("中", 4, EM, "cjk")];
    assert!(apply_auto_space_policy(&cjk_after, &[edges(EastAsianSpacingValue::Wide, EastAsianSpacingValue::Wide), edges(EastAsianSpacingValue::Other, EastAsianSpacingValue::Other), edges(EastAsianSpacingValue::Wide, EastAsianSpacingValue::Wide)], &attachments, AutoSpacePolicy::default(), EM, &HashSet::new(), &HashSet::new()).decisions.is_empty());
}

#[test]
fn spacing_boundaries_count_each_wide_narrow_gap_once() {
    let wn = [cluster("中", 0, EM, "cjk"), cluster("a", 1, EM, "latin")];
    assert!(is_east_asian_spacing_boundary_at(1, &wn, &[edges(EastAsianSpacingValue::Wide, EastAsianSpacingValue::Wide), edges(EastAsianSpacingValue::Narrow, EastAsianSpacingValue::Narrow)]));
    let nw = [cluster("a", 0, EM, "latin"), cluster("中", 1, EM, "cjk")];
    assert!(is_east_asian_spacing_boundary_at(1, &nw, &[edges(EastAsianSpacingValue::Narrow, EastAsianSpacingValue::Narrow), edges(EastAsianSpacingValue::Wide, EastAsianSpacingValue::Wide)]));
    let right_space = [cluster("中", 0, EM, "cjk"), cluster(" ", 1, EM, "latin"), cluster("a", 2, EM, "latin")];
    assert!(is_east_asian_spacing_boundary_at(1, &right_space, &[edges(EastAsianSpacingValue::Wide, EastAsianSpacingValue::Wide), edges(EastAsianSpacingValue::Other, EastAsianSpacingValue::Other), edges(EastAsianSpacingValue::Narrow, EastAsianSpacingValue::Narrow)]));
    let left_space = [cluster("a", 0, EM, "latin"), cluster(" ", 1, EM, "latin"), cluster("中", 2, EM, "cjk")];
    assert!(is_east_asian_spacing_boundary_at(2, &left_space, &[edges(EastAsianSpacingValue::Narrow, EastAsianSpacingValue::Narrow), edges(EastAsianSpacingValue::Other, EastAsianSpacingValue::Other), edges(EastAsianSpacingValue::Wide, EastAsianSpacingValue::Wide)]));
    let cjk = [cluster("中", 0, EM, "cjk"), cluster("中", 1, EM, "cjk")];
    assert!(!is_east_asian_spacing_boundary_at(1, &cjk, &[edges(EastAsianSpacingValue::Wide, EastAsianSpacingValue::Wide), edges(EastAsianSpacingValue::Wide, EastAsianSpacingValue::Wide)]));
}

#[test]
fn attached_ascii_point_marks_need_a_contiguous_non_space_base() {
    assert!(is_attached_ascii_point_mark_at(&[cluster("中", 0, EM, "cjk"), cluster(",", 1, EM, "latin")], 1));
    assert!(!is_attached_ascii_point_mark_at(&[cluster("中", 0, EM, "cjk"), cluster(",", 1, EM, "latin")], 0));
    assert!(!is_attached_ascii_point_mark_at(&[cluster("中", 0, EM, "cjk"), cluster("", 1, EM, "latin")], 1));
    assert!(!is_attached_ascii_point_mark_at(&[cluster("中", 0, EM, "cjk"), cluster("a", 1, EM, "latin")], 1));
    assert!(!is_attached_ascii_point_mark_at(&[cluster("中", 0, EM, "cjk"), cluster(" ", 1, EM, "latin"), cluster(",", 2, EM, "latin")], 2));
    assert!(!is_attached_ascii_point_mark_at(&[cluster("中", 0, EM, "cjk"), cluster(",", 2, EM, "latin")], 1));
}

#[test]
fn inline_box_spans_add_structural_edges_and_skip_degenerate_ranges() {
    let clusters = vec![cluster("a", 0, 8.0, "latin"), cluster("b", 1, 8.0, "latin"), cluster("c", 2, 8.0, "latin")];
    assert_eq!(clusters, apply_inline_box_spans(&clusters, &[]).clusters);
    assert!(apply_inline_box_spans(&[], &[InlineBoxSpan::with_edges(TextRange::new(0, 1), 2.0, 0.0)]).clusters.is_empty());
    let skipped = apply_inline_box_spans(&clusters, &[InlineBoxSpan::with_edges(TextRange::new(2, 2), 4.0, 0.0), InlineBoxSpan::with_edges(TextRange::new(10, 11), 4.0, 0.0)]);
    assert!(skipped.decisions.is_empty());
    assert!(skipped.advance_by_cluster.is_empty());
    let applied = apply_inline_box_spans(&clusters, &[
        InlineBoxSpan::with_edges(TextRange::new(0, 1), 2.0, 0.0),
        InlineBoxSpan::with_edges(TextRange::new(1, 2), 0.0, 3.0),
        InlineBoxSpan::with_edges(TextRange::new(0, 2), 0.0, 1.5),
    ]);
    assert_eq!(3, applied.decisions.len());
    assert_eq!(2.0, applied.advance_by_cluster[&0]);
    assert_eq!(4.5, applied.advance_by_cluster[&1]);
    assert_eq!(10.0, applied.clusters[0].advance);
    assert_eq!(2.0, applied.clusters[0].leading_layout_advance);
    assert_eq!(12.5, applied.clusters[1].advance);
    assert_eq!(8.0, applied.clusters[2].advance);
    assert_eq!(0.0, applied.clusters[2].leading_layout_advance);
    let clamped = apply_inline_box_spans(&[cluster("a", 0, 2.0, "latin")], &[InlineBoxSpan::with_edges(TextRange::new(0, 1), 0.0, -6.0)]);
    assert_eq!(0.0, clamped.clusters[0].advance);
}
