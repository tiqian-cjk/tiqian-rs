// 对应 Kotlin 源文件：engine/src/commonMain/kotlin/org/tiqian/layout/QuotePairAnalyzer.kt

use std::collections::HashMap;

use super::super::core::Geometry::TextRange;
use super::super::core::Text::Text;
use super::super::font::FontPolicy::{FontRole, FontRoleClassifier, FontRoleContext};
use super::ContextualQuoteRoleResolver::ContextualQuoteRoleResolver;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct QuotePair {
    pub open_index: i32,
    pub close_index: i32,
    pub quote_type: QuoteType,
}

impl QuotePair {
    pub fn new(open_index: i32, close_index: i32, quote_type: QuoteType) -> Self {
        Self {
            open_index,
            close_index,
            quote_type,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum QuoteType {
    Double,
    Single,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuoteRoleDecision {
    pub index: i32,
    pub role: FontRole,
    pub source: String,
    pub reason: String,
}

impl QuoteRoleDecision {
    pub fn new(index: i32, role: FontRole, source: String, reason: String) -> Self {
        Self {
            index,
            role,
            source,
            reason,
        }
    }
}

/**
 * 找出结构化配对的弯引号，并将其脚本角色委托给 [`ContextualQuoteRoleResolver`]。
 *
 * 配对和语言/脚本解析保持分离，使任何一个阶段都不必猜测另一个阶段的状态。
 */
#[derive(Clone, Copy, Debug, Default)]
pub struct QuotePairAnalyzer;

impl QuotePairAnalyzer {
    pub fn analyze(&self, text: &Text) -> Vec<QuotePair> {
        let mut stack = Vec::new();
        let mut pairs = Vec::new();
        let text_length = text.utf16_len();
        for index in 0..text_length {
            match text.utf16_code_unit_at(index) {
                0x201C => stack.push((index, QuoteType::Double)),
                0x2018 => stack.push((index, QuoteType::Single)),
                0x201D
                    if stack
                        .last()
                        .is_some_and(|(_, quote_type)| *quote_type == QuoteType::Double) =>
                {
                    let (open_index, quote_type) = stack.pop().expect("stack was checked above");
                    pairs.push(QuotePair::new(open_index, index, quote_type));
                }
                0x2019
                    if !is_non_cjk_in_word_apostrophe(text, index)
                        && stack
                            .last()
                            .is_some_and(|(_, quote_type)| *quote_type == QuoteType::Single) =>
                {
                    let (open_index, quote_type) = stack.pop().expect("stack was checked above");
                    pairs.push(QuotePair::new(open_index, index, quote_type));
                }
                _ => {}
            }
        }
        pairs
    }

    pub fn classify_pairs(
        &self,
        text: &Text,
        pairs: &[QuotePair],
        context: &FontRoleContext,
    ) -> HashMap<i32, FontRole> {
        self.classify_quote_roles(text, pairs, context)
            .into_iter()
            .map(|decision| (decision.index, decision.role))
            .collect()
    }

    /**
     * 为 first alpha 调用方保留的 source-compatible 入口。脚本证据现由 Unicode 定义，
     * 不再委托给字体分类器，因此有意忽略 `font_role_classifier`。
     */
    pub fn classify_pairs_with_font_role_classifier(
        &self,
        text: &Text,
        pairs: &[QuotePair],
        _font_role_classifier: &dyn FontRoleClassifier,
        context: &FontRoleContext,
    ) -> HashMap<i32, FontRole> {
        self.classify_pairs(text, pairs, context)
    }

    pub fn classify_quote_roles(
        &self,
        text: &Text,
        pairs: &[QuotePair],
        context: &FontRoleContext,
    ) -> Vec<QuoteRoleDecision> {
        ContextualQuoteRoleResolver::new(text, pairs, context).resolve()
    }

    /** `classify_pairs` 的 source-compatible 对应入口。 */
    pub fn classify_quote_roles_with_font_role_classifier(
        &self,
        text: &Text,
        pairs: &[QuotePair],
        _font_role_classifier: &dyn FontRoleClassifier,
        context: &FontRoleContext,
    ) -> Vec<QuoteRoleDecision> {
        self.classify_quote_roles(text, pairs, context)
    }
}

pub fn is_non_cjk_in_word_apostrophe(text: &Text, index: i32) -> bool {
    text.code_point_before(index)
        .is_some_and(is_non_cjk_word_character)
        && text
            .code_point_at_or_none(index + 1)
            .is_some_and(is_non_cjk_word_character)
}

fn is_non_cjk_word_character(code_point: i32) -> bool {
    super::super::core::UnicodeWordCharacter::unicode_word_character::contains(code_point)
        && super::super::core::UnicodeScriptEvidence::unicode_script_evidence_classifier::classify(
            code_point,
        ) != super::super::core::UnicodeScriptEvidence::UnicodeScriptEvidence::EastAsian
}

/**
 * 仅对本文件的 `QuotePairAwareFontRoleClassifier` 覆盖已为当前段落解析的弯引号。
 * 每个 override 仍可通过 [`QuoteRoleDecision`] 追踪。
 */
pub struct QuotePairAwareFontRoleClassifier<'a> {
    delegate: &'a dyn FontRoleClassifier,
    quote_roles: &'a HashMap<i32, FontRole>,
}

impl<'a> QuotePairAwareFontRoleClassifier<'a> {
    pub fn new(
        delegate: &'a dyn FontRoleClassifier,
        quote_roles: &'a HashMap<i32, FontRole>,
    ) -> Self {
        Self {
            delegate,
            quote_roles,
        }
    }
}

impl FontRoleClassifier for QuotePairAwareFontRoleClassifier<'_> {
    fn classify(&self, text: &Text, range: TextRange, context: &FontRoleContext) -> FontRole {
        self.quote_roles
            .get(&range.start())
            .copied()
            .unwrap_or_else(|| self.delegate.classify(text, range, context))
    }
}
