use tiqian::linebreak::unicode_punctuation_line_break::{
    UnicodePunctuationLineBreakClass, unicode_punctuation_line_break,
};

#[test]
fn exposes_pinned_western_and_cjk_punctuation_classes() {
    for (character, expected) in [
        ('(', UnicodePunctuationLineBreakClass::OpenPunctuation),
        (')', UnicodePunctuationLineBreakClass::CloseParenthesis),
        ('{', UnicodePunctuationLineBreakClass::OpenPunctuation),
        ('}', UnicodePunctuationLineBreakClass::ClosePunctuation),
        ('!', UnicodePunctuationLineBreakClass::Exclamation),
        (',', UnicodePunctuationLineBreakClass::InfixNumericSeparator),
        (
            '/',
            UnicodePunctuationLineBreakClass::SymbolsAllowingBreakAfter,
        ),
        ('-', UnicodePunctuationLineBreakClass::Hyphen),
        ('…', UnicodePunctuationLineBreakClass::Inseparable),
        ('“', UnicodePunctuationLineBreakClass::Quotation),
        ('”', UnicodePunctuationLineBreakClass::Quotation),
        ('（', UnicodePunctuationLineBreakClass::OpenPunctuation),
        ('）', UnicodePunctuationLineBreakClass::ClosePunctuation),
    ] {
        assert_eq!(
            expected,
            unicode_punctuation_line_break::class_of(character),
            "{character}"
        );
    }
}

#[test]
fn ordinary_letters_are_outside_the_punctuation_subset() {
    assert_eq!(
        UnicodePunctuationLineBreakClass::Other,
        unicode_punctuation_line_break::class_of('A')
    );
    assert_eq!(
        UnicodePunctuationLineBreakClass::Other,
        unicode_punctuation_line_break::class_of('中')
    );
}

#[test]
fn test_unicode_punctuation_line_break() {
    assert_eq!("17.0.0", unicode_punctuation_line_break::DATA_REVISION);
    assert!(!unicode_punctuation_line_break::DATA_SOURCE.is_empty());
    assert!(!unicode_punctuation_line_break::DATA_SHA256.is_empty());

    for (character, expected) in [
        ('\t', UnicodePunctuationLineBreakClass::BreakAfter),
        ('—', UnicodePunctuationLineBreakClass::BreakBoth),
        ('}', UnicodePunctuationLineBreakClass::ClosePunctuation),
        (')', UnicodePunctuationLineBreakClass::CloseParenthesis),
        ('!', UnicodePunctuationLineBreakClass::Exclamation),
        ('\u{058A}', UnicodePunctuationLineBreakClass::HyphenHH),
        ('-', UnicodePunctuationLineBreakClass::Hyphen),
        ('\u{2025}', UnicodePunctuationLineBreakClass::Inseparable),
        (',', UnicodePunctuationLineBreakClass::InfixNumericSeparator),
        ('々', UnicodePunctuationLineBreakClass::Nonstarter),
        ('(', UnicodePunctuationLineBreakClass::OpenPunctuation),
        ('"', UnicodePunctuationLineBreakClass::Quotation),
        (
            '/',
            UnicodePunctuationLineBreakClass::SymbolsAllowingBreakAfter,
        ),
        ('A', UnicodePunctuationLineBreakClass::Other),
    ] {
        assert_eq!(
            expected,
            unicode_punctuation_line_break::class_of(character)
        );
    }
}

#[test]
fn lookup_classes_cover_the_uax_tailorable_punctuation_classes() {
    for (character, expected) in [
        ('|', UnicodePunctuationLineBreakClass::BreakAfter),
        ('—', UnicodePunctuationLineBreakClass::BreakBoth),
        ('\u{058A}', UnicodePunctuationLineBreakClass::HyphenHH),
        ('‼', UnicodePunctuationLineBreakClass::Nonstarter),
    ] {
        assert_eq!(
            expected,
            unicode_punctuation_line_break::class_of(character)
        );
    }
}
