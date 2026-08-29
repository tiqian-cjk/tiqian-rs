use tiqian::clreq::clreq_profile::{
    InteriorPunctuationStyle, PunctuationGluePlacement, PunctuationWidthPolicy,
};
use tiqian::core::geometry::{Rect, TextRange};
use tiqian::layout::punctuation_model::{
    PunctuationAnchor, PunctuationAtomBuilder, PunctuationInkInput,
};

const EM: f32 = 16.0;

fn atom(
    character: char,
    ink: PunctuationInkInput,
    placement: PunctuationGluePlacement,
    width: PunctuationWidthPolicy,
) -> tiqian::layout::punctuation_model::PunctuationAtom {
    PunctuationAtomBuilder::new(placement, width)
        .build(
            character,
            TextRange::new(0, 1),
            EM,
            Some(ink),
            placement,
            width,
        )
        .unwrap()
}

#[test]
fn halt_advance_without_placement_uses_named_profile_fallback() {
    let result = atom(
        '。',
        PunctuationInkInput::builder(16.0)
            .halt_advance(Some(7.5))
            .build(),
        PunctuationGluePlacement::MainlandSimplified,
        PunctuationWidthPolicy::default(),
    );

    assert_eq!(7.5, result.body_width);
    assert_eq!(Some(7.5), result.halt_advance);
    assert_eq!(0.0, result.leading_glue.natural);
    assert_eq!(8.5, result.trailing_glue.natural);
    assert_eq!("FontHaltAdvanceWithProfileFallback", result.geometry_source);
}

#[test]
fn halt_placement_defines_both_compression_sides() {
    let result = atom(
        '（',
        PunctuationInkInput::builder(16.0)
            .ink_bounds(Some(Rect {
                left: 5.0,
                top: -12.0,
                right: 11.0,
                bottom: 2.0,
            }))
            .halt_advance(Some(8.0))
            .halt_placement_x(Some(-4.0))
            .build(),
        PunctuationGluePlacement::MainlandSimplified,
        PunctuationWidthPolicy::default(),
    );

    assert_eq!(4.0, result.leading_glue.natural);
    assert_eq!(4.0, result.trailing_glue.natural);
    assert_eq!(8.0, result.body_width);
    assert_eq!(PunctuationAnchor::Center, result.anchor);
    assert_eq!("FontHaltFittedBodyCompression", result.geometry_source);
    assert_eq!(None, result.halt_validation);
}

#[test]
fn ink_bounds_caps_halt_trim_that_would_cut_painted_glyph() {
    let result = atom(
        '（',
        PunctuationInkInput::builder(16.0)
            .ink_bounds(Some(Rect {
                left: 2.0,
                top: -12.0,
                right: 15.0,
                bottom: 2.0,
            }))
            .halt_advance(Some(8.0))
            .halt_placement_x(Some(-8.0))
            .build(),
        PunctuationGluePlacement::MainlandSimplified,
        PunctuationWidthPolicy::default(),
    );

    assert_eq!(2.0, result.leading_glue.natural);
    assert_eq!(0.0, result.trailing_glue.natural);
    assert_eq!(14.0, result.body_width);
    assert_eq!(
        Some("halt-trim-limited-by-default-ink-bounds".to_owned()),
        result.halt_validation
    );
    assert!(result.ink_containment_applied);
}

#[test]
fn underwidth_opening_quote_is_placed_in_full_width_cell() {
    let result = atom(
        '“',
        PunctuationInkInput::builder(6.0)
            .ink_bounds(Some(Rect {
                left: 1.0,
                top: -10.0,
                right: 5.0,
                bottom: 0.0,
            }))
            .build(),
        PunctuationGluePlacement::MainlandSimplified,
        PunctuationWidthPolicy::default(),
    );

    assert_eq!(16.0, result.advance);
    assert_eq!(10.0, result.advance_expansion);
    assert_eq!(8.0, result.body_width);
    assert_eq!(8.0, result.leading_glue.natural);
    assert_eq!(0.0, result.trailing_glue.natural);
    assert_eq!(10.0, result.glyph_inline_shift);
    assert_eq!(
        Some("UnderwidthPunctuationFullWidthBoxPlacement".to_owned()),
        result.glyph_placement_reason
    );
}

#[test]
fn fixed_half_width_consumes_measured_sidebearing() {
    let result = atom(
        '《',
        PunctuationInkInput::builder(16.0)
            .ink_bounds(Some(Rect {
                left: 6.5,
                top: -12.0,
                right: 15.5,
                bottom: 2.0,
            }))
            .build(),
        PunctuationGluePlacement::MainlandSimplified,
        PunctuationWidthPolicy::with_interior(InteriorPunctuationStyle::Kaiming),
    );

    assert_eq!(16.0, result.advance);
    assert_eq!(9.5, result.body_width);
    assert_eq!(6.5, result.leading_glue_initially_consumed);
    assert_eq!(0.0, result.trailing_glue_initially_consumed);
    assert_eq!(0.0, result.glyph_inline_shift);
    assert_eq!(
        "InkBoundsFittedBodyCompressionFixedHalfWidth",
        result.geometry_source
    );
}
