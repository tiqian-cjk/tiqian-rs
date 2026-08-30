// 对应 Kotlin 源文件：engine/src/commonMain/kotlin/org/tiqian/core/UnicodeWordCharacter.kt

use icu_properties::{CodePointMapData, props::GeneralCategory};

/// 为词法边界提供稳定的 Unicode 17 Letter/Mark/Number 成员判断。
pub mod unicode_word_character {
    use super::*;

    pub const DATA_REVISION: &str = "17.0.0";
    pub const DATA_SOURCE: &str =
        "https://www.unicode.org/Public/17.0.0/ucd/extracted/DerivedGeneralCategory.txt";
    pub const DATA_SHA256: &str =
        "d62e5bab70ca74f099343f71224fa051cb1fdd61a1ab45c0488c44cfc0b6102e";

    pub fn contains(code_point: i32) -> bool {
        assert!(
            (0..=0x10FFFF).contains(&code_point),
            "Not a Unicode scalar value: {code_point}"
        );
        assert!(
            !(0xD800..=0xDFFF).contains(&code_point),
            "Surrogate is not a Unicode scalar value: {code_point}"
        );
        matches!(
            CodePointMapData::<GeneralCategory>::new().get32(code_point as u32),
            GeneralCategory::UppercaseLetter
                | GeneralCategory::LowercaseLetter
                | GeneralCategory::TitlecaseLetter
                | GeneralCategory::ModifierLetter
                | GeneralCategory::OtherLetter
                | GeneralCategory::NonspacingMark
                | GeneralCategory::SpacingMark
                | GeneralCategory::EnclosingMark
                | GeneralCategory::DecimalNumber
                | GeneralCategory::LetterNumber
                | GeneralCategory::OtherNumber
        )
    }

    pub fn is_number(code_point: i32) -> bool {
        assert!(
            (0..=0x10FFFF).contains(&code_point),
            "Not a Unicode scalar value: {code_point}"
        );
        assert!(
            !(0xD800..=0xDFFF).contains(&code_point),
            "Surrogate is not a Unicode scalar value: {code_point}"
        );
        matches!(
            CodePointMapData::<GeneralCategory>::new().get32(code_point as u32),
            GeneralCategory::DecimalNumber
                | GeneralCategory::LetterNumber
                | GeneralCategory::OtherNumber
        )
    }
}
