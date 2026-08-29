// 对应 Kotlin 源文件：engine/src/commonMain/kotlin/org/tiqian/layout/DefaultHyphenator.kt

use super::super::linebreak::english_hyphenation::english_hyphenation;
use super::super::linebreak::hyphenation::Hyphenator;

/// 没有显式传入 Western hyphenator 时使用的平台默认值。
///
/// Rust 迁移目标与 JVM/Android 一样内置 en-US TeX patterns；调用方可显式传入
/// `NoHyphenator` 关闭连字，以保持 deterministic test 或特定排版策略。
pub fn default_hyphenator() -> &'static dyn Hyphenator {
    english_hyphenation::en_us()
}
