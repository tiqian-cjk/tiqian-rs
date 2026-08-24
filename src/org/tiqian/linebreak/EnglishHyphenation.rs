// 对应 Kotlin 源文件：engine/src/commonMain/kotlin/org/tiqian/linebreak/EnglishHyphenation.kt

use std::sync::OnceLock;

use super::Hyphenation::{Hyphenator, LiangHyphenator, parse_tex_hyphenation_patterns};

/**
 * 内置的美式英语 [`Hyphenator`]。它基于标准 TeX `hyph-en-us` pattern set（Gerard D.C. Kuiken，
 * hyph-utf8；宽松许可，其版权与许可头保留在随附资源中），`left_min`/`right_min` 遵循文件的
 * `hyphenmins`（2/3）。
 *
 * 这是 Tiqian 在需要确定性、可枚举 English hyphenation opportunity 的平台上共享的内置 pattern
 * hyphenator。
 */
pub mod english_hyphenation {
    use super::*;

    static EN_US: OnceLock<LiangHyphenator> = OnceLock::new();

    pub fn en_us() -> &'static dyn Hyphenator {
        EN_US.get_or_init(|| {
            let (patterns, exceptions) =
                parse_tex_hyphenation_patterns(load_bundled_english_hyphenation_patterns());
            LiangHyphenator::with_options(patterns, exceptions, 2, 3)
        })
    }
}

fn load_bundled_english_hyphenation_patterns() -> &'static str {
    include_str!("../../../../resources/hyphenation/hyph-en-us.tex")
}
