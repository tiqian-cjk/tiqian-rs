use tiqian::org::tiqian::core::Text::Text;
use tiqian::org::tiqian::font::FontPolicy::{FontRole, FontRoleContext};
use tiqian::org::tiqian::layout::QuotePairAnalyzer::{QuotePair, QuotePairAnalyzer, QuoteType};

fn decisions(text: &str) -> Vec<tiqian::org::tiqian::layout::QuotePairAnalyzer::QuoteRoleDecision> {
    let analyzer = QuotePairAnalyzer;
    let text = Text::from(text);
    let pairs = analyzer.analyze(&text);
    analyzer.classify_quote_roles(&text, &pairs, &FontRoleContext::default())
}

#[test]
fn matches_simple_and_nested_quote_pairs() {
    let analyzer = QuotePairAnalyzer;
    assert_eq!(
        vec![QuotePair::new(2, 5, QuoteType::Double)],
        analyzer.analyze(&Text::from("他说“你好”")),
    );
    let nested = analyzer.analyze(&Text::from("他说：“她说‘你好’。”"));
    assert_eq!(2, nested.len());
    assert!(nested.contains(&QuotePair::new(6, 9, QuoteType::Single)));
    assert!(nested.contains(&QuotePair::new(3, 11, QuoteType::Double)));
}

#[test]
fn in_word_apostrophe_does_not_consume_outer_single_quote_pair() {
    let analyzer = QuotePairAnalyzer;
    let text = "‘that’s’";
    assert_eq!(
        vec![QuotePair::new(0, 7, QuoteType::Single)],
        analyzer.analyze(&Text::from(text))
    );

    for word in [
        "that’s",
        "l’été",
        "rock’n’roll",
        "version2’s",
        "α’β",
        "а’б",
        "e\u{0301}’s",
    ] {
        let word = Text::from(word);
        assert!(analyzer.analyze(&word).is_empty(), "{word}");
        let roles = analyzer.classify_quote_roles(&word, &[], &FontRoleContext::default());
        assert!(
            roles
                .iter()
                .all(|decision| decision.role == FontRole::LatinText)
        );
        assert!(
            roles
                .iter()
                .all(|decision| decision.source == "NonCjkInWordApostrophe")
        );
    }
}

#[test]
fn cjk_outer_context_classifies_paired_quotes_as_cjk() {
    let text = "他说“hello”";
    let result = decisions(text);

    assert_eq!(
        vec![2, 8],
        result
            .iter()
            .map(|decision| decision.index)
            .collect::<Vec<_>>()
    );
    assert!(
        result
            .iter()
            .all(|decision| decision.role == FontRole::CjkPunctuation)
    );
    assert!(
        result
            .iter()
            .all(|decision| decision.source == "PairedPunctuationOuterScriptContext")
    );
}

#[test]
fn latin_content_at_text_start_classifies_quotes_as_latin() {
    let result = decisions("“Hello” world");

    assert_eq!(
        vec![0, 6],
        result
            .iter()
            .map(|decision| decision.index)
            .collect::<Vec<_>>()
    );
    assert!(
        result
            .iter()
            .all(|decision| decision.role == FontRole::LatinText)
    );
}

#[test]
fn whitespace_delimited_western_quotation_overrides_cjk_outer_context() {
    let result = decisions("（如 ‘O’, ‘Q’）");

    assert_eq!(
        vec![3, 5, 8, 10],
        result
            .iter()
            .map(|decision| decision.index)
            .collect::<Vec<_>>()
    );
    assert!(
        result
            .iter()
            .all(|decision| decision.role == FontRole::LatinText)
    );
    assert!(
        result
            .iter()
            .all(|decision| decision.source == "DelimitedWesternQuotationRun")
    );
}

#[test]
fn paragraph_language_breaks_mixed_or_digit_only_quote_ties() {
    let analyzer = QuotePairAnalyzer;
    for text in ["“Json是谁？”", "“2024”"] {
        let text = Text::from(text);
        let pairs = analyzer.analyze(&text);
        let chinese = analyzer.classify_quote_roles(&text, &pairs, &FontRoleContext::default());
        assert!(
            chinese
                .iter()
                .all(|decision| decision.role == FontRole::CjkPunctuation)
        );
        assert!(
            chinese
                .iter()
                .all(|decision| decision.source == "ParagraphLanguageQuoteContext")
        );
        let english = analyzer.classify_quote_roles(
            &text,
            &pairs,
            &FontRoleContext::with_locale("en".to_owned()),
        );
        assert!(
            english
                .iter()
                .all(|decision| decision.role == FontRole::LatinText)
        );
        assert!(
            english
                .iter()
                .all(|decision| decision.source == "ParagraphLanguageQuoteContext")
        );
    }
}

#[test]
fn unmatched_quotes_use_directional_script_context() {
    let cases = [
        (
            "’90s",
            FontRole::LatinText,
            "UnmatchedQuoteSurroundingScriptContext",
        ),
        (
            "中文“Hello",
            FontRole::CjkPunctuation,
            "ParagraphLanguageQuoteContext",
        ),
        (
            "”",
            FontRole::CjkPunctuation,
            "ParagraphLanguageQuoteContext",
        ),
    ];
    for (text, role, source) in cases {
        let result = decisions(text);
        assert_eq!(1, result.len(), "{text}");
        assert_eq!(role, result[0].role, "{text}");
        assert_eq!(source, result[0].source, "{text}");
    }
}
