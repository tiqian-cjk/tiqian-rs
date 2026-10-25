// 对应 Kotlin 源文件：engine/src/commonMain/kotlin/org/tiqian/core/UnicodeScriptEvidence.kt

use icu_properties::{CodePointMapData, props::Script};

/**
 * 为语言敏感的 Common 标点提供稳定的 Unicode Script 证据。
 * Common、Inherited 和未分配标量均为中性：标点、空格和 ASCII 数字
 * 不参与决定周围引号的语言。
 */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnicodeScriptEvidence {
    Neutral,
    EastAsian,
    Other,
}

pub mod unicode_script_evidence_classifier {
    use super::*;

    pub const DATA_REVISION: &str = "17.0.0";
    pub const DATA_SOURCE: &str = "https://www.unicode.org/Public/17.0.0/ucd/Scripts.txt";
    pub const DATA_SHA256: &str =
        "9f5e50d3abaee7d6ce09480f325c706f485ae3240912527e651954d2d6b035bf";

    pub fn classify(character: char) -> UnicodeScriptEvidence {
        match CodePointMapData::<Script>::new().get(character) {
            Script::Bopomofo
            | Script::Han
            | Script::Hangul
            | Script::Hiragana
            | Script::Katakana => UnicodeScriptEvidence::EastAsian,
            Script::Common | Script::Inherited | Script::Unknown => UnicodeScriptEvidence::Neutral,
            _ => UnicodeScriptEvidence::Other,
        }
    }
}
