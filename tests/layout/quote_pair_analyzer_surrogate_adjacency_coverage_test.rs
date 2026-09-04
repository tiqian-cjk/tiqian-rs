use tiqian::core::geometry::scalar_offset;
use tiqian::core::text::Text;
use tiqian::layout::quote_pair_analyzer::{QuotePair, QuotePairAnalyzer, QuoteType, is_non_cjk_in_word_apostrophe};

#[test]
fn low_quote_code_points_take_the_switch_default_without_pairing() {
    let analyzer = QuotePairAnalyzer;
    assert_eq!(
        vec![QuotePair::new(scalar_offset(2), scalar_offset(3), QuoteType::Double)],
        analyzer.analyze(&Text::from("‚‛“”")),
    );
}

#[test]
fn apostrophe_after_a_supplementary_scalar_walks_the_combine_arm_before() {
    let text = Text::from("😀’x");
    assert!(!is_non_cjk_in_word_apostrophe(&text, scalar_offset(1)));

    let private_use = Text::from("\u{E000}’b");
    assert!(!is_non_cjk_in_word_apostrophe(&private_use, scalar_offset(1)));
}

#[test]
fn apostrophe_before_a_supplementary_scalar_walks_both_low_check_arms() {
    let text = Text::from("a’😀");
    assert!(!is_non_cjk_in_word_apostrophe(&text, scalar_offset(1)));
}

#[test]
fn plain_and_boundary_neighbours_walk_the_non_surrogate_arms() {
    let plain = Text::from("a’b");
    assert!(is_non_cjk_in_word_apostrophe(&plain, scalar_offset(1)));

    let start = Text::from("’a");
    assert!(!is_non_cjk_in_word_apostrophe(&start, scalar_offset(0)));

    let end = Text::from("a’");
    assert!(!is_non_cjk_in_word_apostrophe(&end, scalar_offset(1)));
}
