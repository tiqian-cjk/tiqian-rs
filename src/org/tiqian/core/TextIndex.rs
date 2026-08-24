// Rust 原生辅助文件：不对应单个 Kotlin 源文件。
//
// Kotlin source range 使用 UTF-16 code-unit offset；Rust String 使用 UTF-8 byte offset。
// 本文件只在两种索引单位之间转换，不承载排版规则。

/// 将 Kotlin 语义的 UTF-16 code-unit offset 转换为 Rust UTF-8 byte offset。
///
/// 仅当 offset 位于 Unicode scalar 的边界时返回 `Some`；位于代理对中间或超出文本范围时返回 `None`。
pub fn utf16_offset_to_utf8_byte_index(text: &str, utf16_offset: i32) -> Option<usize> {
    if utf16_offset < 0 {
        return None;
    }

    let mut utf16_index = 0_i32;
    for (byte_index, ch) in text.char_indices() {
        if utf16_index == utf16_offset {
            return Some(byte_index);
        }
        utf16_index += ch.len_utf16() as i32;
    }

    (utf16_index == utf16_offset).then_some(text.len())
}

/// 将 Rust UTF-8 byte offset 转换为 Kotlin 语义的 UTF-16 code-unit offset。
///
/// 仅当 byte offset 位于 Unicode scalar 的边界时返回 `Some`。
pub fn utf8_byte_index_to_utf16_offset(text: &str, byte_index: usize) -> Option<i32> {
    if byte_index > text.len() || !text.is_char_boundary(byte_index) {
        return None;
    }

    Some(
        text[..byte_index]
            .chars()
            .map(|ch| ch.len_utf16() as i32)
            .sum(),
    )
}
