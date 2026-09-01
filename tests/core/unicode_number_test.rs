use tiqian::core::unicode_word_character::unicode_word_character;

#[test]
fn numbers_are_members_across_scripts_and_non_scalars_are_rejected() {
    for code_point in ['0' as i32, 0x0662, '½' as i32] {
        assert!(unicode_word_character::is_number(code_point), "U+{code_point:04X}");
    }
    for code_point in ['a' as i32, '中' as i32, 0x2019] {
        assert!(!unicode_word_character::is_number(code_point), "U+{code_point:04X}");
    }
    for code_point in [0xDC00, -1, 0x110000] {
        assert!(std::panic::catch_unwind(|| unicode_word_character::is_number(code_point)).is_err());
    }
}