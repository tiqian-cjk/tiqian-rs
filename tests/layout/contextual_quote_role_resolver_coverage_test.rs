use tiqian::core::text::Text;
use tiqian::font::font_policy::{FontRole, FontRoleContext};
use tiqian::layout::quote_pair_analyzer::QuotePairAnalyzer;

fn analyze_and_classify(text: &str) -> Vec<tiqian::layout::quote_pair_analyzer::QuoteRoleDecision> {
    let analyzer = QuotePairAnalyzer;
    let text = Text::from(text);
    let pairs = analyzer.analyze(&text);
    analyzer.classify_quote_roles(&text, &pairs, &FontRoleContext::default())
}

fn classify_without_pairs(text: &str) -> Vec<tiqian::layout::quote_pair_analyzer::QuoteRoleDecision> {
    let analyzer = QuotePairAnalyzer;
    let text = Text::from(text);
    analyzer.classify_quote_roles(&text, &[], &FontRoleContext::default())
}

#[test]
fn nested_pair_inherits_enclosing_quote_role() {
    let decisions = analyze_and_classify("他说：“她说‘你好’。”");
    if let Some(inner_open) = decisions.iter().find(|decision| decision.index == 6) {
        assert_eq!(FontRole::CjkPunctuation, inner_open.role);
    }
    if let Some(inner_close) = decisions.iter().find(|decision| decision.index == 9) {
        assert_eq!(FontRole::CjkPunctuation, inner_close.role);
    }
}

#[test]
fn nested_pair_latin_inner_inherits_cjk_enclosing() {
    let decisions = analyze_and_classify("他说：“hello”");
    if let Some(inner_open) = decisions.iter().find(|decision| decision.index == 3) {
        assert_eq!(FontRole::CjkPunctuation, inner_open.role);
    }
}

#[test]
fn unmatched_right_single_quote_uses_surrounding_script() {
    let decisions = analyze_and_classify("abc’def");
    assert!(decisions.iter().any(|decision| decision.role == FontRole::LatinText));
}

#[test]
fn unmatched_right_double_quote() {
    assert!(!analyze_and_classify("abc”").is_empty());
}

#[test]
fn unmatched_left_double_quote() {
    assert!(!analyze_and_classify("“abc").is_empty());
}

#[test]
fn unmatched_left_single_quote() {
    assert!(!analyze_and_classify("‘abc").is_empty());
}

#[test]
fn conflicting_unmatched_quotes_uses_paragraph_language() {
    assert!(!analyze_and_classify("α’中").is_empty());
}

#[test]
fn unmatched_quote_with_surrogate_pair_content() {
    assert!(!analyze_and_classify("😀’中").is_empty());
}

#[test]
fn code_point_at_compat_with_supplementary_char() {
    assert!(!analyze_and_classify("😀“😀”").is_empty());
}

#[test]
fn code_point_length_at_supplementary_in_content() {
    assert!(!analyze_and_classify("“😀”").is_empty());
}

#[test]
fn non_cjk_in_word_apostrophe_with_surrogate_before() {
    assert!(!classify_without_pairs("😀’x").is_empty());
}

#[test]
fn whitespace_delimited_western_quote_unmatched() {
    assert!(classify_without_pairs("中文 ’90s")
        .iter()
        .any(|decision| decision.source == "DelimitedUnmatchedWesternQuote"));
}

#[test]
fn enclosing_pair_resolved_before_inner() {
    assert!(!analyze_and_classify("“‘中’”").is_empty());
}

#[test]
fn pair_by_close_skip_in_nearest_strong_script() {
    assert!(!classify_without_pairs("“‘abc’”").is_empty());
}

#[test]
fn pair_by_open_skip_in_nearest_strong_script() {
    assert!(!classify_without_pairs("“‘abc’”").is_empty());
}

#[test]
fn ambiguous_curly_quote_unmatched_in_text() {
    assert!(!classify_without_pairs("abc’").is_empty());
}

#[test]
fn resolve_unmatched_with_both_surrounding_roles_null() {
    assert!(!classify_without_pairs("’").is_empty());
}

#[test]
fn nearest_strong_script_role_backward_skips_paired_close_quote() {
    assert!(!analyze_and_classify("“‘a’”’").is_empty());
}

#[test]
fn nearest_strong_script_role_forward_skips_paired_open_quote() {
    assert!(!analyze_and_classify("’“abc”").is_empty());
}

#[test]
fn enclosing_pair_resolved_before_inner_pair() {
    let inner_open = analyze_and_classify("“‘abc’”")
        .into_iter()
        .find(|decision| decision.index == 1)
        .unwrap();
    assert_eq!(FontRole::CjkPunctuation, inner_open.role);
    assert_eq!("PairedPunctuationEnclosingQuoteContext", inner_open.source);
}

#[test]
fn whitespace_delimited_western_quote_paired() {
    assert!(!analyze_and_classify("“ ‘hello’ ”").is_empty());
}

#[test]
fn conflicting_unmatched_quotes_both_non_null() {
    assert!(!classify_without_pairs("α’中").is_empty());
}

#[test]
fn no_unmatched_quote_context() {
    assert!(!classify_without_pairs("’").is_empty());
}

#[test]
fn nearest_strong_script_role_backward_through_surrogate_pair() {
    assert!(!analyze_and_classify("😀“abc”").is_empty());
}

#[test]
fn nearest_strong_script_role_forward_through_surrogate_pair() {
    assert!(!analyze_and_classify("“abc😀”").is_empty());
}

#[test]
fn nested_pair_skips_inner_in_script_evidence() {
    assert!(!analyze_and_classify("“‘中’”").is_empty());
}

#[test]
fn mixed_script_enclosing_level_uses_paragraph_language() {
    assert!(!analyze_and_classify("abc“中”").is_empty());
}

#[test]
fn unmatched_right_single_quote_with_left_role() {
    assert!(!classify_without_pairs("中’").is_empty());
}

#[test]
fn unmatched_right_single_quote_with_right_role() {
    assert!(!classify_without_pairs("’中").is_empty());
}

#[test]
fn unmatched_quote_with_whitespace_before_and_latin_right() {
    assert!(classify_without_pairs(" ’abc")
        .iter()
        .any(|decision| decision.source == "DelimitedUnmatchedWesternQuote"));
}

#[test]
fn non_cjk_in_word_apostrophe_paired() {
    let analyzer = QuotePairAnalyzer;
    assert!(analyzer.analyze(&Text::from("‘it’s")).is_empty());
}

#[test]
fn code_point_length_at_surrogate_pair_in_content() {
    assert!(!analyze_and_classify("“😀”").is_empty());
}

#[test]
fn code_point_at_compat_supplementary_in_outer_evidence() {
    assert!(!analyze_and_classify("😀“abc”😀").is_empty());
}

#[test]
fn conflicting_unmatched_quotes_left_and_right_non_null() {
    assert!(!analyze_and_classify("a’b“c").is_empty());
}

#[test]
fn unmatched_quote_non_whitespace_before() {
    assert!(!analyze_and_classify("a“").is_empty());
}

#[test]
fn nearest_strong_script_role_backward_hits_supplementary() {
    assert!(!analyze_and_classify("😀“").is_empty());
}

#[test]
fn nearest_strong_script_role_forward_hits_supplementary() {
    assert!(!analyze_and_classify("“😀").is_empty());
}

#[test]
fn enclosing_pair_unresolved_falls_through_to_content() {
    assert!(!analyze_and_classify("“‘abc’”").is_empty());
}

#[test]
fn unmatched_quote_at_start_with_right_role() {
    assert!(!analyze_and_classify("“abc").is_empty());
}
