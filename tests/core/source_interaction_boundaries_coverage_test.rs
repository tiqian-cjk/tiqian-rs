use tiqian::core::geometry::TextRange;
use tiqian::core::source_interaction_boundaries::{
    SourceBoundaryBias, coerce_to_interaction_boundary, interaction_boundaries,
    source_grapheme_boundaries,
};
use tiqian::core::text::Text;

fn boundaries(text: &str) -> Vec<i32> {
    let text = Text::from(text);
    interaction_boundaries(&text, TextRange::new(0, text.utf16_len()))
}

#[test]
fn crlf_stays_one_unit() {
    assert_eq!(vec![0, 2], boundaries("\r\n"));
    assert_eq!(vec![0, 1], boundaries("\r"));
    assert_eq!(vec![0, 1, 2], boundaries("a\n"));
}

#[test]
fn regional_indicators_pair_up() {
    assert_eq!(vec![0, 4], boundaries("🇦🇨"));
    assert_eq!(vec![0, 4, 6], boundaries("🇦🇦🇦"));
    assert_eq!(vec![0, 2, 3], boundaries("🇦A"));
    assert_eq!(vec![0, 2], boundaries("🇦"));
}

#[test]
fn hangul_jamo_runs_merge_into_syllable_blocks() {
    assert_eq!(vec![0, 3], boundaries("ᄀ가"));
    assert_eq!(vec![0, 3], boundaries("각"));
    assert_eq!(vec![0, 4], boundaries("각ᆨ"));
    assert_eq!(vec![0, 1, 2], boundaries("ᄀA"));
    assert_eq!(vec![0, 2, 3], boundaries("가A"));
    assert_eq!(vec![0, 2], boundaries("ꥠᅡ"));
    assert_eq!(vec![0, 2], boundaries("ᄀힰ"));
    assert_eq!(vec![0, 3], boundaries("가ퟋ"));
}

#[test]
fn precomposed_hangul_syllables_absorb_jamo() {
    assert_eq!(vec![0, 3], boundaries("가ᅡᆨ"));
    assert_eq!(vec![0, 2], boundaries("각ᆨ"));
    assert_eq!(vec![0, 1, 2], boundaries("각A"));
    assert_eq!(vec![0, 2], boundaries("각"));
}

#[test]
fn extenders_attach_to_the_preceding_unit() {
    assert_eq!(vec![0, 2], boundaries("a\u{0301}"));
    assert_eq!(vec![0, 2], boundaries("a\u{FE0F}"));
    assert_eq!(vec![0, 3], boundaries("a\u{E0100}"));
    assert_eq!(
        vec![0, 12],
        boundaries("🏴\u{E0067}\u{E0062}\u{E0065}\u{E006E}\u{E0067}"),
    );
    assert_eq!(vec![0, 2], boundaries("가\u{200C}"));
    assert_eq!(vec![0, 1, 2], boundaries("aA"));
}

#[test]
fn band_edges_and_gaps_exercise_every_representable_range_arm() {
    assert_eq!(vec![0, 1, 2], boundaries("\rA"));
    assert_eq!(vec![0, 1, 2], boundaries("aᄀ"));
    assert_eq!(vec![0, 2, 3], boundaries("\u{1100}\u{1161}\u{E000}"));
    assert_eq!(vec![0, 1, 3], boundaries("a\u{E01F0}"));
    assert_eq!(vec![0, 1, 3], boundaries("a\u{E00A0}"));
    assert_eq!(vec![0, 2, 3], boundaries("👍甲"));
    assert_eq!(vec![0, 2, 4], boundaries("👍😀"));
}

#[test]
fn emoji_modifiers_only_attach_to_bases() {
    assert_eq!(vec![0, 4], boundaries("👍🏻"));
    assert_eq!(vec![0, 5], boundaries("👍🏻\u{FE0F}"));
    assert_eq!(vec![0, 1, 3], boundaries("a🏻"));
    assert_eq!(vec![0, 2], boundaries("👍"));
}

#[test]
fn zwj_chains_join_only_extended_pictographic() {
    assert_eq!(vec![0, 8], boundaries("👩‍👩‍👦"));
    assert_eq!(vec![0, 2], boundaries("a‍"));
    assert_eq!(vec![0, 3, 4], boundaries("👩‍a"));
    assert_eq!(vec![0, 2, 3], boundaries("a‍a"));
    assert_eq!(vec![0, 7], boundaries("👍‍👍🏻"));
}

#[test]
fn source_grapheme_boundaries_respect_the_requested_window() {
    let text = Text::from("abcd");
    assert_eq!(
        vec![1, 2, 3],
        interaction_boundaries(&text, TextRange::new(1, 3)),
    );

    let text = Text::from("ab");
    assert_eq!(
        vec![2],
        interaction_boundaries(&text, TextRange::new(5, 9)),
    );

    let text = Text::from("😀b");
    assert_eq!(
        vec![0, 2, 3],
        source_grapheme_boundaries(&text, TextRange::new(0, 3)),
    );
}

#[test]
fn coercion_honours_every_bias_and_edge_case() {
    let family = Text::from("👨‍👩‍👧‍👧");
    let family_range = TextRange::new(0, family.utf16_len());
    assert_eq!(11, family.utf16_len());
    assert_eq!(
        0,
        coerce_to_interaction_boundary(&family, 2, family_range, SourceBoundaryBias::Nearest),
    );
    assert_eq!(
        0,
        coerce_to_interaction_boundary(&family, 2, family_range, SourceBoundaryBias::Backward),
    );
    assert_eq!(
        family.utf16_len(),
        coerce_to_interaction_boundary(&family, 2, family_range, SourceBoundaryBias::Forward),
    );

    let text = Text::from("😀b");
    let range = TextRange::new(0, 3);
    assert_eq!(
        2,
        coerce_to_interaction_boundary(&text, 2, range, SourceBoundaryBias::Nearest),
    );
    assert_eq!(
        3,
        coerce_to_interaction_boundary(&text, 9, range, SourceBoundaryBias::Backward),
    );
    assert_eq!(
        0,
        coerce_to_interaction_boundary(&text, -1, range, SourceBoundaryBias::Forward),
    );
}

// Kotlin also verifies lone UTF-16 surrogate handling. Rust `Text` deliberately stores valid
// UTF-8, so such input cannot enter this API; valid supplementary scalars are covered above.
#[test]
fn valid_supplementary_scalars_are_single_interaction_units() {
    let text = Text::from("😀A");
    assert_eq!(vec![0, 2, 3], interaction_boundaries(&text, TextRange::new(0, 3)));
    assert_eq!(0x1F600, text.code_point_at_compat(0, 3));
}