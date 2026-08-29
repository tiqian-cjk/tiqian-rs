// 对应 Kotlin 源文件：engine/src/commonMain/kotlin/org/tiqian/layout/ContextualQuoteRoleResolver.kt

use crate::common::{HashMap, HashSet};

use super::super::core::east_asian_spacing::unicode_east_asian_spacing;
use super::super::core::text::Text;
use super::super::core::unicode_script_evidence::{
    UnicodeScriptEvidence, unicode_script_evidence_classifier,
};
use super::super::font::font_policy::{FontRole, FontRoleContext};
use super::quote_pair_analyzer::{QuotePair, QuoteRoleDecision, is_non_cjk_in_word_apostrophe};

/**
 * 从完整引号结构解析共享弯引号码点的字体角色。
 *
 * Unicode Script 将引号归入 Common，并建议按所在层级解析成对标点。因此本解析器不会让相邻的
 * 单个字符决定整对引号，而会依次考虑：
 *
 * 1. 成对引号所在层级两侧的强脚本文本；
 * 2. 已完成解析的外层引号；
 * 3. 引号对内部的全部强脚本文本；
 * 4. 文本混合或没有证据时的段落语言。
 *
 * 由空白分隔、且内部完全不含 CJK 的引号对仍作为独立 Western inline run。这样会保留
 * `（如 ‘O’, ‘Q’）` 的作者拼写，同时不让混合中文引号起始处的 Latin identifier 接管整对引号。
 */
pub struct ContextualQuoteRoleResolver<'a> {
    text: &'a Text,
    pairs: &'a [QuotePair],
    context: &'a FontRoleContext,
    pair_by_open: HashMap<i32, QuotePair>,
    pair_by_close: HashMap<i32, QuotePair>,
    parent_by_pair: HashMap<QuotePair, Option<QuotePair>>,
}

impl<'a> ContextualQuoteRoleResolver<'a> {
    pub fn new(text: &'a Text, pairs: &'a [QuotePair], context: &'a FontRoleContext) -> Self {
        let pair_by_open = pairs
            .iter()
            .copied()
            .map(|pair| (pair.open_index, pair))
            .collect();
        let pair_by_close = pairs
            .iter()
            .copied()
            .map(|pair| (pair.close_index, pair))
            .collect();
        let parent_by_pair = pairs
            .iter()
            .copied()
            .map(|pair| (pair, Self::find_parent(pairs, pair)))
            .collect();
        Self {
            text,
            pairs,
            context,
            pair_by_open,
            pair_by_close,
            parent_by_pair,
        }
    }

    pub fn resolve(&self) -> Vec<QuoteRoleDecision> {
        let mut decisions = Vec::new();
        let mut resolved_pairs = HashMap::new();
        let mut sorted_pairs = self.pairs.to_vec();
        sorted_pairs.sort_by(|left, right| {
            left.open_index
                .cmp(&right.open_index)
                .then_with(|| right.close_index.cmp(&left.close_index))
        });

        for pair in sorted_pairs {
            let decision = self.resolve_pair(pair, &resolved_pairs);
            resolved_pairs.insert(pair, decision.role);
            decisions.push(QuoteRoleDecision::new(
                pair.open_index,
                decision.role,
                decision.source.clone(),
                decision.reason.clone(),
            ));
            decisions.push(QuoteRoleDecision::new(
                pair.close_index,
                decision.role,
                decision.source,
                decision.reason,
            ));
        }

        let paired_indices: HashSet<i32> = self
            .pairs
            .iter()
            .flat_map(|pair| [pair.open_index, pair.close_index])
            .collect();
        let text_length = self.text.utf16_len();
        for index in 0..text_length {
            if paired_indices.contains(&index)
                || !is_ambiguous_curly_quote(self.text.utf16_code_unit_at(index))
            {
                continue;
            }
            let decision = self.resolve_unmatched(index);
            decisions.push(QuoteRoleDecision::new(
                index,
                decision.role,
                decision.source,
                decision.reason,
            ));
        }

        decisions.sort_by_key(|decision| decision.index);
        decisions
    }

    fn resolve_pair(
        &self,
        pair: QuotePair,
        resolved_pairs: &HashMap<QuotePair, FontRole>,
    ) -> Resolution {
        let parent = self.parent_by_pair[&pair];
        let enclosing_start = parent.map_or(0, |parent_pair| parent_pair.open_index + 1);
        let enclosing_end =
            parent.map_or(self.text.utf16_len(), |parent_pair| parent_pair.close_index);
        let mut outer_evidence = ScriptEvidence::default();
        self.add_script_evidence_range(&mut outer_evidence, enclosing_start, pair.open_index);
        self.add_script_evidence_range(&mut outer_evidence, pair.close_index + 1, enclosing_end);
        let mut content_evidence = ScriptEvidence::default();
        self.add_script_evidence_range(
            &mut content_evidence,
            pair.open_index + 1,
            pair.close_index,
        );

        if self
            .text
            .utf16_code_unit_at_or_none(pair.open_index - 1)
            .is_some_and(is_ascii_space_or_tab)
            && content_evidence.has_western
            && !content_evidence.has_cjk
        {
            return Resolution::new(
                FontRole::LatinText,
                "DelimitedWesternQuotationRun",
                "whitespace-delimited-wholly-western-quotation",
            );
        }

        if let Some(role) = outer_evidence.unambiguous_role() {
            return Resolution::new(
                role,
                "PairedPunctuationOuterScriptContext",
                "quote-pair-inherits-enclosing-level-script",
            );
        }

        if outer_evidence.is_mixed() {
            return self.paragraph_language_resolution("mixed-enclosing-level-script");
        }

        if let Some(enclosing_pair) = parent
            && let Some(role) = resolved_pairs.get(&enclosing_pair)
        {
            return Resolution::new(
                *role,
                "PairedPunctuationEnclosingQuoteContext",
                "quote-pair-inherits-enclosing-quotation",
            );
        }

        if let Some(role) = content_evidence.unambiguous_role() {
            return Resolution::new(
                role,
                "PairedPunctuationContentScriptContext",
                "quoted-content-script",
            );
        }

        self.paragraph_language_resolution(if content_evidence.is_mixed() {
            "mixed-quoted-content"
        } else {
            "no-strong-script-context"
        })
    }

    fn resolve_unmatched(&self, index: i32) -> Resolution {
        if self.text.utf16_code_unit_at(index) == 0x2019
            && is_non_cjk_in_word_apostrophe(self.text, index)
        {
            return Resolution::new(
                FontRole::LatinText,
                "NonCjkInWordApostrophe",
                "non-cjk-in-word-apostrophe",
            );
        }

        let left_role = self.nearest_strong_script_role(index - 1, -1);
        let right_role = self.nearest_strong_script_role(index + 1, 1);
        if self
            .text
            .utf16_code_unit_at_or_none(index - 1)
            .is_some_and(is_ascii_space_or_tab)
            && right_role == Some(FontRole::LatinText)
        {
            return Resolution::new(
                FontRole::LatinText,
                "DelimitedUnmatchedWesternQuote",
                "whitespace-delimited-unmatched-western-quote",
            );
        }

        if let Some(left_role) = left_role
            && (right_role.is_none() || right_role == Some(left_role))
        {
            return Resolution::new(
                left_role,
                "UnmatchedQuoteSurroundingScriptContext",
                "unmatched-quote-surrounding-script",
            );
        }
        if let Some(right_role) = right_role
            && left_role.is_none()
        {
            return Resolution::new(
                right_role,
                "UnmatchedQuoteSurroundingScriptContext",
                "unmatched-quote-surrounding-script",
            );
        }
        self.paragraph_language_resolution(if left_role.is_some() && right_role.is_some() {
            "conflicting-unmatched-quote-context"
        } else {
            "no-unmatched-quote-context"
        })
    }

    fn nearest_strong_script_role(&self, start_index: i32, direction: i32) -> Option<FontRole> {
        let mut index = start_index;
        let text_length = self.text.utf16_len();
        while (0..text_length).contains(&index) {
            if direction < 0 {
                if let Some(pair) = self.pair_by_close.get(&index) {
                    index = pair.open_index - 1;
                    continue;
                }
            } else if let Some(pair) = self.pair_by_open.get(&index) {
                index = pair.close_index + 1;
                continue;
            }

            let scalar_start = if direction < 0
                && (0xDC00..=0xDFFF).contains(&self.text.utf16_code_unit_at(index))
                && index > 0
                && (0xD800..=0xDBFF).contains(&self.text.utf16_code_unit_at(index - 1))
            {
                index - 1
            } else {
                index
            };
            let scalar_length = code_point_length_at(self.text, scalar_start, text_length);
            if let Some(role) = self.strong_script_role(scalar_start, scalar_length) {
                return Some(role);
            }
            index = if direction < 0 {
                scalar_start - 1
            } else {
                scalar_start + scalar_length
            };
        }
        None
    }

    fn paragraph_language_resolution(&self, reason: &str) -> Resolution {
        let role = if unicode_east_asian_spacing::is_chinese_language_context(&self.context.locale)
        {
            FontRole::CjkPunctuation
        } else {
            FontRole::LatinText
        };
        Resolution::new(
            role,
            "ParagraphLanguageQuoteContext",
            format!("{reason}; paragraph-language={}", self.context.locale),
        )
    }

    fn find_parent(pairs: &[QuotePair], pair: QuotePair) -> Option<QuotePair> {
        pairs
            .iter()
            .copied()
            .filter(|candidate| {
                *candidate != pair
                    && candidate.open_index < pair.open_index
                    && candidate.close_index > pair.close_index
            })
            .min_by_key(|candidate| candidate.close_index - candidate.open_index)
    }

    fn add_script_evidence_range(&self, evidence: &mut ScriptEvidence, start: i32, end: i32) {
        let mut index = start;
        while index < end {
            if let Some(nested_pair) = self.pair_by_open.get(&index)
                && nested_pair.close_index < end
            {
                index = nested_pair.close_index + 1;
                continue;
            }

            let code_point_length = code_point_length_at(self.text, index, end);
            match self.strong_script_role(index, code_point_length) {
                Some(FontRole::CjkPunctuation) => evidence.has_cjk = true,
                Some(FontRole::LatinText) => evidence.has_western = true,
                _ => {}
            }
            index += code_point_length;
        }
    }

    fn strong_script_role(&self, index: i32, code_point_length: i32) -> Option<FontRole> {
        let code_point = self
            .text
            .code_point_at_compat(index, index + code_point_length);
        match unicode_script_evidence_classifier::classify(code_point) {
            UnicodeScriptEvidence::Neutral => None,
            UnicodeScriptEvidence::EastAsian => Some(FontRole::CjkPunctuation),
            UnicodeScriptEvidence::Other => Some(FontRole::LatinText),
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct ScriptEvidence {
    has_cjk: bool,
    has_western: bool,
}

impl ScriptEvidence {
    fn is_mixed(self) -> bool {
        self.has_cjk && self.has_western
    }

    fn unambiguous_role(self) -> Option<FontRole> {
        match (self.has_cjk, self.has_western) {
            (true, false) => Some(FontRole::CjkPunctuation),
            (false, true) => Some(FontRole::LatinText),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
struct Resolution {
    role: FontRole,
    source: String,
    reason: String,
}

impl Resolution {
    fn new(role: FontRole, source: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            role,
            source: source.into(),
            reason: reason.into(),
        }
    }
}

fn code_point_length_at(text: &Text, index: i32, end: i32) -> i32 {
    if text.code_point_at_compat(index, end) > 0xFFFF {
        2
    } else {
        1
    }
}

fn is_ambiguous_curly_quote(code_unit: i32) -> bool {
    matches!(code_unit, 0x2018 | 0x2019 | 0x201C | 0x201D)
}

fn is_ascii_space_or_tab(code_unit: i32) -> bool {
    matches!(code_unit, 0x0020 | 0x0009)
}
