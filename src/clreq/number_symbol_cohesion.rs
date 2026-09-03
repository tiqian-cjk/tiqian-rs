// 对应 Kotlin 源文件：engine/src/commonMain/kotlin/org/tiqian/clreq/NumberSymbolCohesion.kt

use icu_properties::{CodePointMapData, props::GeneralCategory};

use super::super::core::geometry::TextRange;
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

    pub fn unbreakable_ranges(text: &Text) -> Vec<TextRange> {
        let mut result = Vec::new();
        let mut scalars = text.scalar_indices().peekable();
        let mut previous = None;
        while let Some((offset, character)) = scalars.next() {
            if !is_digit(character) {
                previous = Some((offset, character));
                continue;
            }

            let start = previous
                .filter(|(_, character)| {
                    PREFIX_SIGN.contains(character) || FRONT_CURRENCY.contains(character)
                })
                .map_or(offset, |(offset, _)| offset);

            // 最大数字串。仅当内部 `.` / `,` 位于两个数字之间时才吸收它们，亦即小数点或
            // 千分位，而不是句末句号或列表逗号。
            let mut end = offset + 1;
            loop {
                let Some((next_offset, next_character)) = scalars.peek().copied() else {
                    break;
                };
                if is_digit(next_character) {
                    let (_, character) = scalars
                        .next()
                        .expect("peek 的 scalar 必须可被消费");
                    end = next_offset + 1;
                    previous = Some((next_offset, character));
                    continue;
                }
                if next_character == '.' || next_character == ',' {
                    let separator = scalars
                        .next()
                        .expect("peek 的 scalar 必须可被消费");
                    if let Some((digit_offset, digit)) = scalars.peek().copied()
                        && is_digit(digit)
                    {
                        let (_, digit) = scalars
                            .next()
                            .expect("peek 的 scalar 必须可被消费");
                        end = digit_offset + 1;
                        previous = Some((digit_offset, digit));
                        continue;
                    }
                    previous = Some(separator);
                }
                break;
            }

            // 后缀：连续单位符号，之后还可带一个后置货币符号。
            while scalars
                .peek()
                .is_some_and(|(_, character)| SUFFIX_UNIT.contains(character))
            {
                let (offset, character) = scalars
                    .next()
                    .expect("peek 的 scalar 必须可被消费");
                end = offset + 1;
                previous = Some((offset, character));
            }
            if scalars
                .peek()
                .is_some_and(|(_, character)| BACK_CURRENCY.contains(character))
            {
                let (offset, character) = scalars
                    .next()
                    .expect("peek 的 scalar 必须可被消费");
                end = offset + 1;
                previous = Some((offset, character));
            }

            result.push(TextRange::new(start, end));
        }
        result
    }

    fn is_digit(character: char) -> bool {
        CodePointMapData::<GeneralCategory>::new().get(character)
            == GeneralCategory::DecimalNumber
    }

    const PREFIX_SIGN: [char; 3] = ['+', '-', '±'];
    const SUFFIX_UNIT: [char; 7] = [
        '%', '‰', '°', '℃', '℉', '′', '″',
    ];
    const FRONT_CURRENCY: [char; 10] = [
        '¥', '￥', '$', '＄', '€', '£', '₩', '₽', '₹', '฿',
    ];
    const BACK_CURRENCY: [char; 1] = ['₫'];

    #[cfg(test)]
    mod tests {
        use super::*;

        fn groups(text: &str) -> Vec<String> {
            let text = Text::from(text);
            unbreakable_ranges(&text)
                .into_iter()
                .map(|range| text.slice_text(range).as_str().to_owned())
                .collect()
        }

        #[test]
        fn scans_number_symbols_without_materializing_all_characters() {
            assert_eq!(vec!["3.14", "¥100", "100₫"], groups("3.14 ¥100 100₫"));
            assert_eq!(vec!["3", "+5"], groups("3.+5"));
            assert_eq!(vec!["¥100"], groups("😀¥100"));
        }
    }
}
