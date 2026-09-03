// 对应 Kotlin 源文件：engine/src/commonMain/kotlin/org/tiqian/layout/ContextualDashEllipsisRoleResolver.kt

use crate::common::HashMap;

use super::super::core::east_asian_spacing::unicode_east_asian_spacing;
use super::super::core::geometry::{ScalarOffset, TextRange};
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
        let pair_resolutions =
            resolve_parenthetical_pairs(text, &runs, &strong_script_context, context);
        runs.into_iter()
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
    roles_by_index: HashMap<ScalarOffset, FontRole>,
}

impl<'a> ContextualDashEllipsisAwareFontRoleClassifier<'a> {
    pub fn new(
        delegate: &'a dyn FontRoleClassifier,
        decisions: &[DashEllipsisRoleDecision],
    ) -> Self {
        let mut roles_by_index = HashMap::new();
        for decision in decisions {
            let mut index = decision.range.start();
            while index < decision.range.end() {
                roles_by_index.insert(index, decision.role);
                index += 1;
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

/// Resolves contextual U+2014 and U+2026 roles for callers classifying ranges from one complete
/// paragraph outside the layout pipeline. `Passthrough` preserves the supplied classifier when
/// the paragraph has no contextual dash or ellipsis decisions.
pub enum ContextualDashEllipsisFontRoleClassifier<'a> {
    Passthrough(&'a dyn FontRoleClassifier),
    Overrides {
        delegate: &'a dyn FontRoleClassifier,
        roles_by_index: HashMap<ScalarOffset, FontRole>,
    },
}

impl FontRoleClassifier for ContextualDashEllipsisFontRoleClassifier<'_> {
    fn classify(&self, text: &Text, range: TextRange, context: &FontRoleContext) -> FontRole {
        match self {
            Self::Passthrough(delegate) => delegate.classify(text, range, context),
            Self::Overrides {
                delegate,
                roles_by_index,
            } => roles_by_index
                .get(&range.start())
                .copied()
                .unwrap_or_else(|| delegate.classify(text, range, context)),
        }
    }
}

/// Creates a contextual dash/ellipsis classifier for one complete paragraph.
pub fn with_contextual_dash_ellipsis_roles<'a>(
    delegate: &'a dyn FontRoleClassifier,
    text: &Text,
    context: &FontRoleContext,
) -> ContextualDashEllipsisFontRoleClassifier<'a> {
    let decisions = ContextualDashEllipsisRoleResolver.resolve(text, context);
    if decisions.is_empty() {
        ContextualDashEllipsisFontRoleClassifier::Passthrough(delegate)
    } else {
        let mut roles_by_index = HashMap::new();
        for decision in decisions {
            let mut index = decision.range.start();
            while index < decision.range.end() {
                roles_by_index.insert(index, decision.role);
                index += 1;
            }
        }
        ContextualDashEllipsisFontRoleClassifier::Overrides {
            delegate,
            roles_by_index,
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

fn collect_runs(text: &Text) -> Vec<TextRange> {
    let mut runs = Vec::new();
    let mut start = None;
    for (offset, character) in text.scalar_indices() {
        if matches!(character, '\u{2014}' | '\u{2026}') {
            start.get_or_insert(offset);
        } else if let Some(start) = start.take() {
            runs.push(TextRange::new(start, offset));
        }
    }
    if let Some(start) = start {
        runs.push(TextRange::new(start, text.scalar_len()));
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
    text.slice_text(TextRange::new(first.end(), second.start()))
        .chars()
        .all(|character| character == ' ' || unicode_word_character::contains(character as i32))
}

fn is_pure_dash_run(text: &Text, range: TextRange) -> bool {
    text.slice_text(range).chars().all(|character| character == '\u{2014}')
}

struct StrongScriptContextIndex {
    left_role_before_boundary: Vec<Option<FontRole>>,
    right_role_from_boundary: Vec<Option<FontRole>>,
}

impl StrongScriptContextIndex {
    fn new(text: &Text) -> Self {
        let text_length = text.scalar_len();
        let mut left_role_before_boundary = vec![None; text_length.value() as usize + 1];
        let mut current_role = None;
        for (offset, character) in text.scalar_indices() {
            let scalar_end = offset + 1;
            current_role = next_strong_script_role(character as i32, current_role);
            left_role_before_boundary[scalar_end.value() as usize] = current_role;
        }

        let mut right_role_from_boundary = vec![None; text_length.value() as usize + 1];
        current_role = None;
        let mut scalar_end = text_length;
        while scalar_end > ScalarOffset::ZERO {
            let scalar_start = scalar_end - 1;
            let code_point = text
                .code_point_at_or_none(scalar_start)
                .expect("scalar offset must be valid");
            current_role = next_strong_script_role(code_point, current_role);
            right_role_from_boundary[scalar_start.value() as usize] = current_role;
            scalar_end = scalar_start;
        }
        Self {
            left_role_before_boundary,
            right_role_from_boundary,
        }
    }

    fn left_of(&self, boundary: ScalarOffset) -> Option<FontRole> {
        self.left_role_before_boundary[boundary.value() as usize]
    }

    fn right_of(&self, boundary: ScalarOffset) -> Option<FontRole> {
        self.right_role_from_boundary[boundary.value() as usize]
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

