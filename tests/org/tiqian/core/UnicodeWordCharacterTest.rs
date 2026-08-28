use tiqian::org::tiqian::core::UnicodeWordCharacter::unicode_word_character;

#[test]
fn letters_and_numbers_are_word_characters_across_scripts() {
    for code_point in [
        'A' as i32,
        '2' as i32,
        '中' as i32,
        0x0301,
        0x03C0,
        0x0416,
        0x0662,
        0x20000,
    ] {
        assert!(
            unicode_word_character::contains(code_point),
            "U+{code_point:04X}"
        );
    }
    for code_point in [0x20, 0x2019, 0xFF1F, 0x1F600] {
        assert!(
            !unicode_word_character::contains(code_point),
            "U+{code_point:04X}"
        );
    }
}
