use tiqian::core::Geometry::TextRange;
use tiqian::core::Text::Text;
use tiqian::font::FontPolicy::{CjkFontRoleClassifier, FontRole};

fn classify(text: &str, start: i32, end: i32) -> FontRole {
    CjkFontRoleClassifier
        .classify_with_default_context(&Text::from(text), TextRange::new(start, end))
}

#[test]
fn classifies_cjk_text() {
    assert_eq!(FontRole::CjkText, classify("提", 0, 1));
}

#[test]
fn classifies_cjk_punctuation() {
    for text in ["……", "⋯⋯", "——", "⸺", "。", "・", "‧", "～", "／"] {
        assert_eq!(FontRole::CjkPunctuation, classify(text, 0, 1), "{text}");
    }
}

#[test]
fn classifies_latin_text() {
    assert_eq!(FontRole::LatinText, classify("English", 0, 1));
}

#[test]
fn classifies_unicode_emoji_presentation_without_reclassifying_plain_keycap_bases() {
    for text in ["⌚", "🀄", "🫪"] {
        assert_eq!(
            FontRole::Emoji,
            classify(text, 0, Text::from(text).utf16_len()),
            "{text}"
        );
    }
    assert_eq!(FontRole::LatinText, classify("1", 0, 1));
    assert_eq!(FontRole::Symbol, classify("❤", 0, 1));
}

#[test]
fn classifies_ascii_symbols_and_punctuation_as_latin() {
    for character in [
        '%', '.', ',', ':', ';', '!', '?', '#', '@', '&', '*', '+', '=', '<', '>', '|', '^', '_',
        '$', '\'', '"',
    ] {
        assert_eq!(
            FontRole::LatinText,
            classify(&character.to_string(), 0, 1),
            "char={character}"
        );
    }
    assert_eq!(FontRole::LatinText, classify("中%文", 1, 2));
}

#[test]
fn classifies_ascii_hyphen_slash_tilde_as_latin_regardless_of_context() {
    assert_eq!(FontRole::LatinText, classify("well-known", 4, 5));
    assert_eq!(FontRole::LatinText, classify("https://example", 6, 7));
    assert_eq!(FontRole::LatinText, classify("https://example", 7, 8));
    assert_eq!(FontRole::LatinText, classify("中文/TERFism", 2, 3));
    assert_eq!(FontRole::LatinText, classify("中文/中文", 2, 3));
    assert_eq!(FontRole::LatinText, classify("中文-中文", 2, 3));
}

#[test]
fn classifies_curly_quotes_as_cjk_when_surrounded_by_cjk() {
    assert_eq!(FontRole::CjkPunctuation, classify("他说“你好”", 2, 3));
    assert_eq!(FontRole::CjkPunctuation, classify("他说“你好”", 5, 6));
    assert_eq!(FontRole::CjkPunctuation, classify("他说‘你好’", 2, 3));
    assert_eq!(FontRole::CjkPunctuation, classify("他说‘你好’", 5, 6));
}

#[test]
fn classifies_curly_quotes_as_latin_when_surrounded_by_latin() {
    assert_eq!(FontRole::LatinText, classify("said “hello” end", 5, 6));
    assert_eq!(FontRole::LatinText, classify("said “hello” end", 11, 12));
    assert_eq!(FontRole::LatinText, classify("it’s", 2, 3));
}

#[test]
fn classifies_curly_quotes_as_cjk_in_mixed_context() {
    assert_eq!(FontRole::CjkPunctuation, classify("他说“hello”", 2, 3));
    assert_eq!(FontRole::CjkPunctuation, classify("他说“hello”", 8, 9));
}

#[test]
fn classifies_ascii_brackets_as_latin() {
    for character in ['(', ')', '[', ']', '{', '}'] {
        assert_eq!(FontRole::LatinText, classify(&character.to_string(), 0, 1));
    }
    assert_eq!(FontRole::LatinText, classify("中(文", 1, 2));
}

#[test]
fn classifies_curly_quotes_as_cjk_at_text_boundary() {
    assert_eq!(FontRole::CjkPunctuation, classify("“你好”", 0, 1));
    assert_eq!(FontRole::CjkPunctuation, classify("“你好”", 3, 4));
}
