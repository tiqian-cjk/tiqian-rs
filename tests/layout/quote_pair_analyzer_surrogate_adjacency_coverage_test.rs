use tiqian::core::text::Text;
use tiqian::layout::quote_pair_analyzer::{QuotePair, QuotePairAnalyzer, QuoteType, is_non_cjk_in_word_apostrophe};

#[test]
fn low_quote_code_points_take_the_switch_default_without_pairing() {
    let analyzer = QuotePairAnalyzer;
    assert_eq!(
        vec![QuotePair::new(2, 3, QuoteType::Double)],
        analyzer.analyze(&Text::from("‚‛“”")),
    );
}

#[test]
fn apostrophe_after_a_surrogate_pair_walks_the_combine_arm_before() {
    let text = Text::from("😀’x");
    assert!(!is_non_cjk_in_word_apostrophe(&text, 2));

    let private_use = Text::from("\u{E000}’b");
    assert!(!is_non_cjk_in_word_apostrophe(&private_use, 1));
}

#[test]
fn apostrophe_before_a_surrogate_walks_both_low_check_arms() {
    let text = Text::from("a’😀");
    assert!(!is_non_cjk_in_word_apostrophe(&text, 1));
}

#[test]
fn plain_and_boundary_neighbours_walk_the_non_surrogate_arms() {
    let plain = Text::from("a’b");
    assert!(is_non_cjk_in_word_apostrophe(&plain, 1));

    let start = Text::from("’a");
    assert!(!is_non_cjk_in_word_apostrophe(&start, 0));

    let end = Text::from("a’");
    assert!(!is_non_cjk_in_word_apostrophe(&end, 1));
}
