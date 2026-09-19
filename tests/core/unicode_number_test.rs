use tiqian::core::unicode_word_character::unicode_word_character;

#[test]
fn numbers_are_members_across_scripts() {
    for character in ['0', '\u{0662}', '½'] {
        assert!(unicode_word_character::is_number(character), "U+{:04X}", character as u32);
    }
    for character in ['a', '中', '\u{2019}'] {
        assert!(!unicode_word_character::is_number(character), "U+{:04X}", character as u32);
    }
}