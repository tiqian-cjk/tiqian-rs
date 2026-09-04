use std::borrow::Borrow;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, OnceLock};

use super::geometry::{ScalarOffset, TextRange};

/// 共享 UTF-8 存储上的文本视图。
///
/// 切片只克隆 `Arc` 并缩窄范围，cluster 大小的子串不分配内存，并与父文本复用 scalar 索引。
pub struct Text {
    inner: Arc<TextInner>,
    byte_start: u32,
    byte_end: u32,
    scalar_start: ScalarOffset,
    /// `None` 表示延伸到共享文本末尾，长度在首次 source coordinate 访问时解析。
    scalar_end: Option<ScalarOffset>,
}

struct TextInner {
    utf8: String,
    scalar_index: OnceLock<ScalarIndex>,
}

/// 每一项是全文对应 scalar boundary 的 UTF-8 byte offset。
///
/// 首项为 `0`，末项为全文 byte length，因此长度始终为 `scalar_len + 1`。
struct ScalarIndex {
    byte_boundaries: Vec<u32>,
}

impl Text {
    pub fn new() -> Self {
        Self::default()
    }

    #[inline]
    pub fn as_str(&self) -> &str {
        &self.inner.utf8[self.byte_start as usize..self.byte_end as usize]
    }

    /// 返回当前文本视图的 UTF-8 byte 长度，不可作为 source coordinate 使用。
    #[inline]
    pub fn byte_len(&self) -> usize {
        self.as_str().len()
    }

    /// 按当前文本视图的 Unicode scalar 顺序遍历字符。
    #[inline]
    pub fn chars(&self) -> std::str::Chars<'_> {
        self.as_str().chars()
    }

    /// 按当前文本视图的本地 scalar offset 遍历字符。
    #[inline]
    pub fn scalar_indices(&self) -> impl Iterator<Item = (ScalarOffset, char)> + '_ {
        self.chars()
            .enumerate()
            .map(|(index, character)| (ScalarOffset::new(index as i32), character))
    }

    #[inline]
    pub fn scalar_len(&self) -> ScalarOffset {
        ScalarOffset::new(self.resolved_scalar_end() - self.scalar_start)
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.byte_start == self.byte_end
    }

    #[inline]
    pub fn code_point_at_or_none(&self, offset: ScalarOffset) -> Option<i32> {
        let byte = self.absolute_byte_index_at(offset)?;
        (byte < self.byte_end as usize)
            .then(|| self.inner.utf8[byte..].chars().next().map(|character| character as i32))
            .flatten()
    }

    pub fn code_point_before(&self, offset: ScalarOffset) -> Option<i32> {
        let byte = self.absolute_byte_index_at(offset)?;
        (byte > self.byte_start as usize)
            .then(|| self.inner.utf8[..byte].chars().next_back().map(|character| character as i32))
            .flatten()
    }

    /// 相对于 [`Self::as_str`] 的 UTF-8 byte offset。
    #[inline]
    pub fn utf8_byte_index_at(&self, offset: ScalarOffset) -> Option<usize> {
        self.absolute_byte_index_at(offset)
            .map(|byte| byte - self.byte_start as usize)
    }

    /// 接收相对于 [`Self::as_str`] 的 UTF-8 byte offset，并仅接受 scalar boundary。
    pub fn scalar_offset_at(&self, byte_offset: usize) -> Option<ScalarOffset> {
        let absolute = self.byte_start as usize + byte_offset;
        if absolute > self.byte_end as usize {
            return None;
        }
        let absolute_scalar = self
            .index()
            .byte_boundaries
            .binary_search(&(absolute as u32))
            .ok()?;
        Some(ScalarOffset::new(
            absolute_scalar as i32 - self.scalar_start.value(),
        ))
    }

    pub fn slice(&self, range: TextRange) -> &str {
        self.slice_offsets(range.start(), range.end())
    }

    pub fn slice_offsets(&self, start: ScalarOffset, end: ScalarOffset) -> &str {
        let (start, end) = self.absolute_byte_bounds(start, end);
        &self.inner.utf8[start..end]
    }

    /// Narrows to a shared view over the same storage; no allocation, no reindexing.
    pub fn slice_text(&self, range: TextRange) -> Self {
        let (byte_start, byte_end) = self.absolute_byte_bounds(range.start(), range.end());
        Self {
            inner: Arc::clone(&self.inner),
            byte_start: byte_start as u32,
            byte_end: byte_end as u32,
            scalar_start: ScalarOffset::new(self.scalar_start.value() + range.start().value()),
            scalar_end: Some(ScalarOffset::new(self.scalar_start.value() + range.end().value())),
        }
    }

    #[inline]
    fn absolute_byte_bounds(&self, start: ScalarOffset, end: ScalarOffset) -> (usize, usize) {
        (
            self.absolute_byte_index_at(start)
                .expect("source slice start must lie on a Unicode scalar boundary"),
            self.absolute_byte_index_at(end)
                .expect("source slice end must lie on a Unicode scalar boundary"),
        )
    }

    #[inline]
    fn absolute_byte_index_at(&self, offset: ScalarOffset) -> Option<usize> {
        if offset > self.scalar_len() {
            return None;
        }
        let absolute = self.scalar_start.value() + offset.value();
        self.index()
            .byte_boundaries
            .get(absolute as usize)
            .copied()
            .map(|byte| byte as usize)
    }

    #[inline]
    fn resolved_scalar_end(&self) -> ScalarOffset {
        self.scalar_end
            .unwrap_or_else(|| ScalarOffset::new(self.index().byte_boundaries.len() as i32 - 1))
    }

    fn index(&self) -> &ScalarIndex {
        self.inner.scalar_index.get_or_init(|| {
            let utf8 = self.inner.utf8.as_str();
            let mut byte_boundaries = Vec::with_capacity(utf8.chars().count() + 1);
            byte_boundaries.extend(utf8.char_indices().map(|(byte, _)| byte as u32));
            byte_boundaries.push(utf8.len() as u32);
            ScalarIndex { byte_boundaries }
        })
    }
}

impl Clone for Text {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            byte_start: self.byte_start,
            byte_end: self.byte_end,
            scalar_start: self.scalar_start,
            scalar_end: self.scalar_end,
        }
    }
}

impl Default for Text {
    fn default() -> Self {
        Self::from(String::new())
    }
}

impl From<String> for Text {
    fn from(utf8: String) -> Self {
        Self {
            byte_start: 0,
            byte_end: utf8.len() as u32,
            scalar_start: ScalarOffset::ZERO,
            scalar_end: None,
            inner: Arc::new(TextInner {
                utf8,
                scalar_index: OnceLock::new(),
            }),
        }
    }
}

impl From<&str> for Text {
    fn from(utf8: &str) -> Self {
        Self::from(utf8.to_owned())
    }
}

impl From<Text> for String {
    fn from(text: Text) -> Self {
        let full_view = text.byte_start == 0 && text.byte_end as usize == text.inner.utf8.len();
        if !full_view {
            return text.as_str().to_owned();
        }
        Arc::try_unwrap(text.inner)
            .map(|inner| inner.utf8)
            .unwrap_or_else(|inner| inner.utf8.clone())
    }
}

impl From<&Text> for String {
    fn from(text: &Text) -> Self {
        text.as_str().to_owned()
    }
}

impl fmt::Debug for Text {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_str().fmt(formatter)
    }
}

impl fmt::Display for Text {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_str().fmt(formatter)
    }
}

impl PartialEq for Text {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl Eq for Text {}

impl PartialEq<str> for Text {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<&str> for Text {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl PartialEq<String> for Text {
    fn eq(&self, other: &String) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<Text> for str {
    fn eq(&self, other: &Text) -> bool {
        self == other.as_str()
    }
}

impl PartialEq<Text> for &str {
    fn eq(&self, other: &Text) -> bool {
        *self == other.as_str()
    }
}

impl PartialEq<Text> for String {
    fn eq(&self, other: &Text) -> bool {
        self.as_str() == other.as_str()
    }
}

impl Hash for Text {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_str().hash(state);
    }
}

impl AsRef<str> for Text {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Borrow<str> for Text {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

#[cfg(test)]
mod tests {
    use super::Text;
    use crate::core::geometry::{scalar_offset, text_range};

    #[test]
    fn scalar_and_utf8_boundaries_round_trip() {
        let text = Text::from("A中😀e\u{301}");
        assert_eq!(text.scalar_len(), scalar_offset(5));
        assert_eq!(text.utf8_byte_index_at(scalar_offset(0)), Some(0));
        assert_eq!(text.utf8_byte_index_at(scalar_offset(1)), Some(1));
        assert_eq!(text.utf8_byte_index_at(scalar_offset(2)), Some(4));
        assert_eq!(text.utf8_byte_index_at(scalar_offset(3)), Some(8));
        assert_eq!(text.utf8_byte_index_at(scalar_offset(5)), Some(11));
        assert_eq!(text.scalar_offset_at(8), Some(scalar_offset(3)));
        assert_eq!(text.scalar_offset_at(9), Some(scalar_offset(4)));
        assert_eq!(text.code_point_at_or_none(scalar_offset(2)), Some(0x1F600));
        assert_eq!(text.code_point_before(scalar_offset(3)), Some(0x1F600));
    }

    #[test]
    fn scalar_slices_share_the_index_and_keep_local_offsets() {
        let text = Text::from("A中😀e\u{301}");
        let slice = text.slice_text(text_range(1, 4));
        assert_eq!(slice, "中😀e");
        assert_eq!(slice.scalar_len(), scalar_offset(3));
        assert_eq!(slice.utf8_byte_index_at(scalar_offset(2)), Some(7));
        assert_eq!(slice.scalar_offset_at(7), Some(scalar_offset(2)));
        assert_eq!(slice.slice_offsets(scalar_offset(1), scalar_offset(2)), "😀");
    }

    #[test]
    fn scalar_iteration_and_byte_length_are_local_to_the_view() {
        let text = Text::from("A中😀");
        let slice = text.slice_text(text_range(1, 3));

        assert_eq!(slice.byte_len(), "中😀".len());
        assert_eq!(slice.chars().collect::<Vec<_>>(), vec!['中', '😀']);
        assert_eq!(
            slice.scalar_indices().collect::<Vec<_>>(),
            vec![(scalar_offset(0), '中'), (scalar_offset(1), '😀')]
        );
    }
}
