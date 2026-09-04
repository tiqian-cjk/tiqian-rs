use tiqian::core::geometry::{scalar_offset, text_range};
use tiqian::core::text::Text;
use tiqian::font::font_policy::{CjkFontRoleClassifier, FontRole, FontRoleClassifier, FontRoleContext};
use tiqian::layout::quote_pair_analyzer::{QuotePair, QuotePairAnalyzer, QuoteType};

fn analyze(text: &str) -> Vec<QuotePair> {
    QuotePairAnalyzer.analyze(&Text::from(text))
}

fn classify(text: &str) -> Vec<tiqian::layout::quote_pair_analyzer::QuoteRoleDecision> {
    let text = Text::from(text);
    let analyzer = QuotePairAnalyzer;
    analyzer.classify_quote_roles(
        &text,
        &analyzer.analyze(&text),
        &FontRoleContext::default(),
    )
}

fn role_signature(text: &str) -> String {
    let roles = classify(text);
    let mut scalar_index = 0;
    text.chars()
        .filter_map(|character| {
            let index = scalar_index;
            scalar_index += 1;
            matches!(character, '‘' | '’' | '“' | '”').then(|| {
                match roles.iter().find(|decision| decision.index.value() == index).unwrap().role {
                    FontRole::LatinText => 'L',
                    FontRole::CjkPunctuation => 'C',
                    _ => '?',
                }
            })
        })
        .collect()
}

#[test]
fn matches_double_quote_pair() {
    assert_eq!(
        vec![QuotePair::new(scalar_offset(2), scalar_offset(5), QuoteType::Double)],
        analyze("他说“你好”"),
    );
}

#[test]
fn matches_single_quote_pair() {
    assert_eq!(
        vec![QuotePair::new(scalar_offset(2), scalar_offset(5), QuoteType::Single)],
        analyze("他说‘你好’"),
    );
}

#[test]
fn matches_nested_quote_pairs() {
    let pairs = analyze("他说：“她说‘你好’。”");
    assert_eq!(2, pairs.len());
    assert!(pairs.contains(&QuotePair::new(scalar_offset(6), scalar_offset(9), QuoteType::Single)));
    assert!(pairs.contains(&QuotePair::new(scalar_offset(3), scalar_offset(11), QuoteType::Double)));
}

#[test]
fn unmatched_quotes_produce_no_pairs() {
    assert!(analyze("it’s").is_empty());
}

#[test]
fn contraction_apostrophe_does_not_close_outer_single_quote() {
    let text = "‘that’s’";
    assert_eq!(
        vec![QuotePair::new(scalar_offset(0), scalar_offset(text.chars().count() as i32 - 1), QuoteType::Single)],
        analyze(text),
    );
}

#[test]
fn contraction_inside_cjk_single_quotes_keeps_apostrophe_latin() {
    let text = "中‘that’s’中";
    let roles = classify(text);
    let classifier = CjkFontRoleClassifier;
    let context = FontRoleContext::default();
    assert_eq!(FontRole::CjkPunctuation, roles.iter().find(|decision| decision.index.value() == 1).unwrap().role);
    assert_eq!(FontRole::CjkPunctuation, roles.iter().find(|decision| decision.index.value() == 8).unwrap().role);
    assert_eq!(FontRole::LatinText, roles.iter().find(|decision| decision.index.value() == 6).unwrap().role);
    assert_eq!(FontRole::LatinText, classifier.classify(&Text::from(text), text_range(6, 7), &context));
}

#[test]
fn in_word_apostrophe_matrix_does_not_consume_outer_quote_pairs() {
    for word in ["that’s", "l’été", "rock’n’roll", "version2’s", "α’β", "а’б", "e\u{0301}’s"] {
        assert!(analyze(word).is_empty(), "{word}");
        let decisions = classify(word);
        assert!(decisions.iter().all(|decision| decision.role == FontRole::LatinText), "{word}: {decisions:?}");
        assert!(decisions.iter().all(|decision| decision.source == "NonCjkInWordApostrophe"), "{word}: {decisions:?}");

        let quoted = format!("‘{word}’");
        assert_eq!(
            vec![QuotePair::new(scalar_offset(0), scalar_offset(quoted.chars().count() as i32 - 1), QuoteType::Single)],
            analyze(&quoted),
            "{quoted}",
        );
        assert_eq!("L".repeat(quoted.chars().filter(|character| matches!(character, '‘' | '’')).count()), role_signature(&quoted), "{quoted}");
    }
}

#[test]
fn unmatched_curly_quotes_use_directional_context() {
    for (text, expected_signature) in [
        ("’90s", "L"),
        ("中文 ’90s", "L"),
        ("James’ book", "L"),
        ("“Hello", "L"),
        ("Hello”", "L"),
        ("中文“Hello", "C"),
        ("中文”", "C"),
        ("”", "C"),
    ] {
        assert_eq!(expected_signature, role_signature(text), "{text}");
    }
}

#[test]
fn mismatched_nesting_leaves_quotes_unmatched() {
    assert!(analyze("“hello’").is_empty());
}

#[test]
fn classifies_pair_as_cjk_when_outer_context_is_cjk() {
    let text = "他说“你好”";
    let roles = classify(text);
    assert_eq!(FontRole::CjkPunctuation, roles.iter().find(|decision| decision.index.value() == 2).unwrap().role);
    assert_eq!(FontRole::CjkPunctuation, roles.iter().find(|decision| decision.index.value() == 5).unwrap().role);
}

#[test]
fn classifies_pair_as_latin_when_outer_context_is_latin() {
    let roles = classify("he said “hello” world");
    assert_eq!(FontRole::LatinText, roles.iter().find(|decision| decision.index.value() == 8).unwrap().role);
    assert_eq!(FontRole::LatinText, roles.iter().find(|decision| decision.index.value() == 14).unwrap().role);
}

#[test]
fn classifies_both_quotes_as_cjk_for_cjk_quoted_latin_content() {
    let text = "他说“hello”";
    let roles = classify(text);
    assert_eq!(FontRole::CjkPunctuation, roles.iter().find(|decision| decision.index.value() == 2).unwrap().role);
    assert_eq!(FontRole::CjkPunctuation, roles.iter().find(|decision| decision.index.value() == text.chars().count() as i32 - 1).unwrap().role);
}

#[test]
fn whitespace_delimited_latin_quote_pair_overrides_cjk_outer_context() {
    let decisions = classify("（如 ‘O’, ‘Q’）");
    assert_eq!(vec![scalar_offset(3), scalar_offset(5), scalar_offset(8), scalar_offset(10)], decisions.iter().map(|decision| decision.index).collect::<Vec<_>>());
    assert!(decisions.iter().all(|decision| decision.role == FontRole::LatinText), "{decisions:?}");
    assert!(decisions.iter().all(|decision| decision.source == "DelimitedWesternQuotationRun"), "{decisions:?}");
}

#[test]
fn unspaced_cjk_quotation_of_latin_text_remains_cjk() {
    let text = "他说‘hello’";
    let roles = classify(text);
    assert_eq!(FontRole::CjkPunctuation, roles.iter().find(|decision| decision.index.value() == 2).unwrap().role);
    assert_eq!(FontRole::CjkPunctuation, roles.iter().find(|decision| decision.index.value() == text.chars().count() as i32 - 1).unwrap().role);
}

#[test]
fn adjacent_quoted_list_items_do_not_use_previous_item_content_as_outer_context() {
    for (text, expected_signature) in [
        ("便延伸出了“乃子”“大波”“大灯”“大雷”“大扎”“对A”“波霸”这些词", "CCCCCCCCCCCCCC"),
        ("这些太直白了是吧，\n “欧派”“double”“double may”呢", "CCCCCC"),
    ] {
        assert_eq!(expected_signature, role_signature(text), "{text}");
        let final_open = text[..text.rfind('“').unwrap()].chars().count() as i32;
        let final_close = text[..text.rfind('”').unwrap()].chars().count() as i32;
        let final_pair_decisions: Vec<_> = classify(text)
            .into_iter()
            .filter(|decision| decision.index.value() == final_open || decision.index.value() == final_close)
            .collect();
        assert_eq!(2, final_pair_decisions.len(), "{text}");
        assert!(final_pair_decisions.iter().all(|decision| decision.source == "PairedPunctuationOuterScriptContext"), "{text}: {final_pair_decisions:?}");
    }
}

#[test]
fn spaced_cjk_quoted_content_remains_cjk() {
    let text = "他说 ‘你好’";
    let roles = classify(text);
    assert_eq!(FontRole::CjkPunctuation, roles.iter().find(|decision| decision.index.value() == 3).unwrap().role);
    assert_eq!(FontRole::CjkPunctuation, roles.iter().find(|decision| decision.index.value() == text.chars().count() as i32 - 1).unwrap().role);
}

#[test]
fn classifies_pair_as_cjk_at_text_boundary() {
    let roles = classify("“你好”");
    assert_eq!(FontRole::CjkPunctuation, roles.iter().find(|decision| decision.index.value() == 0).unwrap().role);
    assert_eq!(FontRole::CjkPunctuation, roles.iter().find(|decision| decision.index.value() == 3).unwrap().role);
}

#[test]
fn classifies_text_start_latin_pair_from_quoted_content() {
    let roles = classify("“Hello” world");
    assert_eq!(FontRole::LatinText, roles.iter().find(|decision| decision.index.value() == 0).unwrap().role);
    assert_eq!(FontRole::LatinText, roles.iter().find(|decision| decision.index.value() == 6).unwrap().role);
}

#[test]
fn mixed_chinese_question_at_paragraph_start_uses_paragraph_language() {
    let text = "“Json是谁？”";
    let decisions = classify(text);
    assert_eq!(vec![scalar_offset(0), scalar_offset(text.chars().count() as i32 - 1)], decisions.iter().map(|decision| decision.index).collect::<Vec<_>>());
    assert!(decisions.iter().all(|decision| decision.role == FontRole::CjkPunctuation), "{decisions:?}");
    assert!(decisions.iter().all(|decision| decision.source == "ParagraphLanguageQuoteContext"), "{decisions:?}");
}

#[test]
fn explicit_english_paragraph_language_wins_for_mixed_quotation() {
    let text = Text::from("“Json是谁？”");
    let analyzer = QuotePairAnalyzer;
    let decisions = analyzer.classify_quote_roles(
        &text,
        &analyzer.analyze(&text),
        &FontRoleContext::with_locale("en".to_owned()),
    );
    assert!(decisions.iter().all(|decision| decision.role == FontRole::LatinText), "{decisions:?}");
    assert!(decisions.iter().all(|decision| decision.source == "ParagraphLanguageQuoteContext"), "{decisions:?}");
}

#[test]
fn common_digits_do_not_choose_the_quote_role() {
    let text = Text::from("“2024”");
    let analyzer = QuotePairAnalyzer;
    let pairs = analyzer.analyze(&text);
    let chinese = analyzer.classify_quote_roles(&text, &pairs, &FontRoleContext::default());
    assert!(chinese.iter().all(|decision| decision.role == FontRole::CjkPunctuation), "{chinese:?}");
    assert!(chinese.iter().all(|decision| decision.source == "ParagraphLanguageQuoteContext"), "{chinese:?}");
    let english = analyzer.classify_quote_roles(&text, &pairs, &FontRoleContext::with_locale("en".to_owned()));
    assert!(english.iter().all(|decision| decision.role == FontRole::LatinText), "{english:?}");
    assert!(english.iter().all(|decision| decision.source == "ParagraphLanguageQuoteContext"), "{english:?}");
}

#[test]
fn non_latin_western_scripts_participate_as_strong_script_evidence() {
    for (text, expected_signature) in [
        ("“Привет”", "LL"),
        ("“π是谁？”", "CC"),
        ("他说“Привет”", "CC"),
    ] {
        assert_eq!(expected_signature, role_signature(text), "{text}");
    }
}

#[test]
fn numbered_cjk_quote_prefix_uses_quoted_content() {
    let text = "1.“你知道李白是怎么死的吗？”";
    let decisions = classify(text);
    assert_eq!(FontRole::CjkPunctuation, decisions.iter().find(|decision| decision.index.value() == 2).unwrap().role);
    assert_eq!(FontRole::CjkPunctuation, decisions.iter().find(|decision| decision.index.value() == text.chars().count() as i32 - 1).unwrap().role);
    assert_eq!("PairedPunctuationContentScriptContext", decisions.iter().find(|decision| decision.index.value() == 2).unwrap().source);
}

#[test]
fn numbered_latin_quote_prefix_still_uses_latin_content() {
    let roles = classify("1.“Hello”");
    assert_eq!(FontRole::LatinText, roles.iter().find(|decision| decision.index.value() == 2).unwrap().role);
    assert_eq!(FontRole::LatinText, roles.iter().find(|decision| decision.index.value() == 8).unwrap().role);
}

#[test]
fn classifies_nested_pairs_by_outermost_context() {
    let roles = classify("他说：“她说‘你好’。”");
    for index in [3, 11, 6, 9] {
        assert_eq!(FontRole::CjkPunctuation, roles.iter().find(|decision| decision.index.value() == index).unwrap().role);
    }
}

#[test]
fn classifies_latin_nested_quotes_by_outer_context() {
    let roles = classify("She said “he said ‘hello’ today” end");
    assert!(roles.iter().all(|decision| decision.role == FontRole::LatinText));
}

#[test]
fn skips_ascii_punctuation_when_resolving_context() {
    let roles = classify("English: “hello”");
    assert_eq!(FontRole::LatinText, roles.iter().find(|decision| decision.index.value() == 9).unwrap().role);
    assert_eq!(FontRole::LatinText, roles.iter().find(|decision| decision.index.value() == 15).unwrap().role);
}

#[test]
fn skips_neutral_dash_when_resolving_context() {
    let roles = classify("English — “hello”");
    assert_eq!(FontRole::LatinText, roles.iter().find(|decision| decision.index.value() == 10).unwrap().role);
    assert_eq!(FontRole::LatinText, roles.iter().find(|decision| decision.index.value() == 16).unwrap().role);
}

#[test]
fn end_of_text_quote_pair_classified_by_outer_context() {
    let roles = classify("he said “hello”");
    assert_eq!(FontRole::LatinText, roles.iter().find(|decision| decision.index.value() == 8).unwrap().role);
    assert_eq!(FontRole::LatinText, roles.iter().find(|decision| decision.index.value() == 14).unwrap().role);
}

#[test]
fn representative_quote_context_matrix_remains_stable() {
    for (text, expected_signature) in [
        ("“Hello”", "LL"), ("“你好”", "CC"), ("“Json是谁？”", "CC"), ("“Привет”", "LL"),
        ("他说“hello”", "CC"), ("He said “你好”", "LL"), ("（如 ‘O’, ‘Q’）", "LLLL"),
        ("他说 ‘你好’", "CC"), ("“”English", "LL"), ("“”中文", "CC"), ("“”", "CC"),
        ("1.“中文”", "CC"), ("1.“Hello”", "LL"), ("他说：“She said ‘hello’.”", "CLLC"),
        ("English “他说‘你好’” end", "LCCL"), ("中文‘don’t’", "CLC"),
        ("中文 ‘don’t’", "LLL"), ("他说：“第一行\n第二行。”", "CC"), ("（如\t‘O’）", "LL"),
    ] {
        assert_eq!(expected_signature, role_signature(text), "{text}");
    }
}

#[test]
fn role_decision_sources_stay_explainable_across_fallback_paths() {
    for (text, expected_source) in [
        ("“Hello”", "PairedPunctuationContentScriptContext"),
        ("“Json是谁？”", "ParagraphLanguageQuoteContext"),
        ("English—“Hello”", "PairedPunctuationOuterScriptContext"),
        ("（如 ‘O’）", "DelimitedWesternQuotationRun"),
        ("1.“中文”", "PairedPunctuationContentScriptContext"),
        ("“”English", "PairedPunctuationOuterScriptContext"),
        ("“”", "ParagraphLanguageQuoteContext"),
        ("that’s", "NonCjkInWordApostrophe"),
        ("中文 ’90s", "DelimitedUnmatchedWesternQuote"),
        ("James’", "UnmatchedQuoteSurroundingScriptContext"),
        ("’90s", "UnmatchedQuoteSurroundingScriptContext"),
        ("”", "ParagraphLanguageQuoteContext"),
    ] {
        let decisions = classify(text);
        assert!(!decisions.is_empty(), "{text}");
        assert!(decisions.iter().all(|decision| decision.source == expected_source), "{text}: {decisions:?}");
    }
}
