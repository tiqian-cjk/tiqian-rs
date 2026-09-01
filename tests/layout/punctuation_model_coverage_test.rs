use tiqian::clreq::clreq_profile::{
    ClreqProfile, ClreqProfileResolver, GlueSide, InteriorPunctuationStyle, PunctuationClass, PunctuationGluePlacement,
    PunctuationWidthPolicy,
};
use tiqian::core::geometry::{Rect, TextRange};
use tiqian::core::text::Text;
use tiqian::layout::punctuation_model::{
    AdjustmentOpportunity, Glue, GlueKind, PunctuationAnchor, PunctuationAtom,
    PunctuationAtomBuilder, PunctuationInkInput, PunctuationSpacingAdjustment,
    PunctuationSpacingCompressionResult, PunctuationSpacingCompressor,
};
use tiqian::layout::paragraph_layout_engine::ParagraphLayoutEngine;

const EM: f32 = 16.0;

struct KaimingProfile {
    width: PunctuationWidthPolicy,
}

impl ClreqProfileResolver for KaimingProfile {
    fn resolve(&self, _: &tiqian::core::text_model::LayoutProfileId) -> ClreqProfile {
        let mut profile = ClreqProfile::mainland_horizontal();
        profile.punctuation_width = self.width;
        profile
    }
}

fn atom(character: char, index: i32, ink: Option<PunctuationInkInput>) -> PunctuationAtom {
    PunctuationAtomBuilder::default()
        .build(
            character,
            TextRange::new(index, index + 1),
            EM,
            ink,
            PunctuationGluePlacement::MainlandSimplified,
            PunctuationWidthPolicy::default(),
        )
        .unwrap()
}

fn punctuation_glue(natural: f32) -> Glue {
    Glue::new(
        GlueKind::PunctuationTrailing,
        0.0,
        natural,
        natural,
        0,
        0,
    )
}

fn advance_of_mid_line_punctuation(text: &str, punctuation: &str, width: PunctuationWidthPolicy) -> f32 {
    let mut engine = tiqian::layout::paragraph_layout_engine::ExplainableStubParagraphLayoutEngine::default();
    engine.clreq_profile_resolver = Box::new(KaimingProfile { width });
    let result = engine.layout(
        tiqian::core::text_model::LayoutInput::builder(
            tiqian::core::text_model::TiqianTextContent::new(Text::from(text)),
            tiqian::core::geometry::LayoutConstraints::with_defaults(320.0),
        )
        .paragraph_style(
            tiqian::core::text_model::ParagraphStyle::builder()
                .first_line_indent(Some(tiqian::core::units::Ic::ZERO))
                .build(),
        )
        .build(),
    );
    result
        .clusters
        .iter()
        .find(|cluster| cluster.text == punctuation)
        .unwrap()
        .advance
}

#[test]
#[should_panic(expected = "Glue min must not exceed natural.")]
fn glue_rejects_minimum_above_natural() {
    let _ = Glue::new(GlueKind::PunctuationTrailing, 2.0, 1.0, 3.0, 0, 0);
}

#[test]
#[should_panic(expected = "Glue natural must not exceed max.")]
fn glue_rejects_natural_above_maximum() {
    let _ = Glue::new(GlueKind::PunctuationTrailing, 0.0, 3.0, 1.0, 0, 0);
}

#[test]
fn adjustment_opportunity_carries_range_and_glue() {
    let opportunity = AdjustmentOpportunity {
        range: TextRange::new(1, 2),
        glue: punctuation_glue(4.0),
    };
    assert_eq!(TextRange::new(1, 2), opportunity.range);
    assert_eq!(4.0, opportunity.glue.natural);
}

#[test]
fn compression_result_sums_adjustment_reductions() {
    let result = PunctuationSpacingCompressionResult::new(vec![
        PunctuationSpacingAdjustment {
            range: TextRange::new(0, 2),
            reduction_target_range: TextRange::new(0, 1),
            left_char: '。',
            right_char: '「',
            natural_inner_glue: 16.0,
            adjusted_inner_glue: 8.0,
            reduction: 8.0,
            reason: "test-a".to_owned(),
        },
        PunctuationSpacingAdjustment {
            range: TextRange::new(2, 4),
            reduction_target_range: TextRange::new(2, 3),
            left_char: '，',
            right_char: '「',
            natural_inner_glue: 8.0,
            adjusted_inner_glue: 4.0,
            reduction: 4.0,
            reason: "test-b".to_owned(),
        },
    ]);
    assert_eq!(12.0, result.total_reduction());
    assert_eq!(0.0, PunctuationSpacingCompressionResult::new(Vec::new()).total_reduction());
}

#[test]
fn adjacent_punctuation_inner_glue_collapses_by_half_em() {
    let stop = atom('。', 0, None);
    let opening = atom('「', 1, None);
    let adjustment = PunctuationSpacingCompressor
        .compress(&[stop.clone(), opening.clone()], EM)
        .adjustments
        .pop()
        .unwrap();
    assert_eq!(16.0, adjustment.natural_inner_glue);
    assert_eq!(8.0, adjustment.adjusted_inner_glue);
    assert_eq!(8.0, adjustment.reduction);
    assert_eq!(stop.range, adjustment.reduction_target_range);
    assert_eq!('。', adjustment.left_char);
    assert_eq!('「', adjustment.right_char);
    assert_eq!("collapse-adjacent-punctuation-inner-glue", adjustment.reason);
    assert_eq!(TextRange::new(0, 2), adjustment.range);
}

#[test]
fn adjacent_punctuation_targets_the_wider_side() {
    let mut stop = atom('。', 0, None);
    stop.trailing_glue_initially_consumed = 8.0;
    let opening = atom('「', 1, None);
    let adjustment = PunctuationSpacingCompressor
        .compress(&[stop, opening.clone()], EM)
        .adjustments
        .pop()
        .unwrap();
    assert_eq!(opening.range, adjustment.reduction_target_range);
}

#[test]
fn adjacent_punctuation_skips_non_adjacent_zero_glue_and_zero_em() {
    let stop = atom('。', 0, None);
    let opening = atom('「', 2, None);
    assert!(PunctuationSpacingCompressor
        .compress(&[stop.clone(), opening], EM)
        .adjustments
        .is_empty());
    let mut consumed_stop = stop.clone();
    consumed_stop.trailing_glue_initially_consumed = 8.0;
    let mut consumed_opening = atom('「', 1, None);
    consumed_opening.leading_glue_initially_consumed = 8.0;
    assert!(PunctuationSpacingCompressor
        .compress(&[consumed_stop, consumed_opening], EM)
        .adjustments
        .is_empty());
    assert!(PunctuationSpacingCompressor.compress(&[stop.clone()], EM).adjustments.is_empty());
    assert!(PunctuationSpacingCompressor
        .compress(&[stop, atom('「', 1, None)], 0.0)
        .adjustments
        .is_empty());
}

#[test]
fn cjk_closing_before_ascii_point_mark_collapses_trailing_glue() {
    let closing = atom('」', 0, None);
    let adjustment = PunctuationSpacingCompressor
        .compress_cjk_closing_before_ascii_point_mark(&[closing.clone()], &Text::from("」, rest"), EM)
        .adjustments
        .pop()
        .unwrap();
    assert_eq!(TextRange::new(0, 2), adjustment.range);
    assert_eq!(closing.range, adjustment.reduction_target_range);
    assert_eq!(8.0, adjustment.natural_inner_glue);
    assert_eq!(0.0, adjustment.adjusted_inner_glue);
    assert_eq!('」', adjustment.left_char);
    assert_eq!(',', adjustment.right_char);
    assert_eq!("collapse-cjk-closing-before-ascii-point-mark", adjustment.reason);
}

#[test]
fn cjk_closing_compression_rejects_non_matching_neighbours() {
    let opening = atom('「', 0, None);
    assert!(PunctuationSpacingCompressor
        .compress_cjk_closing_before_ascii_point_mark(&[opening], &Text::from("「, x"), EM)
        .adjustments
        .is_empty());
    let closing = atom('」', 0, None);
    for text in ["」", "」中"] {
        assert!(PunctuationSpacingCompressor
            .compress_cjk_closing_before_ascii_point_mark(&[closing.clone()], &Text::from(text), EM)
            .adjustments
            .is_empty());
    }
    let mut consumed = closing.clone();
    consumed.trailing_glue_initially_consumed = 8.0;
    assert!(PunctuationSpacingCompressor
        .compress_cjk_closing_before_ascii_point_mark(&[consumed], &Text::from("」,x"), EM)
        .adjustments
        .is_empty());
    assert!(PunctuationSpacingCompressor
        .compress_cjk_closing_before_ascii_point_mark(&[closing], &Text::from("」,x"), 0.0)
        .adjustments
        .is_empty());
}

#[test]
fn indexed_build_rejects_out_of_range_index() {
    let builder = PunctuationAtomBuilder::default();
    assert_eq!(None, builder.build_at(&Text::from("，"), 5, EM));
    let built = builder.build_at(&Text::from("，"), 0, EM).unwrap();
    assert_eq!(TextRange::new(0, 1), built.range);
    assert_eq!('，', built.character);
}

#[test]
fn non_punctuation_characters_produce_no_atom() {
    assert_eq!(None, PunctuationAtomBuilder::default().build_at(&Text::from("中"), 0, EM));
    assert_eq!(None, PunctuationAtomBuilder::default().build_at(&Text::from("a"), 0, EM));
}

#[test]
fn builds_two_em_punctuation_atom_for_recommended_dash_codepoint() {
    let dash = atom('⸺', 0, None);

    assert_eq!(32.0, dash.advance);
    assert_eq!(32.0, dash.body_width);
}

#[test]
fn ink_bounds_determine_compression_amount_and_sides() {
    let comma = atom(
        '，',
        0,
        Some(
            PunctuationInkInput::builder(16.0)
                .ink_bounds(Some(Rect {
                    left: 9.0,
                    top: -2.0,
                    right: 11.0,
                    bottom: 2.0,
                }))
                .build(),
        ),
    );

    assert_eq!(16.0, comma.advance);
    assert_eq!(8.0, comma.body_width);
    assert_eq!(Some(8.0), comma.ink_containment_body_floor);
    assert!(!comma.ink_containment_applied);
    assert_eq!(4.0, comma.leading_glue.natural);
    assert_eq!(4.0, comma.trailing_glue.natural);
    assert_eq!("InkBoundsFittedBodyCompression", comma.geometry_source);
    assert_eq!(Some(2.0), comma.ink_width);
    assert_eq!(Some(10.0), comma.ink_center);
}

#[test]
fn short_hyphen_connector_is_half_width_wavy_tilde_full_width() {
    let hyphen = atom('–', 0, None);
    let tilde = atom('～', 0, None);

    assert_eq!(8.0, hyphen.advance);
    assert_eq!(16.0, tilde.advance);
}

#[test]
fn kaiming_style_halves_interior_punctuation_but_not_sentence_end() {
    let full = PunctuationWidthPolicy::default();
    let kaiming = PunctuationWidthPolicy::with_interior(InteriorPunctuationStyle::Kaiming);

    assert_eq!(16.0, advance_of_mid_line_punctuation("中，中文", "，", full));
    assert_eq!(8.0, advance_of_mid_line_punctuation("中，中文", "，", kaiming));
    assert_eq!(8.0, advance_of_mid_line_punctuation("中（中）文", "（", kaiming));
    assert_eq!(16.0, advance_of_mid_line_punctuation("中。中文", "。", kaiming));
}

#[test]
fn policy_fallback_splits_glue_by_class_side() {
    let stop = atom('，', 0, None);
    assert_eq!(0.0, stop.leading_glue.natural);
    assert_eq!(8.0, stop.trailing_glue.natural);
    assert_eq!(PunctuationAnchor::Leading, stop.anchor);
    assert_eq!(8.0, stop.body_width);
    assert_eq!("ProfileGlueFallbackWithoutFontGeometry", stop.geometry_source);
    assert_eq!(None, stop.halt_advance);
    assert_eq!(None, stop.ink_containment_body_floor);
    assert!(!stop.ink_containment_applied);
    assert_eq!(None, stop.ink_bounds_fallback);
    assert_eq!(None, stop.halt_validation);
    let opening = atom('「', 0, None);
    assert_eq!(8.0, opening.leading_glue.natural);
    assert_eq!(0.0, opening.trailing_glue.natural);
    assert_eq!(PunctuationAnchor::Trailing, opening.anchor);
    let traditional = PunctuationAtomBuilder::new(
        PunctuationGluePlacement::Traditional,
        PunctuationWidthPolicy::default(),
    )
    .build(
        '，', TextRange::new(0, 1), EM, None, PunctuationGluePlacement::Traditional,
        PunctuationWidthPolicy::default(),
    )
    .unwrap();
    assert_eq!(4.0, traditional.leading_glue.natural);
    assert_eq!(4.0, traditional.trailing_glue.natural);
    assert_eq!(PunctuationAnchor::Center, traditional.anchor);
}

#[test]
fn underwidth_glyphs_expand_into_full_width_cell_by_class_side() {
    let opening = atom('「', 0, Some(PunctuationInkInput::new(8.0)));
    assert_eq!(8.0, opening.glyph_inline_shift);
    assert_eq!(Some("UnderwidthPunctuationFullWidthBoxPlacement".to_owned()), opening.glyph_placement_reason);
    assert_eq!(8.0, opening.advance_expansion);
    assert_eq!(16.0, opening.advance);
    let dot = atom('·', 0, Some(PunctuationInkInput::new(8.0)));
    assert_eq!(4.0, dot.glyph_inline_shift);
    assert_eq!(8.0, dot.advance_expansion);
    let closing = atom('」', 0, Some(PunctuationInkInput::new(8.0)));
    assert_eq!(0.0, closing.glyph_inline_shift);
    assert_eq!(None, closing.glyph_placement_reason);
    assert_eq!(8.0, closing.advance_expansion);
    let exact = atom('「', 0, Some(PunctuationInkInput::new(16.0)));
    assert_eq!(0.0, exact.glyph_inline_shift);
    assert_eq!(None, exact.glyph_placement_reason);
    assert_eq!(0.0, exact.advance_expansion);
}

#[test]
fn halt_fitted_compression_uses_font_measurements() {
    let result = atom('·', 0, Some(PunctuationInkInput::builder(16.0)
        .ink_bounds(Some(Rect { left: 2.0, top: 4.0, right: 10.0, bottom: 12.0 }))
        .halt_advance(Some(8.0)).halt_placement_x(Some(-2.0)).build()));
    assert_eq!("FontHaltFittedBodyCompression", result.geometry_source);
    assert_eq!(2.0, result.leading_glue.natural);
    assert_eq!(6.0, result.trailing_glue.natural);
    assert_eq!(8.0, result.body_width);
    assert_eq!(PunctuationAnchor::Center, result.anchor);
    assert_eq!(Some(8.0), result.halt_advance);
    assert_eq!(None, result.halt_validation);
    assert!(!result.ink_containment_applied);
    assert_eq!(Some(8.0), result.ink_containment_body_floor);
}

#[test]
fn halt_trim_is_limited_by_ink_bounds_and_records_why() {
    let result = atom('·', 0, Some(PunctuationInkInput::builder(16.0)
        .ink_bounds(Some(Rect { left: 2.0, top: 4.0, right: 14.0, bottom: 12.0 }))
        .halt_advance(Some(8.0)).halt_placement_x(Some(-2.0)).build()));
    assert_eq!(2.0, result.leading_glue.natural);
    assert_eq!(2.0, result.trailing_glue.natural);
    assert!(result.ink_containment_applied);
    assert_eq!(Some("halt-trim-limited-by-default-ink-bounds".to_owned()), result.halt_validation);
}

#[test]
fn halt_advance_without_placement_falls_back_to_fitted_ink_or_profile() {
    let with_ink = atom('·', 0, Some(PunctuationInkInput::builder(16.0)
        .ink_bounds(Some(Rect { left: 8.0, top: 4.0, right: 16.0, bottom: 12.0 }))
        .halt_advance(Some(8.0)).build()));
    assert_eq!("FontHaltAdvanceWithInkBoundsFittedPlacement", with_ink.geometry_source);
    let without_ink = atom('，', 0, Some(PunctuationInkInput::builder(16.0).halt_advance(Some(8.0)).build()));
    assert_eq!("FontHaltAdvanceWithProfileFallback", without_ink.geometry_source);
    assert_eq!(0.0, without_ink.leading_glue.natural);
    assert_eq!(8.0, without_ink.trailing_glue.natural);
}

#[test]
fn halt_from_proportional_glyph_is_rejected() {
    let result = atom('「', 0, Some(PunctuationInkInput::builder(8.0)
        .halt_advance(Some(4.0)).halt_placement_x(Some(-2.0)).build()));
    assert_eq!(None, result.halt_advance);
    assert_eq!(8.0, result.glyph_inline_shift);
    assert_eq!(Some("UnderwidthPunctuationFullWidthBoxPlacement".to_owned()), result.glyph_placement_reason);
}

#[test]
fn ink_bounds_fitted_frame_picks_the_narrowest_containing_anchor() {
    let right = atom('」', 0, Some(PunctuationInkInput::builder(16.0)
        .ink_bounds(Some(Rect { left: 8.0, top: 4.0, right: 16.0, bottom: 12.0 })).build()));
    assert_eq!(PunctuationAnchor::Trailing, right.anchor);
    assert_eq!(8.0, right.leading_glue.natural);
    assert_eq!(0.0, right.trailing_glue.natural);
    assert_eq!("InkBoundsFittedBodyCompression", right.geometry_source);
    assert!(!right.ink_containment_applied);
    let left = atom('「', 0, Some(PunctuationInkInput::builder(16.0)
        .ink_bounds(Some(Rect { left: 0.0, top: 4.0, right: 8.0, bottom: 12.0 })).build()));
    assert_eq!(PunctuationAnchor::Leading, left.anchor);
    assert_eq!(0.0, left.leading_glue.natural);
    assert_eq!(8.0, left.trailing_glue.natural);
    let wide = atom('」', 0, Some(PunctuationInkInput::builder(16.0)
        .ink_bounds(Some(Rect { left: 1.0, top: 4.0, right: 15.0, bottom: 12.0 })).build()));
    assert!(wide.ink_containment_applied);
    assert_eq!(Some(14.0), wide.ink_containment_body_floor);
}

#[test]
fn forced_half_width_connectors_consume_glue_up_front() {
    let hyphen = atom('-', 0, None);
    assert_eq!(8.0, hyphen.advance);
    assert_eq!(8.0, hyphen.body_width);
    assert!(hyphen.geometry_source.ends_with("FixedHalfWidth"));
    assert_eq!(0.0, hyphen.leading_glue_initially_consumed);
    assert_eq!(0.0, hyphen.trailing_glue_initially_consumed);
    let gb = PunctuationWidthPolicy::with_gb_fixed_separators(true);
    let dot = PunctuationAtomBuilder::new(PunctuationGluePlacement::MainlandSimplified, gb)
        .build('·', TextRange::new(0, 1), EM, None, PunctuationGluePlacement::MainlandSimplified, gb)
        .unwrap();
    assert!(dot.geometry_source.ends_with("FixedHalfWidth"));
    assert_eq!(4.0, dot.leading_glue_initially_consumed);
    assert_eq!(4.0, dot.trailing_glue_initially_consumed);
    let kaiming = PunctuationWidthPolicy::with_interior(InteriorPunctuationStyle::Kaiming);
    let comma = PunctuationAtomBuilder::new(PunctuationGluePlacement::MainlandSimplified, kaiming)
        .build('，', TextRange::new(0, 1), EM, None, PunctuationGluePlacement::MainlandSimplified, kaiming)
        .unwrap();
    assert!(comma.geometry_source.ends_with("FixedHalfWidth"));
    assert_eq!(8.0, comma.trailing_glue_initially_consumed);
    let stop = PunctuationAtomBuilder::new(PunctuationGluePlacement::MainlandSimplified, kaiming)
        .build('。', TextRange::new(0, 1), EM, None, PunctuationGluePlacement::MainlandSimplified, kaiming)
        .unwrap();
    assert!(!stop.geometry_source.ends_with("FixedHalfWidth"));
}

#[test]
fn ink_input_records_why_bounds_are_missing() {
    let no_ink = atom('，', 0, Some(PunctuationInkInput::builder(16.0)
        .bounds_fallback_reason(Some("shaper-no-ink-bounds".to_owned())).build()));
    assert_eq!(Some("shaper-no-ink-bounds".to_owned()), no_ink.ink_bounds_fallback);
    assert_eq!(None, no_ink.ink_bounds);
    let ambiguous = atom('，', 0, Some(PunctuationInkInput::builder(0.0)
        .bounds_fallback_reason(Some("glyph-cluster-mapping-ambiguous".to_owned())).build()));
    assert_eq!(Some("glyph-cluster-mapping-ambiguous".to_owned()), ambiguous.ink_bounds_fallback);
    assert_eq!(16.0, ambiguous.advance);
}

#[test]
fn glue_side_for_mainland_simplified_maps_classes_to_sides() {
    assert_eq!(GlueSide::LeadingOnly, PunctuationGluePlacement::MainlandSimplified.glue_side_for(PunctuationClass::Opening));
    assert_eq!(GlueSide::TrailingOnly, PunctuationGluePlacement::MainlandSimplified.glue_side_for(PunctuationClass::Closing));
    assert_eq!(GlueSide::TrailingOnly, PunctuationGluePlacement::MainlandSimplified.glue_side_for(PunctuationClass::PauseOrStop));
    assert_eq!(GlueSide::BothSides, PunctuationGluePlacement::MainlandSimplified.glue_side_for(PunctuationClass::MiddleDot));
    assert_eq!(GlueSide::BothSides, PunctuationGluePlacement::Traditional.glue_side_for(PunctuationClass::Opening));
}