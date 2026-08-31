// 对应 Kotlin 源文件：engine/src/commonMain/kotlin/org/tiqian/layout/ContextualDashEllipsisRoleResolver.kt

use crate::common::HashMap;

use super::super::core::east_asian_spacing::unicode_east_asian_spacing;
use super::super::core::geometry::TextRange;
use super::super::core::text::Text;
use super::super::core::unicode_script_evidence::{
    UnicodeScriptEvidence, unicode_script_evidence_classifier,
};
use super::super::core::unicode_word_character::unicode_word_character;
use super::super::font::font_policy::{FontRole, FontRoleClassifier, FontRoleContext};
use super::super::linebreak::line_break::is_mandatory_break_code_point;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DashEllipsisRoleDecision {
    pub range: TextRange,
    pub role: FontRole,
    pub source: String,
    pub reason: String,
}

/**
 * `ContextualDashEllipsisRoleResolution` resolves U+2014 EM DASH and U+2026
 * HORIZONTAL ELLIPSIS from surrounding strong-script text. The number of
 * repeated marks only defines the source run; it never decides its language.
 *
 * Matching strong evidence on both sides, or the only available side, wins.
 * Conflicting or absent evidence falls back to the paragraph language. A
 * mandatory break is a hard context boundary, so an otherwise empty source
 * line cannot borrow the script of a neighbouring line.
 *
 * `ParentheticalDashPairContext`: two adjacent equal-length pure U+2014 runs
 * whose separating content is only word characters and ASCII spaces form one
 * parenthetical insertion and resolve jointly from the text outside it.
 */
#[derive(Clone, Copy, Debug, Default)]
pub struct ContextualDashEllipsisRoleResolver;

impl ContextualDashEllipsisRoleResolver {
    pub fn resolve(&self, text: &Text, context: &FontRoleContext) -> Vec<DashEllipsisRoleDecision> {
        if !text
            .chars()
            .any(|character| matches!(character, '\u{2014}' | '\u{2026}'))
        {
            return Vec::new();
        }
        let strong_script_context = StrongScriptContextIndex::new(text);
        let runs = collect_runs(text);
        let pair_resolutions = resolve_parenthetical_pairs(text, &runs, &strong_script_context, context);
        runs
            .into_iter()
            .map(|range| {
                let resolution = pair_resolutions
                    .get(&range)
                    .cloned()
                    .unwrap_or_else(|| resolve_single_run(range, &strong_script_context, context));
                DashEllipsisRoleDecision {
                    range,
                    role: resolution.role,
                    source: resolution.source,
                    reason: resolution.reason,
                }
            })
            .collect()
    }
}

pub struct ContextualDashEllipsisAwareFontRoleClassifier<'a> {
    delegate: &'a dyn FontRoleClassifier,
    roles_by_index: HashMap<i32, FontRole>,
}

impl<'a> ContextualDashEllipsisAwareFontRoleClassifier<'a> {
    pub fn new(delegate: &'a dyn FontRoleClassifier, decisions: &[DashEllipsisRoleDecision]) -> Self {
        let mut roles_by_index = HashMap::new();
        for decision in decisions {
            for index in decision.range.start()..decision.range.end() {
                roles_by_index.insert(index, decision.role);
            }
        }
        Self {
            delegate,
            roles_by_index,
        }
    }
}

impl FontRoleClassifier for ContextualDashEllipsisAwareFontRoleClassifier<'_> {
    fn classify(&self, text: &Text, range: TextRange, context: &FontRoleContext) -> FontRole {
        self.roles_by_index
            .get(&range.start())
            .copied()
            .unwrap_or_else(|| self.delegate.classify(text, range, context))
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

fn collect_runs(text: &Text) -> Vec<TextRange> {
    let mut runs = Vec::new();
    let mut index = 0;
    let text_length = text.utf16_len();
    while index < text_length {
        if !is_contextual_dash_or_ellipsis(text.code_point_at_compat(index, text_length)) {
            index += code_point_length_at(text, index, text_length);
            continue;
        }
        let start = index;
        while index < text_length && is_contextual_dash_or_ellipsis(text.code_point_at_compat(index, text_length)) {
            index += 1;
        }
        runs.push(TextRange::new(start, index));
    }
    runs
}

fn resolve_single_run(
    range: TextRange,
    strong_script_context: &StrongScriptContextIndex,
    context: &FontRoleContext,
) -> Resolution {
    let left_role = strong_script_context.left_of(range.start());
    let right_role = strong_script_context.right_of(range.end());
    match (left_role, right_role) {
        (Some(left), Some(right)) if left == right => Resolution::new(
            left,
            "DashEllipsisSurroundingScriptContext",
            "matching-surrounding-script",
        ),
        (Some(left), None) => Resolution::new(
            left,
            "DashEllipsisSurroundingScriptContext",
            "only-left-strong-script",
        ),
        (None, Some(right)) => Resolution::new(
            right,
            "DashEllipsisSurroundingScriptContext",
            "only-right-strong-script",
        ),
        _ => paragraph_language_resolution(
            context,
            if left_role.is_some() && right_role.is_some() {
                "conflicting-surrounding-script"
            } else {
                "no-strong-script-context"
            },
        ),
    }
}

fn resolve_parenthetical_pairs(
    text: &Text,
    runs: &[TextRange],
    strong_script_context: &StrongScriptContextIndex,
    context: &FontRoleContext,
) -> HashMap<TextRange, Resolution> {
    let mut resolutions = HashMap::new();
    let mut index = 0;
    while index + 1 < runs.len() {
        let first = runs[index];
        let second = runs[index + 1];
        if !is_parenthetical_dash_pair(text, first, second) {
            index += 1;
            continue;
        }
        let left_role = strong_script_context.left_of(first.start());
        let right_role = strong_script_context.right_of(second.end());
        let resolution = match (left_role, right_role) {
            (Some(left), Some(right)) if left == right => Resolution::new(
                left,
                "ParentheticalDashPairContext",
                "matching-outer-script",
            ),
            (Some(left), None) => Resolution::new(
                left,
                "ParentheticalDashPairContext",
                "only-left-outer-script",
            ),
            (None, Some(right)) => Resolution::new(
                right,
                "ParentheticalDashPairContext",
                "only-right-outer-script",
            ),
            _ => paragraph_language_resolution(
                context,
                if left_role.is_some() && right_role.is_some() {
                    "parenthetical-pair-conflicting-outer-script"
                } else {
                    "parenthetical-pair-no-outer-context"
                },
            ),
        };
        resolutions.insert(first, resolution.clone());
        resolutions.insert(second, resolution);
        index += 2;
    }
    resolutions
}

fn paragraph_language_resolution(context: &FontRoleContext, reason: &str) -> Resolution {
    let role = if unicode_east_asian_spacing::is_chinese_language_context(&context.locale) {
        FontRole::CjkPunctuation
    } else {
        FontRole::LatinText
    };
    Resolution::new(
        role,
        "ParagraphLanguageDashEllipsisContext",
        format!("{reason}; paragraph-language={}", context.locale),
    )
}

fn is_parenthetical_dash_pair(text: &Text, first: TextRange, second: TextRange) -> bool {
    if !is_pure_dash_run(text, first)
        || !is_pure_dash_run(text, second)
        || first.length() != second.length()
    {
        return false;
    }
    let mut index = first.end();
    while index < second.start() {
        let code_point = text.code_point_at_compat(index, second.start());
        if code_point != 0x20 && !unicode_word_character::contains(code_point) {
            return false;
        }
        index += code_point_length_at(text, index, second.start());
    }
    true
}

fn is_pure_dash_run(text: &Text, range: TextRange) -> bool {
    (range.start()..range.end()).all(|index| text.utf16_code_unit_at(index) == 0x2014)
}

struct StrongScriptContextIndex {
    left_role_before_boundary: Vec<Option<FontRole>>,
    right_role_from_boundary: Vec<Option<FontRole>>,
}

impl StrongScriptContextIndex {
    fn new(text: &Text) -> Self {
        let text_length = text.utf16_len();
        let mut left_role_before_boundary = vec![None; text_length as usize + 1];
        let mut current_role = None;
        let mut scalar_start = 0;
        while scalar_start < text_length {
            let scalar_end = scalar_start + code_point_length_at(text, scalar_start, text_length);
            current_role = next_strong_script_role(
                text.code_point_at_compat(scalar_start, text_length),
                current_role,
            );
            for boundary in scalar_start + 1..=scalar_end {
                left_role_before_boundary[boundary as usize] = current_role;
            }
            scalar_start = scalar_end;
        }

        let mut right_role_from_boundary = vec![None; text_length as usize + 1];
        current_role = None;
        let mut scalar_end = text_length;
        while scalar_end > 0 {
            scalar_start = scalar_start_before(text, scalar_end);
            let code_point = text.code_point_at_compat(scalar_start, scalar_end);
            current_role = next_strong_script_role(code_point, current_role);
            for boundary in scalar_start..scalar_end {
                right_role_from_boundary[boundary as usize] = current_role;
            }
            scalar_end = scalar_start;
        }
        Self {
            left_role_before_boundary,
            right_role_from_boundary,
        }
    }

    fn left_of(&self, boundary: i32) -> Option<FontRole> {
        self.left_role_before_boundary[boundary as usize]
    }

    fn right_of(&self, boundary: i32) -> Option<FontRole> {
        self.right_role_from_boundary[boundary as usize]
    }
}

fn next_strong_script_role(code_point: i32, current_role: Option<FontRole>) -> Option<FontRole> {
    if is_mandatory_break_code_point(code_point) {
        return None;
    }
    match unicode_script_evidence_classifier::classify(code_point) {
        UnicodeScriptEvidence::EastAsian => Some(FontRole::CjkPunctuation),
        UnicodeScriptEvidence::Other => Some(FontRole::LatinText),
        UnicodeScriptEvidence::Neutral => current_role,
    }
}

fn scalar_start_before(text: &Text, end_exclusive: i32) -> i32 {
    let last_index = end_exclusive - 1;
    if (0xDC00..=0xDFFF).contains(&text.utf16_code_unit_at(last_index))
        && last_index > 0
        && (0xD800..=0xDBFF).contains(&text.utf16_code_unit_at(last_index - 1))
    {
        last_index - 1
    } else {
        last_index
    }
}

fn is_contextual_dash_or_ellipsis(code_point: i32) -> bool {
    matches!(code_point, 0x2014 | 0x2026)
}

fn code_point_length_at(text: &Text, index: i32, end: i32) -> i32 {
    if text.code_point_at_compat(index, end) > 0xFFFF {
        2
    } else {
        1
    }
}