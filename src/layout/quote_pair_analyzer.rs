// 对应 Kotlin 源文件：engine/src/commonMain/kotlin/org/tiqian/layout/QuotePairAnalyzer.kt

use crate::common::HashMap;
use icu_properties::{CodePointMapData, props::EastAsianWidth};

use super::super::core::geometry::{ScalarOffset, TextRange, scalar_offset};
use super::super::core::text::Text;
use super::super::font::font_policy::{FontRole, FontRoleClassifier, FontRoleContext};
use super::contextual_quote_role_resolver::ContextualQuoteRoleResolver;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct QuotePair {
    pub open_index: ScalarOffset,
    pub close_index: ScalarOffset,
    pub quote_type: QuoteType,
}

impl QuotePair {
    pub fn new(open_index: ScalarOffset, close_index: ScalarOffset, quote_type: QuoteType) -> Self {
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
    pub index: ScalarOffset,
    pub role: FontRole,
    pub source: String,
    pub reason: String,
}

impl QuoteRoleDecision {
    pub fn new(index: ScalarOffset, role: FontRole, source: String, reason: String) -> Self {
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
        for (raw_index, character) in text.chars().enumerate() {
            let index = scalar_offset(raw_index as i32);
            match character as u32 {
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
    ) -> HashMap<ScalarOffset, FontRole> {
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
    ) -> HashMap<ScalarOffset, FontRole> {
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

pub fn is_non_cjk_in_word_apostrophe(text: &Text, index: ScalarOffset) -> bool {
    let Some(before) = text.code_point_before(index) else {
        return false;
    };
    let Some(after) = text.code_point_at_or_none(index + 1) else {
        return false;
    };
    // Digits alone stay neutral, so `1‘2’3` keeps its single quotes pairable
    // while `don’t` and `90’s` remain in-word apostrophes.
    is_non_cjk_word_character(before)
        && is_non_cjk_word_character(after)
        && (is_non_cjk_non_numeric_word_character(before)
            || is_non_cjk_non_numeric_word_character(after))
}

pub fn is_digit_bound_closing_quote(text: &Text, index: ScalarOffset) -> bool {
    matches!(text.code_point_at_or_none(index), Some(0x2019 | 0x201D))
        && text.code_point_before(index).is_some_and(
            super::super::core::unicode_word_character::unicode_word_character::is_number,
        )
}

pub fn is_non_cjk_word_internal_quote_pair(text: &Text, pair: QuotePair) -> bool {
    if !(text
        .code_point_before(pair.open_index)
        .is_some_and(is_non_cjk_non_numeric_word_character)
        && text
            .code_point_at_or_none(pair.close_index + 1)
            .is_some_and(is_non_cjk_non_numeric_word_character))
    {
        return false;
    }

    let mut index = pair.open_index + 1;
    while index < pair.close_index {
        let Some(code_point) = text.code_point_at_or_none(index) else {
            return false;
        };
        if !is_non_cjk_word_character(code_point) {
            return false;
        }
        index += 1;
    }
    true
}

fn is_non_cjk_word_character(code_point: i32) -> bool {
    super::super::core::unicode_word_character::unicode_word_character::contains(code_point)
        && super::super::core::unicode_script_evidence::unicode_script_evidence_classifier::classify(
            code_point,
        ) != super::super::core::unicode_script_evidence::UnicodeScriptEvidence::EastAsian
}

fn is_non_cjk_non_numeric_word_character(code_point: i32) -> bool {
    is_non_cjk_word_character(code_point)
        && !super::super::core::unicode_word_character::unicode_word_character::is_number(
            code_point,
        )
        && CodePointMapData::<EastAsianWidth>::new().get32(code_point as u32)
            != EastAsianWidth::Fullwidth
}

/**
 * 仅对本文件的 `QuotePairAwareFontRoleClassifier` 覆盖已为当前段落解析的弯引号。
 * 每个 override 仍可通过 [`QuoteRoleDecision`] 追踪。
 */
pub struct QuotePairAwareFontRoleClassifier<'a> {
    delegate: &'a dyn FontRoleClassifier,
    quote_roles: &'a HashMap<ScalarOffset, FontRole>,
}

impl<'a> QuotePairAwareFontRoleClassifier<'a> {
    pub fn new(
        delegate: &'a dyn FontRoleClassifier,
        quote_roles: &'a HashMap<ScalarOffset, FontRole>,
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

/// Resolves contextual curly-quote roles for callers classifying ranges from one complete
/// paragraph outside the layout pipeline. `Passthrough` preserves the supplied classifier when
/// the paragraph has no quote role decisions.
pub enum ContextualQuoteFontRoleClassifier<'a> {
    Passthrough(&'a dyn FontRoleClassifier),
    Overrides {
        delegate: &'a dyn FontRoleClassifier,
        quote_roles: HashMap<ScalarOffset, FontRole>,
    },
}

impl FontRoleClassifier for ContextualQuoteFontRoleClassifier<'_> {
    fn classify(&self, text: &Text, range: TextRange, context: &FontRoleContext) -> FontRole {
        match self {
            Self::Passthrough(delegate) => delegate.classify(text, range, context),
            Self::Overrides {
                delegate,
                quote_roles,
            } => quote_roles
                .get(&range.start())
                .copied()
                .unwrap_or_else(|| delegate.classify(text, range, context)),
        }
    }
}

/// Creates a contextual quote classifier for one complete paragraph.
pub fn with_contextual_quote_roles<'a>(
    delegate: &'a dyn FontRoleClassifier,
    text: &Text,
    context: &FontRoleContext,
) -> ContextualQuoteFontRoleClassifier<'a> {
    let analyzer = QuotePairAnalyzer;
    let decisions = analyzer.classify_quote_roles(text, &analyzer.analyze(text), context);
    if decisions.is_empty() {
        ContextualQuoteFontRoleClassifier::Passthrough(delegate)
    } else {
        ContextualQuoteFontRoleClassifier::Overrides {
            delegate,
            quote_roles: decisions
                .into_iter()
                .map(|decision| (decision.index, decision.role))
                .collect(),
        }
    }
}
