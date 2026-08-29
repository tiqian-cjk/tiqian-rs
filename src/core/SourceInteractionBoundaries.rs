// 对应 Kotlin 源文件：engine/src/commonMain/kotlin/org/tiqian/core/SourceInteractionBoundaries.kt

use icu_properties::{
    CodePointMapData, CodePointSetData,
    props::{
        EmojiModifier, EmojiModifierBase, ExtendedPictographic, GeneralCategory,
        HangulSyllableType, RegionalIndicator, VariationSelector,
    },
};

use super::Geometry::TextRange;
use super::Text::Text;

/// 交互偏移落在一个 source 字素内部时使用的方向。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceBoundaryBias {
    Backward,
    Forward,
    Nearest,
}

/// `SourceInteractionBoundaryMap`：将光标/选区偏移保持在稳定的 UTF-16 source 边界上。
/// 简单拉丁码点仍可单独选择，而代理对、组合/变体序列、emoji modifier、区域指示符对、
/// Hangul 音节序列和 ZWJ 连接序列保持不可分割。
///
/// 这有意作为交互投影；公开 source ABI 仍使用 UTF-16，layout atom/shaping cluster
/// 保持其既有 range。
pub fn coerce_to_interaction_boundary(
    text: &Text,
    offset: i32,
    range: TextRange,
    bias: SourceBoundaryBias,
) -> i32 {
    let text_length = text.utf16_len();
    let start = range.start().clamp(0, text_length);
    let end = range.end().clamp(start, text_length);
    let target = offset.clamp(start, end);
    if target == start || target == end {
        return target;
    }
    let boundaries = interaction_boundaries_in_range(text, start, end);
    if boundaries.binary_search(&target).is_ok() {
        return target;
    }
    let previous = *boundaries
        .iter()
        .rev()
        .find(|&&boundary| boundary < target)
        .expect("interaction boundaries must contain a preceding boundary");
    let next = *boundaries
        .iter()
        .find(|&&boundary| boundary > target)
        .expect("interaction boundaries must contain a following boundary");
    match bias {
        SourceBoundaryBias::Backward => previous,
        SourceBoundaryBias::Forward => next,
        SourceBoundaryBias::Nearest => {
            if target - previous < next - target {
                previous
            } else {
                next
            }
        }
    }
}

pub fn interaction_boundaries(text: &Text, range: TextRange) -> Vec<i32> {
    let text_length = text.utf16_len();
    let start = range.start().clamp(0, text_length);
    let end = range.end().clamp(start, text_length);
    interaction_boundaries_in_range(text, start, end)
}

/// 为绝不能拆分代理对、组合序列、emoji modifier/ZWJ 序列、区域指示符对或 Hangul
/// 音节序列的布局策略提供安全的 source 字素边界。
pub fn source_grapheme_boundaries(text: &Text, range: TextRange) -> Vec<i32> {
    interaction_boundaries(text, range)
}

fn interaction_boundaries_in_range(text: &Text, start: i32, end: i32) -> Vec<i32> {
    let mut out = vec![start];
    let mut index = start;
    while index < end {
        let first = text.code_point_at_compat(index, end);
        let mut next = index + char_count_compat(first);
        let mut preceding_emoji_modifier_base = is_emoji_modifier_base(first);
        let mut preceding_extended_pictographic = is_extended_pictographic(first);

        if first == CR && next < end && text.code_point_at_compat(next, end) == LF {
            next += 1;
        } else if is_regional_indicator(first) && next < end {
            let following = text.code_point_at_compat(next, end);
            if is_regional_indicator(following) {
                next += char_count_compat(following);
            }
        } else if is_hangul_l(first) {
            next = consume_while(text, next, end, is_hangul_l);
            let after_v = consume_while(text, next, end, is_hangul_v);
            if after_v > next {
                next = consume_while(text, after_v, end, is_hangul_t);
            }
        } else if is_hangul_lv_or_lvt(first) {
            if is_hangul_lv(first) {
                next = consume_while(text, next, end, is_hangul_v);
            }
            next = consume_while(text, next, end, is_hangul_t);
        }

        next = consume_while(text, next, end, is_interaction_extender);
        if preceding_emoji_modifier_base && next < end {
            let code_point = text.code_point_at_compat(next, end);
            if is_emoji_modifier(code_point) {
                next += char_count_compat(code_point);
                preceding_emoji_modifier_base = false;
                next = consume_while(text, next, end, is_interaction_extender);
            }
        }
        while next < end && text.code_point_at_compat(next, end) == ZWJ {
            next += 1;
            if next >= end {
                break;
            }
            let joined = text.code_point_at_compat(next, end);
            if !preceding_extended_pictographic || !is_extended_pictographic(joined) {
                break;
            }
            next += char_count_compat(joined);
            preceding_emoji_modifier_base = is_emoji_modifier_base(joined);
            preceding_extended_pictographic = true;
            next = consume_while(text, next, end, is_interaction_extender);
            if preceding_emoji_modifier_base && next < end {
                let modifier = text.code_point_at_compat(next, end);
                if is_emoji_modifier(modifier) {
                    next += char_count_compat(modifier);
                    preceding_emoji_modifier_base = false;
                    next = consume_while(text, next, end, is_interaction_extender);
                }
            }
        }
        index = next;
        out.push(index);
    }
    out
}

fn consume_while(text: &Text, from: i32, end: i32, accept: fn(i32) -> bool) -> i32 {
    let mut index = from;
    while index < end {
        let code_point = text.code_point_at_compat(index, end);
        if !accept(code_point) {
            break;
        }
        index += char_count_compat(code_point);
    }
    index
}

fn char_count_compat(code_point: i32) -> i32 {
    if code_point > 0xFFFF { 2 } else { 1 }
}

fn is_interaction_extender(code_point: i32) -> bool {
    code_point == ZWNJ
        || CodePointSetData::new::<VariationSelector>().contains32(code_point as u32)
        || (EMOJI_TAG_START..=EMOJI_TAG_END).contains(&code_point)
        || matches!(
            CodePointMapData::<GeneralCategory>::new().get32(code_point as u32),
            GeneralCategory::NonspacingMark
                | GeneralCategory::SpacingMark
                | GeneralCategory::EnclosingMark
        )
}

fn is_emoji_modifier(code_point: i32) -> bool {
    CodePointSetData::new::<EmojiModifier>().contains32(code_point as u32)
}

fn is_emoji_modifier_base(code_point: i32) -> bool {
    CodePointSetData::new::<EmojiModifierBase>().contains32(code_point as u32)
}

fn is_extended_pictographic(code_point: i32) -> bool {
    CodePointSetData::new::<ExtendedPictographic>().contains32(code_point as u32)
}

fn is_regional_indicator(code_point: i32) -> bool {
    CodePointSetData::new::<RegionalIndicator>().contains32(code_point as u32)
}

fn is_hangul_l(code_point: i32) -> bool {
    CodePointMapData::<HangulSyllableType>::new().get32(code_point as u32)
        == HangulSyllableType::LeadingJamo
}

fn is_hangul_v(code_point: i32) -> bool {
    CodePointMapData::<HangulSyllableType>::new().get32(code_point as u32)
        == HangulSyllableType::VowelJamo
}

fn is_hangul_t(code_point: i32) -> bool {
    CodePointMapData::<HangulSyllableType>::new().get32(code_point as u32)
        == HangulSyllableType::TrailingJamo
}

fn is_hangul_lv_or_lvt(code_point: i32) -> bool {
    matches!(
        CodePointMapData::<HangulSyllableType>::new().get32(code_point as u32),
        HangulSyllableType::LVSyllable | HangulSyllableType::LVTSyllable
    )
}

fn is_hangul_lv(code_point: i32) -> bool {
    CodePointMapData::<HangulSyllableType>::new().get32(code_point as u32)
        == HangulSyllableType::LVSyllable
}

const EMOJI_TAG_START: i32 = 0xE0020;
const EMOJI_TAG_END: i32 = 0xE007F;
const CR: i32 = 0x000D;
const LF: i32 = 0x000A;
const ZWNJ: i32 = 0x200C;
const ZWJ: i32 = 0x200D;

#[cfg(test)]
mod tests {
    use super::*;

    fn boundaries(text: &str) -> Vec<i32> {
        let text = Text::from(text);
        source_grapheme_boundaries(&text, TextRange::new(0, text.utf16_len()))
    }

    #[test]
    fn emoji_graphemes_require_legal_modifier_and_zwj_context() {
        assert_eq!(vec![0, 7], boundaries("👩🏽‍💻"));
        assert_eq!(vec![0, 1, 3], boundaries("中🏽"));
        assert_eq!(vec![0, 3, 4], boundaries("👩‍中"));
        assert_eq!(vec![0, 2, 4], boundaries("中‍👩"));
    }
}
