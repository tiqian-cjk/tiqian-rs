// 对应 Kotlin 源文件：engine/src/commonMain/kotlin/org/tiqian/clreq/NumberSymbolCohesion.kt

use icu_properties::{CodePointMapData, props::GeneralCategory};

use super::super::core::IntRange::IntRange;

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

    pub fn unbreakable_ranges(text: &str) -> Vec<IntRange> {
        // Kotlin `String` 与 `Char` 按 UTF-16 code unit 索引；source range 必须保留相同语义。
        let code_units: Vec<u16> = text.encode_utf16().collect();
        let mut result = Vec::new();
        let mut i = 0_usize;
        while i < code_units.len() {
            if !is_digit(code_units[i]) {
                i += 1;
                continue;
            }

            // 最大数字串。仅当内部 `.` / `,` 位于两个数字之间时才吸收它们，亦即小数点或
            // 千分位，而不是句末句号或列表逗号。
            let mut end = i;
            while end + 1 < code_units.len() {
                let character = code_units[end + 1];
                if is_digit(character) {
                    end += 1;
                } else if (character == b'.' as u16 || character == b',' as u16)
                    && end + 2 < code_units.len()
                    && is_digit(code_units[end + 2])
                {
                    end += 2;
                } else {
                    break;
                }
            }

            let mut start = i;
            // 前缀：紧邻数字之前的一个正负号或前置货币符号。
            if start > 0
                && (PREFIX_SIGN.contains(&code_units[start - 1])
                    || FRONT_CURRENCY.contains(&code_units[start - 1]))
            {
                start -= 1;
            }
            // 后缀：连续单位符号，之后还可带一个后置货币符号。
            while end + 1 < code_units.len() && SUFFIX_UNIT.contains(&code_units[end + 1]) {
                end += 1;
            }
            if end + 1 < code_units.len() && BACK_CURRENCY.contains(&code_units[end + 1]) {
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
