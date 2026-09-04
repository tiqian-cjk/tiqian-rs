use tiqian::common::HashMap;
use tiqian::core::geometry::{scalar_offset, text_range};
use tiqian::core::text::Text;
use tiqian::font::font_policy::{CjkFontRoleClassifier, FontRole, FontRoleClassifier, FontRoleContext};
use tiqian::layout::quote_pair_analyzer::{
    QuotePairAnalyzer, QuotePairAwareFontRoleClassifier,
};

#[test]
fn deprecated_classify_pairs_with_font_role_classifier_delegates() {
    let analyzer = QuotePairAnalyzer;
    let classifier = CjkFontRoleClassifier;
    let text = Text::from("他说“你好”");
    let pairs = analyzer.analyze(&text);
    let roles = analyzer.classify_pairs_with_font_role_classifier(
        &text,
        &pairs,
        &classifier,
        &FontRoleContext::default(),
    );
    assert_eq!(Some(&FontRole::CjkPunctuation), roles.get(&scalar_offset(2)));
}

#[test]
fn deprecated_classify_quote_roles_with_font_role_classifier_delegates() {
    let analyzer = QuotePairAnalyzer;
    let classifier = CjkFontRoleClassifier;
    let text = Text::from("他说“你好”");
    let pairs = analyzer.analyze(&text);
    assert!(!analyzer
        .classify_quote_roles_with_font_role_classifier(
            &text,
            &pairs,
            &classifier,
            &FontRoleContext::default(),
        )
        .is_empty());
}

#[test]
fn code_point_before_surrogate_pair_returns_supplementary() {
    let analyzer = QuotePairAnalyzer;
    assert!(analyzer.analyze(&Text::from("😀’")).is_empty());
}

#[test]
fn code_point_at_or_null_surrogate_pair_returns_supplementary() {
    let analyzer = QuotePairAnalyzer;
    assert!(!analyzer
        .classify_quote_roles(&Text::from("’😀"), &[], &FontRoleContext::default())
        .is_empty());
}

#[test]
fn code_point_at_or_null_non_surrogate_returns_self() {
    let analyzer = QuotePairAnalyzer;
    assert!(analyzer
        .classify_quote_roles(&Text::from("abc"), &[], &FontRoleContext::default())
        .is_empty());
}

#[test]
fn code_point_before_returns_null_at_start() {
    let analyzer = QuotePairAnalyzer;
    assert!(!analyzer
        .classify_quote_roles(&Text::from("’"), &[], &FontRoleContext::default())
        .is_empty());
}

#[test]
fn code_point_before_returns_supplementary_for_surrogate_pair() {
    let analyzer = QuotePairAnalyzer;
    assert!(!analyzer
        .classify_quote_roles(&Text::from("😀’"), &[], &FontRoleContext::default())
        .is_empty());
}

#[test]
fn quote_pair_aware_font_role_classifier_uses_override() {
    let classifier = CjkFontRoleClassifier;
    let roles = HashMap::from([(scalar_offset(2), FontRole::LatinText)]);
    let override_classifier = QuotePairAwareFontRoleClassifier::new(&classifier, &roles);
    let text = Text::from("ab");
    let result = override_classifier.classify(&text, text_range(0, 2), &FontRoleContext::default());
    assert_eq!(FontRole::LatinText, result);
}

#[test]
fn quote_pair_aware_font_role_classifier_delegates_when_no_override() {
    let classifier = CjkFontRoleClassifier;
    let roles = HashMap::new();
    let override_classifier = QuotePairAwareFontRoleClassifier::new(&classifier, &roles);
    let text = Text::from("ab");
    let range = text_range(0, 2);
    let context = FontRoleContext::default();
    assert_eq!(
        classifier.classify(&text, range, &context),
        override_classifier.classify(&text, range, &context),
    );
}

#[test]
fn double_quote_close_with_empty_stack_ignores() {
    let analyzer = QuotePairAnalyzer;
    assert!(analyzer.analyze(&Text::from("”")).is_empty());
}

#[test]
fn single_quote_close_with_empty_stack_ignores() {
    let analyzer = QuotePairAnalyzer;
    assert!(analyzer.analyze(&Text::from("’")).is_empty());
}

#[test]
fn in_word_apostrophe_after_supplementary_does_not_close() {
    let analyzer = QuotePairAnalyzer;
    assert!(!analyzer
        .classify_quote_roles(&Text::from("😀’x"), &[], &FontRoleContext::default())
        .is_empty());
}

#[test]
fn code_point_at_or_null_with_supplementary_after_quote() {
    let analyzer = QuotePairAnalyzer;
    assert!(!analyzer
        .classify_quote_roles(&Text::from("a’😀"), &[], &FontRoleContext::default())
        .is_empty());
}

#[test]
fn code_point_before_with_high_surrogate_before_quote() {
    let analyzer = QuotePairAnalyzer;
    assert!(!analyzer
        .classify_quote_roles(&Text::from("😀’"), &[], &FontRoleContext::default())
        .is_empty());
}

#[test]
fn code_point_at_or_null_with_index_out_of_range() {
    let analyzer = QuotePairAnalyzer;
    assert!(!analyzer
        .classify_quote_roles(&Text::from("a’"), &[], &FontRoleContext::default())
        .is_empty());
}

#[test]
fn analyze_with_double_quote_open() {
    let analyzer = QuotePairAnalyzer;
    assert!(analyzer.analyze(&Text::from("“abc")).is_empty());
}

#[test]
fn code_point_at_or_null_high_surrogate_not_in_range_returns_high() {
    let analyzer = QuotePairAnalyzer;
    assert!(!analyzer
        .classify_quote_roles(&Text::from("x’a"), &[], &FontRoleContext::default())
        .is_empty());
}

#[test]
fn single_quote_pair_match() {
    let analyzer = QuotePairAnalyzer;
    let pairs = analyzer.analyze(&Text::from("‘’"));
    assert_eq!(1, pairs.len());
    assert_eq!(tiqian::layout::quote_pair_analyzer::QuoteType::Single, pairs[0].quote_type);
}

#[test]
fn analyze_with_all_quote_types() {
    let analyzer = QuotePairAnalyzer;
    assert_eq!(2, analyzer.analyze(&Text::from("“‘abc’”")).len());
}

#[test]
fn code_point_before_non_surrogate_bmp_char() {
    let analyzer = QuotePairAnalyzer;
    assert!(!analyzer
        .classify_quote_roles(&Text::from("A’"), &[], &FontRoleContext::default())
        .is_empty());
}
