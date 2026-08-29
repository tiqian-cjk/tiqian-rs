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
