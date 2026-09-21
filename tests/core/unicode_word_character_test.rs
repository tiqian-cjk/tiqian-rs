use tiqian::core::unicode_word_character::unicode_word_character;

#[test]
fn letters_and_numbers_are_word_characters_across_scripts() {
    for character in [
        'A',
        '2',
        '中',
        '\u{0301}',
        'π',
        'Ж',
        '\u{0662}',
        '\u{20000}',
    ] {
        assert!(
            unicode_word_character::contains(character),
            "U+{:04X}",
            character as u32,
        );
    }
    for character in [' ', '\u{2019}', '？', '😀'] {
        assert!(
            !unicode_word_character::contains(character),
            "U+{:04X}",
            character as u32,
        );
    }
}
