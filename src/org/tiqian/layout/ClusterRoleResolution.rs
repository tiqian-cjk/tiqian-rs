// 对应 Kotlin 源文件：engine/src/commonMain/kotlin/org/tiqian/layout/ClusterRoleResolution.kt

use std::collections::{HashMap, HashSet};

use unicode_general_category::{GeneralCategory, get_general_category};

use super::super::clreq::ClreqProfile::{ClreqProfile, clreq_punctuation_policies};
use super::super::core::Geometry::TextRange;
use super::super::core::LayoutModel::{Cluster, RoleOverrideInfo};
use super::super::core::SourceInteractionBoundaries::interaction_boundaries;
use super::super::core::TextModel::InlineObjectSpan;
use super::super::core::TextIndex::utf16_offset_to_utf8_byte_index;
use super::super::core::UnicodeEmojiModifierBaseData;
use super::super::font::FontPolicy::{
    FontDecision, FontRole, FontRoleClassifier, FontRoleContext,
};
use super::super::font::{UnicodeEmojiData, UnicodeEmojiStyleVariationData};
use super::super::linebreak::LineBreak::{
    is_mandatory_break_code_point, is_zero_width_space_code_point,
};

/**
 * Kotlin `clusterRoleRanges` 的可选参数映射。sized span 的边界切开 Latin/coalesced 标点 run，
 * 使每个 cluster 只有一个 font size（ADR 0030）；`inline_objects_by_start` 的 key 为对象 range 的
 * UTF-16 source start。
 */
#[derive(Clone, Debug, Default)]
pub struct ClusterRoleRangeOptions {
    pub span_boundaries: HashSet<i32>,
    /// Boundaries that must interrupt emoji grapheme shaping because they
    /// carry a distinct layout style or occupied inline geometry. Ordinary
    /// source boundaries remain in `span_boundaries` for exact geometry, but
    /// do not belong here.
    pub emoji_shaping_boundaries: HashSet<i32>,
    pub inline_objects_by_start: HashMap<i32, InlineObjectSpan>,
}

impl ClusterRoleRangeOptions {
    pub fn builder() -> ClusterRoleRangeOptionsBuilder {
        ClusterRoleRangeOptionsBuilder {
            options: Self::default(),
        }
    }
}

pub struct ClusterRoleRangeOptionsBuilder {
    options: ClusterRoleRangeOptions,
}

impl ClusterRoleRangeOptionsBuilder {
    pub fn span_boundaries(mut self, value: HashSet<i32>) -> Self {
        self.options.emoji_shaping_boundaries = value.clone();
        self.options.span_boundaries = value;
        self
    }

    pub fn emoji_shaping_boundaries(mut self, value: HashSet<i32>) -> Self {
        self.options.emoji_shaping_boundaries = value;
        self
    }

    pub fn inline_objects_by_start(mut self, value: HashMap<i32, InlineObjectSpan>) -> Self {
        self.options.inline_objects_by_start = value;
        self
    }

    pub fn build(self) -> ClusterRoleRangeOptions {
        self.options
    }
}

pub fn cluster_role_ranges(
    text: &str,
    classifier: &dyn FontRoleClassifier,
    context: &FontRoleContext,
    profile: &ClreqProfile,
) -> Vec<ResolvedClusterRange> {
    cluster_role_ranges_with_options(
        text,
        classifier,
        context,
        profile,
        &ClusterRoleRangeOptions::default(),
    )
}

pub fn cluster_role_ranges_with_options(
    text: &str,
    classifier: &dyn FontRoleClassifier,
    context: &FontRoleContext,
    profile: &ClreqProfile,
    options: &ClusterRoleRangeOptions,
) -> Vec<ResolvedClusterRange> {
    let text_length = utf16_length(text);
    let source_grapheme_boundaries = interaction_boundaries(text, TextRange::new(0, text_length));
    let coalesce_set = &profile.coalesce_repeatable_punctuation;
    let mut ranges = Vec::new();
    let mut index = 0_i32;
    while index < text_length {
        if let Some(inline_object) = options.inline_objects_by_start.get(&index) {
            ranges.push(ResolvedClusterRange::new(
                inline_object.range,
                FontRole::Unknown,
            ));
            index = inline_object.range.end();
            continue;
        }

        let code_point = code_point_at_compat(text, index);
        let code_point_length = char_count(code_point);
        let start = index;
        if is_mandatory_break_code_point_at(code_point, text, index) {
            let end = if code_point == 0x000D
                && index + 1 < text_length
                && utf16_code_unit_at(text, index + 1) == 0x000A
            {
                index + 2
            } else {
                index + code_point_length
            };
            ranges.push(ResolvedClusterRange::mandatory_break(TextRange::new(
                start, end,
            )));
            index = end;
            continue;
        }
        if is_zero_width_space_code_point(code_point) {
            let end = index + code_point_length;
            ranges.push(ResolvedClusterRange::zero_width_soft_break(TextRange::new(
                start, end,
            )));
            index = end;
            continue;
        }

        let first_range = TextRange::new(start, start + code_point_length);
        let grapheme_end = source_grapheme_boundaries
            [source_grapheme_boundaries.partition_point(|boundary| *boundary <= start)];
        let classified_role = classifier.classify(text, first_range, context);
        let promotion_reason = emoji_role_promotion_reason(text, start, grapheme_end);
        let role = if classified_role == FontRole::Emoji || promotion_reason.is_some() {
            FontRole::Emoji
        } else {
            classified_role
        };
        let previous_range = ranges.last();
        let attached_ascii_point_mark = role == FontRole::LatinText
            && is_ascii_point_mark_code_point(code_point)
            && previous_range.is_some_and(|previous| {
                previous.role != FontRole::Unknown
                    && previous.range.end() == start
                    && !is_whitespace_code_unit(utf16_code_unit_at(text, previous.range.end() - 1))
            });

        index += code_point_length;
        if role == FontRole::Emoji {
            // `EmojiGraphemeShapingAtomicity`: source graphemes preserve modifier,
            // variation-selector, keycap, RI-pair, tag, and ZWJ shaping context. A real
            // layout style/object edge still wins because ShapingInput holds one TextStyle;
            // geometry-only source boundaries deliberately do not participate here.
            index = options
                .emoji_shaping_boundaries
                .iter()
                .copied()
                .filter(|boundary| *boundary > start && *boundary < grapheme_end)
                .min()
                .unwrap_or(grapheme_end);
        } else if role == FontRole::LatinText {
            if attached_ascii_point_mark {
                // `AttachedAsciiPointMarkSegmentation`：保持前导点号 run 独立于后续 Latin text，
                // 这样 kinsoku 无须移动整个 `,anyway` token。
                while index < text_length && !options.span_boundaries.contains(&index) {
                    let next_code_point = code_point_at_compat(text, index);
                    if !is_ascii_point_mark_code_point(next_code_point) {
                        break;
                    }
                    index += char_count(next_code_point);
                }
            } else {
                // Latin run 或 coalesced 标点 run 内的 sized-span edge 在此结束 cluster，
                // 从而令每个 cluster 携带单个 font size（ADR 0030）。
                while index < text_length && !options.span_boundaries.contains(&index) {
                    let next_code_point = code_point_at_compat(text, index);
                    let next_char_count = char_count(next_code_point);
                    let next_range = TextRange::new(index, index + next_char_count);
                    if classifier.classify(text, next_range, context) != FontRole::LatinText
                        || emoji_role_promotion_reason(text, index, text_length).is_some()
                    {
                        break;
                    }
                    index += next_char_count;
                }
            }
        } else if role == FontRole::CjkPunctuation && coalesce_set.contains(&code_point) {
            while index < text_length && !options.span_boundaries.contains(&index) {
                let next_code_point = code_point_at_compat(text, index);
                if next_code_point != code_point {
                    break;
                }
                index += char_count(next_code_point);
            }
        }

        /*
         * `GraphemeExtendStaysWithBaseCluster`：common code 可通过 Char.category 分类 BMP Mn/Mc/Me；
         * BMP 与 supplementary variation selector 会显式覆盖。若将这些 extender shape 为独立的
         * Unknown run，会丢失 base context 并产生合理的 zero advance，而 web capability validation
         * 会将其错认为损坏的 visible glyph。保持 source range 完整，让 base 与每个受覆盖的 extending
         * mark 通过同一个 font decision 与 shaping call。其他 supplementary combining category 有意
         * 处于这个窄辅助函数范围之外。
         */
        while index < text_length && !options.span_boundaries.contains(&index) {
            let extender = code_point_at_compat(text, index);
            if !is_combining_mark_code_point(extender)
                && !is_variation_selector_code_point(extender)
            {
                break;
            }
            index += char_count(extender);
        }

        let range = TextRange::new(start, index);
        ranges.push(ResolvedClusterRange::with_role_override(
            range,
            role,
            (role == FontRole::Emoji && classified_role != FontRole::Emoji).then(|| {
                RoleOverrideInfo {
                    range,
                    source_text: source_slice(text, range).to_owned(),
                    original_role: format!("{classified_role:?}"),
                    overridden_role: format!("{role:?}"),
                    source: "UnicodeEmojiSequenceRolePromotion".to_owned(),
                    reason: promotion_reason
                        .unwrap_or("EmojiPresentationCodePoint")
                        .to_owned(),
                }
            }),
        ));
    }
    ranges
}

pub fn require_covered_by(clusters: &[Cluster], font_decisions: &[FontDecision]) {
    let mut cluster_index = 0_usize;
    for decision in font_decisions {
        while cluster_index < clusters.len()
            && clusters[cluster_index].range.end() <= decision.range.start()
        {
            cluster_index += 1;
        }
        let mut cursor = decision.range.start();
        while cluster_index < clusters.len()
            && clusters[cluster_index].range.start() < decision.range.end()
        {
            let cluster = &clusters[cluster_index];
            assert!(
                is_inside(cluster.range, decision.range),
                "TextShaper returned cluster {} crossing {}",
                kotlin_text_range_string(cluster.range),
                kotlin_text_range_string(decision.range)
            );
            assert!(
                cluster.range.start() == cursor,
                "TextShaper returned non-contiguous clusters for {}; expected start={cursor}, actual={}",
                kotlin_text_range_string(decision.range),
                cluster.range.start()
            );
            cursor = cluster.range.end();
            cluster_index += 1;
        }
        assert!(
            cursor == decision.range.end(),
            "TextShaper must return clusters covering {}; coveredUntil={cursor}",
            kotlin_text_range_string(decision.range)
        );
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedClusterRange {
    pub range: TextRange,
    pub role: FontRole,
    pub mandatory_break: bool,
    pub zero_width_soft_break: bool,
    pub role_override: Option<RoleOverrideInfo>,
}

impl ResolvedClusterRange {
    pub fn new(range: TextRange, role: FontRole) -> Self {
        Self {
            range,
            role,
            mandatory_break: false,
            zero_width_soft_break: false,
            role_override: None,
        }
    }

    pub fn with_role_override(
        range: TextRange,
        role: FontRole,
        role_override: Option<RoleOverrideInfo>,
    ) -> Self {
        Self {
            range,
            role,
            mandatory_break: false,
            zero_width_soft_break: false,
            role_override,
        }
    }

    pub fn mandatory_break(range: TextRange) -> Self {
        Self {
            range,
            role: FontRole::Unknown,
            mandatory_break: true,
            zero_width_soft_break: false,
            role_override: None,
        }
    }

    pub fn zero_width_soft_break(range: TextRange) -> Self {
        Self {
            range,
            role: FontRole::Unknown,
            mandatory_break: false,
            zero_width_soft_break: true,
            role_override: None,
        }
    }
}

fn is_mandatory_break_code_point_at(code_point: i32, text: &str, index: i32) -> bool {
    is_mandatory_break_code_point(code_point)
        && !(code_point == 0x000A && index > 0 && utf16_code_unit_at(text, index - 1) == 0x000D)
}

fn code_point_at_compat(text: &str, index: i32) -> i32 {
    let high = utf16_code_unit_at(text, index);
    let text_length = utf16_length(text);
    if !(0xD800..=0xDBFF).contains(&high) || index + 1 >= text_length {
        return high;
    }
    let low = utf16_code_unit_at(text, index + 1);
    if !(0xDC00..=0xDFFF).contains(&low) {
        return high;
    }
    0x10000 + ((high - 0xD800) << 10) + (low - 0xDC00)
}

fn utf16_length(text: &str) -> i32 {
    text.encode_utf16().count() as i32
}

fn utf16_code_unit_at(text: &str, index: i32) -> i32 {
    text.encode_utf16()
        .nth(index as usize)
        .expect("cluster role offset must address a UTF-16 code unit") as i32
}

fn char_count(code_point: i32) -> i32 {
    if code_point > 0xFFFF { 2 } else { 1 }
}

fn is_variation_selector_code_point(code_point: i32) -> bool {
    (0xFE00..=0xFE0F).contains(&code_point) || (0xE0100..=0xE01EF).contains(&code_point)
}

/// `UnicodeEmojiSequenceRolePromotion`: promotes a text-default scalar to the Emoji fallback
/// policy only for a Unicode keycap, emoji-style variation, or modifier sequence.
fn emoji_role_promotion_reason(text: &str, start: i32, end: i32) -> Option<&'static str> {
    let base = code_point_at_compat(text, start);
    let mut next = start + char_count(base);

    if is_keycap_base_code_point(base) {
        if next < end && code_point_at_compat(text, next) == EMOJI_VARIATION_SELECTOR {
            next += char_count(EMOJI_VARIATION_SELECTOR);
        }
        if next < end && code_point_at_compat(text, next) == COMBINING_ENCLOSING_KEYCAP {
            return Some("KeycapSequence");
        }
    }

    if UnicodeEmojiData::contains(base)
        && UnicodeEmojiStyleVariationData::contains(base)
        && next < end
        && code_point_at_compat(text, next) == EMOJI_VARIATION_SELECTOR
    {
        return Some("EmojiStyleVariationSequence");
    }

    if UnicodeEmojiModifierBaseData::contains(base) {
        while next < end {
            let code_point = code_point_at_compat(text, next);
            if !is_combining_mark_code_point(code_point)
                && !is_variation_selector_code_point(code_point)
            {
                break;
            }
            next += char_count(code_point);
        }
        if next < end && (EMOJI_MODIFIER_START..=EMOJI_MODIFIER_END).contains(&code_point_at_compat(text, next)) {
            return Some("EmojiModifierSequence");
        }
    }

    None
}

fn is_keycap_base_code_point(code_point: i32) -> bool {
    code_point == 0x0023 || code_point == 0x002A || (0x0030..=0x0039).contains(&code_point)
}

fn source_slice(text: &str, range: TextRange) -> &str {
    let start = utf16_offset_to_utf8_byte_index(text, range.start())
        .expect("source range start must lie on scalar boundary");
    let end = utf16_offset_to_utf8_byte_index(text, range.end())
        .expect("source range end must lie on scalar boundary");
    &text[start..end]
}

fn is_combining_mark_code_point(code_point: i32) -> bool {
    code_point <= 0xFFFF
        && char::from_u32(code_point as u32).is_some_and(|character| {
            matches!(
                get_general_category(character),
                GeneralCategory::NonspacingMark
                    | GeneralCategory::SpacingMark
                    | GeneralCategory::EnclosingMark
            )
        })
}

fn is_ascii_point_mark_code_point(code_point: i32) -> bool {
    code_point <= 0xFFFF
        && char::from_u32(code_point as u32)
            .is_some_and(clreq_punctuation_policies::is_ascii_point_mark)
}

fn is_whitespace_code_unit(code_unit: i32) -> bool {
    char::from_u32(code_unit as u32).is_some_and(char::is_whitespace)
}

fn is_inside(range: TextRange, other: TextRange) -> bool {
    range.start() >= other.start() && range.end() <= other.end()
}

/// Kotlin data class `TextRange` 的稳定 `toString()` 格式，供固定的 `requireCoveredBy` 错误使用。
fn kotlin_text_range_string(range: TextRange) -> String {
    format!("TextRange(start={}, end={})", range.start(), range.end())
}

const EMOJI_VARIATION_SELECTOR: i32 = 0xFE0F;
const COMBINING_ENCLOSING_KEYCAP: i32 = 0x20E3;
const EMOJI_MODIFIER_START: i32 = 0x1F3FB;
const EMOJI_MODIFIER_END: i32 = 0x1F3FF;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::org::tiqian::core::Geometry::LayoutConstraints;
    use crate::org::tiqian::core::TextModel::{LayoutInput, TextSpan, TextStyle, TiqianTextContent};
    use crate::org::tiqian::clreq::ClreqProfile::ClreqProfile;
    use crate::org::tiqian::font::FontPolicy::CjkFontRoleClassifier;
    use crate::org::tiqian::layout::ParagraphLayoutEngine::{
        ExplainableStubParagraphLayoutEngine, ParagraphLayoutEngine,
    };

    #[test]
    fn complex_emoji_graphemes_are_single_emoji_shaping_ranges() {
        let text = "前👩🏽‍💻后🇨🇳与1️⃣。";
        let ranges = cluster_role_ranges(
            text,
            &CjkFontRoleClassifier,
            &FontRoleContext::default(),
            &ClreqProfile::mainland_horizontal(),
        );

        assert_eq!(
            ranges
                .iter()
                .map(|range| (range.range, range.role))
                .collect::<Vec<_>>(),
            vec![
                (TextRange::new(0, 1), FontRole::CjkText),
                (TextRange::new(1, 8), FontRole::Emoji),
                (TextRange::new(8, 9), FontRole::CjkText),
                (TextRange::new(9, 13), FontRole::Emoji),
                (TextRange::new(13, 14), FontRole::CjkText),
                (TextRange::new(14, 17), FontRole::Emoji),
                (TextRange::new(17, 18), FontRole::CjkPunctuation),
            ],
        );
    }

    #[test]
    fn unicode_emoji_properties_cover_text_default_and_composed_graphemes() {
        let text = "⌚🀄❤️❤☝🏻";
        let ranges = cluster_role_ranges(
            text,
            &CjkFontRoleClassifier,
            &FontRoleContext::default(),
            &ClreqProfile::mainland_horizontal(),
        );

        assert_eq!(
            ranges
                .iter()
                .map(|range| (range.range, range.role))
                .collect::<Vec<_>>(),
            vec![
                (TextRange::new(0, 1), FontRole::Emoji),
                (TextRange::new(1, 3), FontRole::Emoji),
                (TextRange::new(3, 5), FontRole::Emoji),
                (TextRange::new(5, 6), FontRole::Symbol),
                (TextRange::new(6, 9), FontRole::Emoji),
            ],
        );
    }

    #[test]
    fn complex_emoji_graphemes_reach_the_text_shaper_as_complete_ranges() {
        let text = "前👩🏽‍💻后🇨🇳与1️⃣。";
        let mut engine = ExplainableStubParagraphLayoutEngine::default();
        let result = engine.layout(
            LayoutInput::builder(
                TiqianTextContent::new(text.to_owned()),
                LayoutConstraints::with_defaults(1_000.0),
            )
            .build(),
        );

        assert_eq!(
            result
                .debug
                .shaping_decisions
                .iter()
                .filter(|decision| decision.font_key == "symbol-fallback")
                .map(|decision| (decision.range, decision.source_text.as_str()))
                .collect::<Vec<_>>(),
            vec![
                (TextRange::new(1, 8), "👩🏽‍💻"),
                (TextRange::new(9, 13), "🇨🇳"),
                (TextRange::new(14, 17), "1️⃣"),
            ],
        );
    }

    #[test]
    fn complex_emoji_graphemes_ignore_geometry_only_source_boundaries() {
        let options = ClusterRoleRangeOptions::builder()
            .span_boundaries([2].into_iter().collect())
            .emoji_shaping_boundaries(HashSet::new())
            .build();
        let ranges = cluster_role_ranges_with_options(
            "👩🏽‍💻",
            &CjkFontRoleClassifier,
            &FontRoleContext::default(),
            &ClreqProfile::mainland_horizontal(),
            &options,
        );

        assert_eq!(
            ranges
                .iter()
                .map(|range| (range.range, range.role))
                .collect::<Vec<_>>(),
            vec![(TextRange::new(0, 7), FontRole::Emoji)],
        );

        let mut engine = ExplainableStubParagraphLayoutEngine::default();
        let result = engine.layout(
            LayoutInput::builder(
                TiqianTextContent::builder("👩🏽‍💻".to_owned())
                    .source_boundaries([2].into_iter().collect())
                    .build(),
                LayoutConstraints::with_defaults(1_000.0),
            )
            .build(),
        );
        assert_eq!(
            result
                .debug
                .font_decisions
                .iter()
                .filter(|decision| decision.role == "Emoji")
                .map(|decision| decision.range)
                .collect::<Vec<_>>(),
            vec![TextRange::new(0, 7)],
        );
    }

    #[test]
    fn complex_emoji_graphemes_honor_layout_style_boundaries() {
        let hard_boundary: HashSet<i32> = [2].into_iter().collect();
        let options = ClusterRoleRangeOptions::builder()
            .span_boundaries(hard_boundary.clone())
            .emoji_shaping_boundaries(hard_boundary)
            .build();
        let ranges = cluster_role_ranges_with_options(
            "👩🏽‍💻",
            &CjkFontRoleClassifier,
            &FontRoleContext::default(),
            &ClreqProfile::mainland_horizontal(),
            &options,
        );

        assert_eq!(
            ranges
                .iter()
                .map(|range| (range.range, range.role))
                .collect::<Vec<_>>(),
            vec![
                (TextRange::new(0, 2), FontRole::Emoji),
                (TextRange::new(2, 7), FontRole::Emoji),
            ],
        );

        let mut engine = ExplainableStubParagraphLayoutEngine::default();
        let result = engine.layout(
            LayoutInput::builder(
                TiqianTextContent::builder("👩🏽‍💻".to_owned())
                    .spans(vec![TextSpan {
                        range: TextRange::new(2, 7),
                        style: TextStyle {
                            font_weight: 700,
                            ..TextStyle::default()
                        },
                    }])
                    .source_boundaries([2].into_iter().collect())
                    .build(),
                LayoutConstraints::with_defaults(1_000.0),
            )
            .build(),
        );
        assert_eq!(
            result
                .debug
                .font_decisions
                .iter()
                .filter(|decision| decision.role == "Emoji")
                .map(|decision| decision.range)
                .collect::<Vec<_>>(),
            vec![TextRange::new(0, 2), TextRange::new(2, 7)],
        );
    }

    #[test]
    fn unicode_emoji_sequence_roles_reject_unrelated_extenders() {
        let resolve = |text| {
            cluster_role_ranges(
                text,
                &CjkFontRoleClassifier,
                &FontRoleContext::default(),
                &ClreqProfile::mainland_horizontal(),
            )
            .into_iter()
            .map(|range| (range.range, range.role))
            .collect::<Vec<_>>()
        };

        assert_eq!(
            vec![(TextRange::new(0, 3), FontRole::Emoji)],
            resolve("1️⃣"),
        );
        assert_eq!(
            vec![(TextRange::new(0, 2), FontRole::Emoji)],
            resolve("❤️"),
        );
        assert_eq!(
            vec![(TextRange::new(0, 4), FontRole::Emoji)],
            resolve("👍🏽"),
        );
        assert_eq!(
            vec![(TextRange::new(0, 2), FontRole::LatinText)],
            resolve("a\u{FE0F}"),
        );
        assert_eq!(
            vec![(TextRange::new(0, 2), FontRole::LatinText)],
            resolve("a\u{20E3}"),
        );
        assert_eq!(
            vec![
                (TextRange::new(0, 1), FontRole::CjkText),
                (TextRange::new(1, 3), FontRole::Emoji),
            ],
            resolve("中🏽"),
        );
    }
}
