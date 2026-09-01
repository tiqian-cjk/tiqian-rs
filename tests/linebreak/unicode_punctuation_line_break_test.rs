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
            unicode_punctuation_line_break::class_of(character as i32),
            "{character}"
        );
    }
}

#[test]
fn ordinary_letters_are_outside_the_punctuation_subset() {
    assert_eq!(
        UnicodePunctuationLineBreakClass::Other,
        unicode_punctuation_line_break::class_of('A' as i32)
    );
    assert_eq!(
        UnicodePunctuationLineBreakClass::Other,
        unicode_punctuation_line_break::class_of('中' as i32)
    );
}

#[test]
fn test_unicode_punctuation_line_break() {
    assert_eq!("17.0.0", unicode_punctuation_line_break::DATA_REVISION);
    assert!(!unicode_punctuation_line_break::DATA_SOURCE.is_empty());
    assert!(!unicode_punctuation_line_break::DATA_SHA256.is_empty());

    for (code_point, expected) in [
        (0x0009, UnicodePunctuationLineBreakClass::BreakAfter),
        (0x2014, UnicodePunctuationLineBreakClass::BreakBoth),
        (0x007D, UnicodePunctuationLineBreakClass::ClosePunctuation),
        (0x0029, UnicodePunctuationLineBreakClass::CloseParenthesis),
        (0x0021, UnicodePunctuationLineBreakClass::Exclamation),
        (0x058A, UnicodePunctuationLineBreakClass::HyphenHH),
        (0x002D, UnicodePunctuationLineBreakClass::Hyphen),
        (0x2025, UnicodePunctuationLineBreakClass::Inseparable),
        (0x002C, UnicodePunctuationLineBreakClass::InfixNumericSeparator),
        (0x3005, UnicodePunctuationLineBreakClass::Nonstarter),
        (0x0028, UnicodePunctuationLineBreakClass::OpenPunctuation),
        (0x0022, UnicodePunctuationLineBreakClass::Quotation),
        (0x002F, UnicodePunctuationLineBreakClass::SymbolsAllowingBreakAfter),
        (0x0041, UnicodePunctuationLineBreakClass::Other),
    ] {
        assert_eq!(expected, unicode_punctuation_line_break::class_of(code_point));
    }
}

#[test]
fn lookup_classes_cover_the_uax_tailorable_punctuation_classes() {
    for (code_point, expected) in [
        (0x007C, UnicodePunctuationLineBreakClass::BreakAfter),
        (0x2014, UnicodePunctuationLineBreakClass::BreakBoth),
        (0x058A, UnicodePunctuationLineBreakClass::HyphenHH),
        (0x203C, UnicodePunctuationLineBreakClass::Nonstarter),
    ] {
        assert_eq!(expected, unicode_punctuation_line_break::class_of(code_point));
    }
}

#[test]
fn non_scalar_code_points_are_rejected() {
    for code_point in [-1, 0x110000, 0xD800, 0xDFFF] {
        assert!(std::panic::catch_unwind(|| unicode_punctuation_line_break::class_of(code_point)).is_err());
    }
}
