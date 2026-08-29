// 对应 Kotlin 源文件：engine/src/commonMain/kotlin/org/tiqian/clreq/NumberSymbolCohesion.kt

use icu_properties::{CodePointMapData, props::GeneralCategory};

use super::super::core::int_range::IntRange;
use super::super::core::text::Text;

/**
 * `NumberSymbolCohesion`——CLREQ §符号分离禁则（数字及其相应的前后缀单位符号）：
 * 阿拉伯数字串及与其绑定的符号不得跨两行拆分。
 *
 * 1. 阿拉伯数字应作为一个整体，包括内部小数点/千分位 `.`、`,`。
 * 2. 后缀 `%`、`‰`、`°`、`℃`、`℉` 与其前面的数字之间不能拆行。
 * 3. 前缀正号 `+`、负号 `-`、正负号 `±` 与其后面的数字之间不能拆行。
 * 4. 货币符号与相关数字不能拆行，包括 `¥` 等前置符号和 `₫` 等后置符号。
 *
 * 返回断行器必须保持完整的 source text 区间，端点 `start..end` 均包含。已经 shape 为
 * 单个 cluster 的情形（如 `-3`、`100`、`100km`）会产生没有内部断点的区间，并无副作用；
 * 此规则实际保护的是会拆成多个 cluster 的情形（如 `50%`、`¥100`、`+5`、`37℃`、`±2`）。
 */
pub mod number_symbol_cohesion {
    use super::*;

    pub fn unbreakable_ranges(text: &Text) -> Vec<IntRange> {
        let text_length = text.utf16_len() as usize;
        let mut result = Vec::new();
        let mut i = 0_usize;
        while i < text_length {
            if !is_digit(text.utf16_code_unit_at(i as i32) as u16) {
                i += 1;
                continue;
            }

            // 最大数字串。仅当内部 `.` / `,` 位于两个数字之间时才吸收它们，亦即小数点或
            // 千分位，而不是句末句号或列表逗号。
            let mut end = i;
            while end + 1 < text_length {
                let character = text.utf16_code_unit_at((end + 1) as i32) as u16;
                if is_digit(character) {
                    end += 1;
                } else if (character == b'.' as u16 || character == b',' as u16)
                    && end + 2 < text_length
                    && is_digit(text.utf16_code_unit_at((end + 2) as i32) as u16)
                {
                    end += 2;
                } else {
                    break;
                }
            }

            let mut start = i;
            // 前缀：紧邻数字之前的一个正负号或前置货币符号。
            if start > 0
                && (PREFIX_SIGN.contains(&(text.utf16_code_unit_at((start - 1) as i32) as u16))
                    || FRONT_CURRENCY
                        .contains(&(text.utf16_code_unit_at((start - 1) as i32) as u16)))
            {
                start -= 1;
            }
            // 后缀：连续单位符号，之后还可带一个后置货币符号。
            while end + 1 < text_length
                && SUFFIX_UNIT.contains(&(text.utf16_code_unit_at((end + 1) as i32) as u16))
            {
                end += 1;
            }
            if end + 1 < text_length
                && BACK_CURRENCY.contains(&(text.utf16_code_unit_at((end + 1) as i32) as u16))
            {
                end += 1;
            }

            result.push(IntRange::new(start as i32, end as i32));
            i = end + 1;
        }
        result
    }

    fn is_digit(code_unit: u16) -> bool {
        CodePointMapData::<GeneralCategory>::new().get32(code_unit as u32)
            == GeneralCategory::DecimalNumber
    }

    const PREFIX_SIGN: [u16; 3] = ['+' as u16, '-' as u16, '±' as u16];
    const SUFFIX_UNIT: [u16; 7] = [
        '%' as u16,
        '‰' as u16,
        '°' as u16,
        '℃' as u16,
        '℉' as u16,
        '′' as u16,
        '″' as u16,
    ];
    const FRONT_CURRENCY: [u16; 10] = [
        '¥' as u16,
        '￥' as u16,
        '$' as u16,
        '＄' as u16,
        '€' as u16,
        '£' as u16,
        '₩' as u16,
        '₽' as u16,
        '₹' as u16,
        '฿' as u16,
    ];
    const BACK_CURRENCY: [u16; 1] = ['₫' as u16];
}
