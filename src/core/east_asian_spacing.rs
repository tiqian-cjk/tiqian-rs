// 对应 Kotlin 源文件：engine/src/commonMain/kotlin/org/tiqian/core/EastAsianSpacing.kt

use icu_properties::{CodePointMapData, props::GeneralCategory};

use super::east_asian_spacing_data::lookup;
use super::geometry::TextRange;
use super::source_interaction_boundaries::interaction_boundaries;
use super::text::Text;

/**
 * Unicode 草案属性 `East_Asian_Spacing` 的取值。
 *
 * 该属性有意与字体/书写系统分类分离：它回答两个字素簇之间是否应用东亚自动间距，
 * 而不决定任一字素簇使用哪个字体 shaping。
 */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EastAsianSpacingValue {
    Wide,
    Narrow,
    Other,
    Conditional,
}

/// 一个 shaping/layout cluster 所含全部 source 字素单元的边界属性。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EastAsianSpacingEdges {
    pub leading: EastAsianSpacingValue,
    pub trailing: EastAsianSpacingValue,
    pub contains_wide: bool,
}

/**
 * Unicode 提议草案 UTR #59 分类器，固定使用官方 2024-12-16 数据文件。
 *
 * UTR #59 是仍在制定中的资料性文档，并非稳定的 Unicode 属性。固定数据版本既能保持布局
 * 确定性，也能让未来的草案更新保持显式。
 */
pub mod unicode_east_asian_spacing {
    use super::*;

    pub const DATA_REVISION: &str = "draft-2024-12-16";
    pub const DATA_SOURCE: &str = "https://www.unicode.org/reports/tr59/east-asian-spacing.txt";
    pub const DATA_SHA256: &str =
        "49fe340a964a6e8e0ebc30099709c665cc6138d444b5c36dc336604047f1010f";
    pub const LANGUAGE_REGISTRY_REVISION: &str = "2026-06-14";
    pub const LANGUAGE_REGISTRY_SOURCE: &str =
        "https://www.iana.org/assignments/language-subtag-registry/language-subtag-registry";

    /// 解析固定 IANA 语言注册表中的 `zh` 宏语言成员。
    pub fn is_chinese_language_context(locale: &str) -> bool {
        super::is_chinese_language_context(locale)
    }

    /// 返回一个 Unicode 标量值尚未解析的属性值。
    pub fn property_of(code_point: i32) -> EastAsianSpacingValue {
        assert!(
            (0..=0x10FFFF).contains(&code_point),
            "Not a Unicode scalar value: {code_point}"
        );
        assert!(
            !(0xD800..=0xDFFF).contains(&code_point),
            "Surrogate is not a Unicode scalar value: {code_point}"
        );
        lookup(code_point)
    }

    /**
     * 为横排解析一个已经分段的字素簇。
     *
     * 根据 UTR #59，属性由第一个码点提供；包含 enclosing mark 的簇解析为
     * [EastAsianSpacingValue.Other]。Conditional 值仅在中文语言上下文中变为 Narrow。
     * 空输入或无效输入返回 Other，而不是虚构一个边界。
     */
    pub fn resolved_for_grapheme_cluster(
        grapheme_cluster: &Text,
        locale: &str,
    ) -> EastAsianSpacingValue {
        if grapheme_cluster.is_empty() {
            return EastAsianSpacingValue::Other;
        }
        if grapheme_cluster.chars().any(|character| {
            CodePointMapData::<GeneralCategory>::new().get(character)
                == GeneralCategory::EnclosingMark
        }) {
            return EastAsianSpacingValue::Other;
        }
        let property =
            property_of(grapheme_cluster.code_point_at_compat(0, grapheme_cluster.utf16_len()));
        match property {
            EastAsianSpacingValue::Conditional => {
                if super::is_chinese_language_context(locale) {
                    EastAsianSpacingValue::Narrow
                } else {
                    EastAsianSpacingValue::Other
                }
            }
            _ => property,
        }
    }

    /**
     * 解析较大 shaping cluster 首尾 source 字素的属性。Shaper 可能合并 `/Hi` 或整个单词；
     * 自动间距仍必须检查实际接触各边界的 source 单元，而不能把首个码点赋给整个 run。
     * 这里有意复用 Tiqian 的交互边界映射，使间距、选择与命中测试不会对不可分割的 source
     * 单元产生分歧；完整 UAX #29 覆盖仍属于该映射自身持续跟踪的契约，不在这里重新实现。
     */
    pub fn resolved_edges(text: &Text, locale: &str) -> EastAsianSpacingEdges {
        if text.is_empty() {
            return EastAsianSpacingEdges {
                leading: EastAsianSpacingValue::Other,
                trailing: EastAsianSpacingValue::Other,
                contains_wide: false,
            };
        }
        let text_length = text.utf16_len();
        let boundaries = interaction_boundaries(text, TextRange::new(0, text_length));
        let values: Vec<EastAsianSpacingValue> = boundaries
            .windows(2)
            .map(|boundary| {
                let cluster = text.slice_text(TextRange::new(boundary[0], boundary[1]));
                resolved_for_grapheme_cluster(&cluster, locale)
            })
            .collect();
        EastAsianSpacingEdges {
            leading: values[0],
            trailing: values[values.len() - 1],
            contains_wide: values.contains(&EastAsianSpacingValue::Wide),
        }
    }
}

/// 上述固定 IANA 语言子标签注册表版本中的 `Macrolanguage: zh` 记录。
const CHINESE_MACROLANGUAGE_MEMBERS: [&str; 19] = [
    "cdo", "cjy", "cmn", "cnp", "cpx", "csp", "czh", "czo", "gan", "hak", "hnm", "hsn", "luh",
    "lzh", "mnp", "nan", "sjc", "wuu", "yue",
];

fn is_chinese_language_context(locale: &str) -> bool {
    let language = locale
        .split(['-', '_'])
        .next()
        .expect("split always yields at least one segment")
        .to_lowercase();
    language == "zh" || CHINESE_MACROLANGUAGE_MEMBERS.contains(&language.as_str())
}
