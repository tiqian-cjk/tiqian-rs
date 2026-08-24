// 对应 Kotlin 源文件：engine/src/commonMain/kotlin/org/tiqian/linebreak/LineBreak.kt

use super::super::core::Geometry::TextRange;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BreakOpportunity {
    pub index: i32,
    pub kind: BreakKind,
    pub penalty: i32,
    pub reason: String,
}

impl BreakOpportunity {
    pub fn new(index: i32, kind: BreakKind, reason: String) -> Self {
        Self {
            index,
            kind,
            penalty: 0,
            reason,
        }
    }

    pub fn with_penalty(index: i32, kind: BreakKind, penalty: i32, reason: String) -> Self {
        Self {
            index,
            kind,
            penalty,
            reason,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BreakKind {
    Allowed,
    Forbidden,
    Required,
    Problematic,
}

/**
 * UAX #14 的 mandatory-break 码点（BK / CR / LF / NL 类）：layout 必须遵循的硬换行
 *（ADR 0037，保留 source 的 plain text）。`CRLF`（U+000D U+000A）是单个换行：码点扫描器
 * 应将紧跟 CR 的 LF 视为同一个换行的一部分，而不是第二个换行。
 */
pub fn is_mandatory_break_code_point(code_point: i32) -> bool {
    matches!(
        code_point,
        0x000A // LF，换行。
            | 0x000B // VT，垂直制表。
            | 0x000C // FF，换页。
            | 0x000D // CR，回车。
            | 0x0085 // NEL，下一行。
            | 0x2028 // LS，行分隔符。
            | 0x2029 // PS，段落分隔符。
    )
}

/**
 * UAX #14 的 ZW 类：U+200B 提供 soft break opportunity，但不提供 ink 或 advance。WORD JOINER
 *（U+2060）和 ZWNBSP/BOM（U+FEFF）特意不包括在内，因为它们具有相反的禁止断行语义。
 */
pub fn is_zero_width_space_code_point(code_point: i32) -> bool {
    code_point == 0x200B
}

pub trait LineBreakAnalyzer {
    fn analyze(&self, text: &str) -> Vec<BreakOpportunity>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SimpleCharacterLineBreakAnalyzer;

impl LineBreakAnalyzer for SimpleCharacterLineBreakAnalyzer {
    fn analyze(&self, text: &str) -> Vec<BreakOpportunity> {
        let code_units: Vec<u16> = text.encode_utf16().collect();
        if code_units.is_empty() {
            return Vec::new();
        }

        let mut opportunities = Vec::with_capacity(code_units.len());
        for index in 1..=code_units.len() {
            let previous = code_units[index - 1] as i32;
            // mandatory-break 字符强制其后的 Required break；但 CRLF 中的 CR 例外，
            // 换行属于紧随其后的 LF。
            let mandatory = is_mandatory_break_code_point(previous)
                && !(previous == 0x000D
                    && index < code_units.len()
                    && code_units[index] as i32 == 0x000A);
            opportunities.push(BreakOpportunity::new(
                index as i32,
                if index == code_units.len() || mandatory {
                    BreakKind::Required
                } else {
                    BreakKind::Allowed
                },
                if mandatory {
                    "MandatoryBreak".to_owned()
                } else {
                    "SimpleCharacterLineBreakAnalyzer".to_owned()
                },
            ));
        }
        opportunities
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForbiddenBreak {
    pub range: TextRange,
    pub reason: String,
}
