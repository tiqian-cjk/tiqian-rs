// 对应 Kotlin 源文件：engine/src/commonMain/kotlin/org/tiqian/linebreak/UnicodePunctuationLineBreak.kt

use icu_properties::{CodePointMapData, props::LineBreak};

/**
 * 用于保护标点边界的 UAX #14 断行类别。此处并不宣称 Tiqian 实现了完整 Unicode Line Breaking
 * Algorithm：词、数字、combining mark 与特定 script 的规则仍位于现有 pipeline stage 中。
 */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnicodePunctuationLineBreakClass {
    BreakAfter,
    BreakBoth,
    ClosePunctuation,
    CloseParenthesis,
    Exclamation,
    HyphenHH,
    Hyphen,
    Inseparable,
    InfixNumericSeparator,
    Nonstarter,
    OpenPunctuation,
    Quotation,
    SymbolsAllowingBreakAfter,
    Other,
}

/// layout layer 使用的 Unicode 17.0.0 标点属性，使可裁剪的 UAX #14 标点边界独立于字体选择。
pub mod unicode_punctuation_line_break {
    use super::*;

    pub const DATA_REVISION: &str = "17.0.0";
    pub const DATA_SOURCE: &str = "https://www.unicode.org/Public/17.0.0/ucd/LineBreak.txt";
    pub const DATA_SHA256: &str =
        "e6a18fa91f8f6a6f8e534b1d3f128c21ada45bfe152eb6b1bcc5e15fd8ac92e6";

    pub fn class_of(code_point: i32) -> UnicodePunctuationLineBreakClass {
        assert!(
            (0..=0x10FFFF).contains(&code_point),
            "Not a Unicode scalar value: {code_point}"
        );
        assert!(
            !(0xD800..=0xDFFF).contains(&code_point),
            "Surrogate is not a Unicode scalar value: {code_point}"
        );
        match CodePointMapData::<LineBreak>::new().get32(code_point as u32) {
            LineBreak::BreakAfter => UnicodePunctuationLineBreakClass::BreakAfter,
            LineBreak::BreakBoth => UnicodePunctuationLineBreakClass::BreakBoth,
            LineBreak::ClosePunctuation => UnicodePunctuationLineBreakClass::ClosePunctuation,
            LineBreak::CloseParenthesis => UnicodePunctuationLineBreakClass::CloseParenthesis,
            LineBreak::Exclamation => UnicodePunctuationLineBreakClass::Exclamation,
            LineBreak::UnambiguousHyphen => UnicodePunctuationLineBreakClass::HyphenHH,
            LineBreak::Hyphen => UnicodePunctuationLineBreakClass::Hyphen,
            LineBreak::Inseparable => UnicodePunctuationLineBreakClass::Inseparable,
            LineBreak::InfixNumeric => {
                UnicodePunctuationLineBreakClass::InfixNumericSeparator
            }
            LineBreak::Nonstarter => UnicodePunctuationLineBreakClass::Nonstarter,
            LineBreak::OpenPunctuation => UnicodePunctuationLineBreakClass::OpenPunctuation,
            LineBreak::Quotation => UnicodePunctuationLineBreakClass::Quotation,
            LineBreak::BreakSymbols => {
                UnicodePunctuationLineBreakClass::SymbolsAllowingBreakAfter
            }
            _ => UnicodePunctuationLineBreakClass::Other,
        }
    }
}
