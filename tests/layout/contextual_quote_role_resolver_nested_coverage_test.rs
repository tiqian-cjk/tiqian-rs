use tiqian::core::text::Text;
use tiqian::font::font_policy::{FontRole, FontRoleContext};
use tiqian::layout::contextual_quote_role_resolver::ContextualQuoteRoleResolver;
use tiqian::layout::quote_pair_analyzer::QuotePairAnalyzer;

fn decisions(text: &str) -> Vec<tiqian::layout::quote_pair_analyzer::QuoteRoleDecision> {
    let text = Text::from(text);
    let analyzer = QuotePairAnalyzer;
    let pairs = analyzer.analyze(&text);
    ContextualQuoteRoleResolver::new(&text, &pairs, &FontRoleContext::default()).resolve()
}

#[test]
fn nested_pair_inside_neutral_enclosing_inherits_the_outer_quotation() {
    let decisions = decisions("“—‘文’—”");
    let outer = decisions.iter().find(|decision| decision.index == 0).unwrap();
    let inner = decisions.iter().find(|decision| decision.index == 2).unwrap();
    assert_eq!("PairedPunctuationEnclosingQuoteContext", inner.source);
    assert_eq!("quote-pair-inherits-enclosing-quotation", inner.reason);
    assert_eq!(outer.role, inner.role);
    assert!(decisions
        .iter()
        .all(|decision| decision.source != "DelimitedWesternQuotationRun"));
}

#[test]
fn space_before_unmatched_quote_with_cjk_right_skips_the_delimited_rule() {
    let decision = decisions(" ’中")
        .into_iter()
        .find(|decision| decision.index == 1)
        .unwrap();
    assert_eq!("UnmatchedQuoteSurroundingScriptContext", decision.source);
    assert_eq!(FontRole::CjkPunctuation, decision.role);
}

#[test]
fn tab_before_a_wholly_western_pair_delimits_like_a_space() {
    let decision = decisions("\t“a”")
        .into_iter()
        .find(|decision| decision.index == 1)
        .unwrap();
    assert_eq!("DelimitedWesternQuotationRun", decision.source);
}

#[test]
fn space_before_a_pair_with_non_western_content_skips_the_delimited_rule() {
    let decision = decisions(" “中”")
        .into_iter()
        .find(|decision| decision.index == 1)
        .unwrap();
    assert_eq!("PairedPunctuationContentScriptContext", decision.source);
    assert_eq!(FontRole::CjkPunctuation, decision.role);
}

#[test]
fn space_before_a_mixed_content_pair_reports_mixed_content() {
    let decision = decisions(" “a中”")
        .into_iter()
        .find(|decision| decision.index == 1)
        .unwrap();
    assert_eq!("ParagraphLanguageQuoteContext", decision.source);
    assert!(decision.reason.contains("mixed-quoted-content"));
}

#[test]
fn mixed_enclosing_level_falls_back_to_paragraph_language() {
    let decision = decisions("a“中”文")
        .into_iter()
        .find(|decision| decision.index == 1)
        .unwrap();
    assert_eq!("ParagraphLanguageQuoteContext", decision.source);
    assert!(decision.reason.contains("mixed-enclosing-level-script"));
}

#[test]
fn non_chinese_locale_resolves_neutral_context_to_latin_text() {
    let text = Text::from("’");
    let context = FontRoleContext::with_locale("en-US".to_owned());
    let decisions = ContextualQuoteRoleResolver::new(&text, &[], &context).resolve();
    assert_eq!(FontRole::LatinText, decisions[0].role);
    assert!(decisions[0].reason.contains("paragraph-language=en-US"));
}

#[test]
fn private_use_char_before_a_quote_fails_the_low_surrogate_range_above() {
    let decision = decisions("\u{E000}“中")
        .into_iter()
        .find(|decision| decision.index == 1)
        .unwrap();
    assert_eq!("UnmatchedQuoteSurroundingScriptContext", decision.source);
    assert_eq!(FontRole::CjkPunctuation, decision.role);
}

#[test]
fn sibling_pairs_inside_one_quotation_each_inherit_the_outer_role() {
    let decisions = decisions("“‘a’‘b’”");
    let first = decisions.iter().find(|decision| decision.index == 1).unwrap();
    let second = decisions.iter().find(|decision| decision.index == 4).unwrap();
    assert_eq!("PairedPunctuationEnclosingQuoteContext", first.source);
    assert_eq!("PairedPunctuationEnclosingQuoteContext", second.source);
    assert_eq!(first.role, second.role);
}
