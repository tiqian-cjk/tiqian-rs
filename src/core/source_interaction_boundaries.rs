// 对应 Kotlin 源文件：engine/src/commonMain/kotlin/org/tiqian/core/SourceInteractionBoundaries.kt

use icu_properties::{
    CodePointMapData, CodePointSetData,
    props::{
        EmojiModifier, EmojiModifierBase, ExtendedPictographic, GeneralCategory,
        HangulSyllableType, RegionalIndicator, VariationSelector,
    },
};

use super::geometry::{ScalarOffset, TextRange};
use super::text::Text;

/// 交互偏移落在一个 source 字素内部时使用的方向。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceBoundaryBias {
    Backward,
    Forward,
    Nearest,
}

/// `SourceInteractionBoundaryMap`：将光标/选区偏移保持在稳定的 scalar source 边界上。
/// 简单拉丁码点仍可单独选择，而组合/变体序列、emoji modifier、区域指示符对、
/// Hangul 音节序列和 ZWJ 连接序列保持不可分割。
///
/// 这有意作为交互投影；公开 source ABI 使用 scalar，layout atom/shaping cluster
/// 保持其既有 range。
pub fn coerce_to_interaction_boundary(
    text: &Text,
    offset: ScalarOffset,
    range: TextRange,
    bias: SourceBoundaryBias,
) -> ScalarOffset {
    let text_length = text.scalar_len();
    let start = range.start().min(text_length);
    let end = range.end().min(text_length).max(start);
    let target = offset.min(end).max(start);
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

pub fn interaction_boundaries(text: &Text, range: TextRange) -> Vec<ScalarOffset> {
    let text_length = text.scalar_len();
    let start = range.start().min(text_length);
    let end = range.end().min(text_length).max(start);
    interaction_boundaries_in_range(text, start, end)
}

/// 为绝不能拆分组合序列、emoji modifier/ZWJ 序列、区域指示符对或 Hangul
/// 音节序列的布局策略提供安全的 source 字素边界。
pub fn source_grapheme_boundaries(text: &Text, range: TextRange) -> Vec<ScalarOffset> {
    interaction_boundaries(text, range)
}

fn interaction_boundaries_in_range(
    text: &Text,
    start: ScalarOffset,
    end: ScalarOffset,
) -> Vec<ScalarOffset> {
    let mut out = vec![start];
    let mut index = start;
    while index < end {
        let first = text
            .code_point_at_or_none(index)
            .expect("range 内的 scalar offset 必须有效");
        let mut next = index + 1;
        let mut preceding_emoji_modifier_base = is_emoji_modifier_base(first);
        let mut preceding_extended_pictographic = is_extended_pictographic(first);

        if first == CR && next < end && text.code_point_at_or_none(next) == Some(LF) {
            next += 1;
        } else if is_regional_indicator(first) && next < end {
            let following = text
                .code_point_at_or_none(next)
                .expect("range 内的 scalar offset 必须有效");
            if is_regional_indicator(following) {
                next += 1;
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
            let code_point = text
                .code_point_at_or_none(next)
                .expect("range 内的 scalar offset 必须有效");
            if is_emoji_modifier(code_point) {
                next += 1;
                preceding_emoji_modifier_base = false;
                next = consume_while(text, next, end, is_interaction_extender);
            }
        }
        while next < end && text.code_point_at_or_none(next) == Some(ZWJ) {
            next += 1;
            if next >= end {
                break;
            }
            let joined = text
                .code_point_at_or_none(next)
                .expect("range 内的 scalar offset 必须有效");
            if !preceding_extended_pictographic || !is_extended_pictographic(joined) {
                break;
            }
            next += 1;
            preceding_emoji_modifier_base = is_emoji_modifier_base(joined);
            preceding_extended_pictographic = true;
            next = consume_while(text, next, end, is_interaction_extender);
            if preceding_emoji_modifier_base && next < end {
                let modifier = text
                    .code_point_at_or_none(next)
                    .expect("range 内的 scalar offset 必须有效");
                if is_emoji_modifier(modifier) {
                    next += 1;
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

fn consume_while(
    text: &Text,
    from: ScalarOffset,
    end: ScalarOffset,
    accept: fn(i32) -> bool,
) -> ScalarOffset {
    for (local_offset, character) in text.slice_text(TextRange::new(from, end)).scalar_indices() {
        if !accept(character as i32) {
            return from + local_offset.value();
        }
    }
    end
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
    use crate::core::geometry::scalar_offset;

    fn boundaries(text: &str) -> Vec<ScalarOffset> {
        let text = Text::from(text);
        source_grapheme_boundaries(&text, TextRange::new(ScalarOffset::ZERO, text.scalar_len()))
    }

    #[test]
    fn emoji_graphemes_require_legal_modifier_and_zwj_context() {
        assert_eq!(vec![ScalarOffset::ZERO, scalar_offset(4)], boundaries("👩🏽‍💻"));
        assert_eq!(
            vec![ScalarOffset::ZERO, scalar_offset(1), scalar_offset(2)],
            boundaries("中🏽")
        );
        assert_eq!(
            vec![ScalarOffset::ZERO, scalar_offset(2), scalar_offset(3)],
            boundaries("👩‍中")
        );
        assert_eq!(
            vec![ScalarOffset::ZERO, scalar_offset(2), scalar_offset(3)],
            boundaries("中‍👩")
        );
    }
}
