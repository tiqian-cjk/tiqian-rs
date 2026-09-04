use tiqian::core::geometry::{scalar_offset, text_range};
use tiqian::core::source_interaction_boundaries::{
    SourceBoundaryBias, coerce_to_interaction_boundary, interaction_boundaries,
    source_grapheme_boundaries,
};
use tiqian::core::text::Text;

fn boundaries(text: &str) -> Vec<i32> {
    let text = Text::from(text);
    interaction_boundaries(&text, text_range(0, text.scalar_len().value()))
        .into_iter()
        .map(|offset| offset.value())
        .collect()
}

#[test]
fn crlf_stays_one_unit() {
    assert_eq!(vec![0, 2], boundaries("\r\n"));
    assert_eq!(vec![0, 1], boundaries("\r"));
    assert_eq!(vec![0, 1, 2], boundaries("a\n"));
}

#[test]
fn regional_indicators_pair_up() {
    assert_eq!(vec![0, 2], boundaries("🇦🇨"));
    assert_eq!(vec![0, 2, 3], boundaries("🇦🇦🇦"));
    assert_eq!(vec![0, 1, 2], boundaries("🇦A"));
    assert_eq!(vec![0, 1], boundaries("🇦"));
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
    assert_eq!(vec![0, 2], boundaries("a\u{E0100}"));
    assert_eq!(
        vec![0, 6],
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
    assert_eq!(vec![0, 1, 2], boundaries("a\u{E01F0}"));
    assert_eq!(vec![0, 1, 2], boundaries("a\u{E00A0}"));
    assert_eq!(vec![0, 1, 2], boundaries("👍甲"));
    assert_eq!(vec![0, 1, 2], boundaries("👍😀"));
}

#[test]
fn emoji_modifiers_only_attach_to_bases() {
    assert_eq!(vec![0, 2], boundaries("👍🏻"));
    assert_eq!(vec![0, 3], boundaries("👍🏻\u{FE0F}"));
    assert_eq!(vec![0, 1, 2], boundaries("a🏻"));
    assert_eq!(vec![0, 1], boundaries("👍"));
}

#[test]
fn zwj_chains_join_only_extended_pictographic() {
    assert_eq!(vec![0, 5], boundaries("👩‍👩‍👦"));
    assert_eq!(vec![0, 2], boundaries("a‍"));
    assert_eq!(vec![0, 2, 3], boundaries("👩‍a"));
    assert_eq!(vec![0, 2, 3], boundaries("a‍a"));
    assert_eq!(vec![0, 4], boundaries("👍‍👍🏻"));
}

#[test]
fn source_grapheme_boundaries_respect_the_requested_window() {
    let text = Text::from("abcd");
    assert_eq!(
        vec![1, 2, 3],
        interaction_boundaries(&text, text_range(1, 3))
            .into_iter()
            .map(|offset| offset.value())
            .collect::<Vec<_>>(),
    );

    let text = Text::from("ab");
    assert_eq!(
        vec![2],
        interaction_boundaries(&text, text_range(5, 9))
            .into_iter()
            .map(|offset| offset.value())
            .collect::<Vec<_>>(),
    );

    let text = Text::from("😀b");
    assert_eq!(
        vec![0, 1, 2],
        source_grapheme_boundaries(&text, text_range(0, 2))
            .into_iter()
            .map(|offset| offset.value())
            .collect::<Vec<_>>(),
    );
}

#[test]
fn coercion_honours_every_bias_and_edge_case() {
    let family = Text::from("👨‍👩‍👧‍👧");
    let family_range = text_range(0, family.scalar_len().value());
    assert_eq!(family.scalar_len(), scalar_offset(7));
    assert_eq!(
        scalar_offset(0),
        coerce_to_interaction_boundary(&family, scalar_offset(2), family_range, SourceBoundaryBias::Nearest),
    );
    assert_eq!(
        scalar_offset(0),
        coerce_to_interaction_boundary(&family, scalar_offset(2), family_range, SourceBoundaryBias::Backward),
    );
    assert_eq!(
        family.scalar_len(),
        coerce_to_interaction_boundary(&family, scalar_offset(2), family_range, SourceBoundaryBias::Forward),
    );

    let text = Text::from("😀b");
    let range = text_range(0, 2);
    assert_eq!(
        scalar_offset(1),
        coerce_to_interaction_boundary(&text, scalar_offset(1), range, SourceBoundaryBias::Nearest),
    );
    assert_eq!(
        scalar_offset(2),
        coerce_to_interaction_boundary(&text, scalar_offset(9), range, SourceBoundaryBias::Backward),
    );
    assert_eq!(
        scalar_offset(0),
        coerce_to_interaction_boundary(&text, scalar_offset(0), range, SourceBoundaryBias::Forward),
    );
}

// Kotlin also verifies lone UTF-16 surrogate handling. Rust `Text` deliberately stores valid
// UTF-8, so such input cannot enter this API; valid supplementary scalars are covered above.
#[test]
fn valid_supplementary_scalars_are_single_interaction_units() {
    let text = Text::from("😀A");
    assert_eq!(
        vec![scalar_offset(0), scalar_offset(1), scalar_offset(2)],
        interaction_boundaries(&text, text_range(0, 2)),
    );
    assert_eq!(Some(0x1F600), text.code_point_at_or_none(scalar_offset(0)));
}