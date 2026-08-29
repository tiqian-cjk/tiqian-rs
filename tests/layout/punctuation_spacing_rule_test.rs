use tiqian::clreq::clreq_profile::{PunctuationGluePlacement, PunctuationWidthPolicy};
use tiqian::core::geometry::TextRange;
use tiqian::core::text::Text;
use tiqian::layout::punctuation_model::{
    PunctuationAtom, PunctuationAtomBuilder, PunctuationSpacingCompressor,
};

const EM: f32 = 16.0;

fn atom(character: char, index: i32) -> PunctuationAtom {
    PunctuationAtomBuilder::new(
        PunctuationGluePlacement::MainlandSimplified,
        PunctuationWidthPolicy::default(),
    )
    .build(
        character,
        TextRange::new(index, index + 1),
        EM,
        None,
        PunctuationGluePlacement::MainlandSimplified,
        PunctuationWidthPolicy::default(),
    )
    .unwrap()
}

fn assert_compression(left: char, right: char, natural: f32, adjusted: f32) {
    let result = PunctuationSpacingCompressor.compress(&[atom(left, 0), atom(right, 1)], EM);
    assert_eq!(1, result.adjustments.len());
    let adjustment = &result.adjustments[0];
    assert_eq!(natural, adjustment.natural_inner_glue);
    assert_eq!(adjusted, adjustment.adjusted_inner_glue);
    assert_eq!(8.0, adjustment.reduction);
}

#[test]
fn closing_plus_closing_collapses_inner_gap_to_zero() {
    assert_compression('」', '。', 8.0, 0.0);
}

#[test]
fn opening_plus_opening_collapses_inner_gap_to_zero() {
    assert_compression('「', '（', 8.0, 0.0);
}

#[test]
fn closing_plus_opening_keeps_half_em_inner_gap() {
    assert_compression('。', '「', 16.0, 8.0);
}

#[test]
fn pause_stop_plus_opening_collapses_by_half_em() {
    assert_compression('，', '「', 16.0, 8.0);
}

#[test]
fn consecutive_pause_stop_and_closing_pause_stop_pairs_compress() {
    assert_compression('！', '！', 8.0, 0.0);
    assert_compression('”', '！', 8.0, 0.0);
}

#[test]
fn non_adjacent_atoms_are_not_compressed() {
    let result = PunctuationSpacingCompressor.compress(&[atom('，', 0), atom('。', 5)], EM);
    assert!(result.adjustments.is_empty());
}

#[test]
fn cjk_closing_before_ascii_point_mark_consumes_only_closing_glue() {
    let result = PunctuationSpacingCompressor.compress_cjk_closing_before_ascii_point_mark(
        &[atom('」', 0)],
        &Text::from("」,"),
        EM,
    );
    assert_eq!(1, result.adjustments.len());
    let adjustment = &result.adjustments[0];
    assert_eq!(8.0, adjustment.natural_inner_glue);
    assert_eq!(0.0, adjustment.adjusted_inner_glue);
    assert_eq!(8.0, adjustment.reduction);
    assert_eq!(TextRange::new(0, 1), adjustment.reduction_target_range);
    assert_eq!(
        "collapse-cjk-closing-before-ascii-point-mark",
        adjustment.reason
    );
}

#[test]
fn whitespace_blocks_closing_to_ascii_point_mark_compression() {
    let result = PunctuationSpacingCompressor.compress_cjk_closing_before_ascii_point_mark(
        &[atom('」', 0)],
        &Text::from("」 ,"),
        EM,
    );
    assert!(result.adjustments.is_empty());
}
