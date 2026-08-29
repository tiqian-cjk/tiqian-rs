use std::borrow::Borrow;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::sync::{Arc, OnceLock};

use super::Geometry::TextRange;

pub struct Text {
    inner: Arc<TextInner>,
}

struct TextInner {
    utf8: String,
    index: OnceLock<Utf16Index>,
}

struct Utf16Index {
    units: Vec<u16>,
    utf16_to_utf8: Vec<Option<usize>>,
    utf8_to_utf16: Vec<(usize, i32)>,
}

impl Text {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn as_str(&self) -> &str {
        &self.inner.utf8
    }

    pub fn utf16_len(&self) -> i32 {
        self.index().units.len() as i32
    }

    pub fn is_empty(&self) -> bool {
        self.inner.utf8.is_empty()
    }

    pub fn utf16_code_unit_at(&self, offset: i32) -> i32 {
        self.utf16_code_unit_at_or_none(offset)
            .expect("UTF-16 offset must address a code unit")
    }

    pub fn utf16_code_unit_at_or_none(&self, offset: i32) -> Option<i32> {
        (offset >= 0)
            .then(|| self.index().units.get(offset as usize).copied())
            .flatten()
            .map(i32::from)
    }

    pub fn code_point_at_compat(&self, offset: i32, end: i32) -> i32 {
        assert!(
            offset >= 0 && offset < end && end <= self.utf16_len(),
            "UTF-16 code point range must lie within the text"
        );
        let high = self.utf16_code_unit_at(offset);
        if !(HIGH_SURROGATE_START..=HIGH_SURROGATE_END).contains(&high) || offset + 1 >= end {
            return high;
        }
        let low = self.utf16_code_unit_at(offset + 1);
        if !(LOW_SURROGATE_START..=LOW_SURROGATE_END).contains(&low) {
            return high;
        }
        supplementary_code_point(high, low)
    }

    pub fn code_point_at_or_none(&self, offset: i32) -> Option<i32> {
        let high = self.utf16_code_unit_at_or_none(offset)?;
        if !(HIGH_SURROGATE_START..=HIGH_SURROGATE_END).contains(&high) {
            return Some(high);
        }
        let Some(low) = self.utf16_code_unit_at_or_none(offset + 1) else {
            return Some(high);
        };
        if !(LOW_SURROGATE_START..=LOW_SURROGATE_END).contains(&low) {
            return Some(high);
        }
        Some(supplementary_code_point(high, low))
    }

    pub fn code_point_before(&self, offset: i32) -> Option<i32> {
        assert!(
            offset <= self.utf16_len(),
            "UTF-16 offset must not exceed the text length"
        );
        if offset <= 0 {
            return None;
        }
        let low = self.utf16_code_unit_at(offset - 1);
        if !(LOW_SURROGATE_START..=LOW_SURROGATE_END).contains(&low) || offset < 2 {
            return Some(low);
        }
        let high = self.utf16_code_unit_at(offset - 2);
        if !(HIGH_SURROGATE_START..=HIGH_SURROGATE_END).contains(&high) {
            return Some(low);
        }
        Some(supplementary_code_point(high, low))
    }

    pub fn utf8_byte_index_at(&self, utf16_offset: i32) -> Option<usize> {
        if utf16_offset < 0 {
            return None;
        }
        self.index()
            .utf16_to_utf8
            .get(utf16_offset as usize)
            .copied()
            .flatten()
    }

    pub fn utf16_offset_at(&self, byte_offset: usize) -> Option<i32> {
        let boundaries = &self.index().utf8_to_utf16;
        boundaries
            .binary_search_by_key(&byte_offset, |(byte, _)| *byte)
            .ok()
            .map(|index| boundaries[index].1)
    }

    pub fn slice(&self, range: TextRange) -> &str {
        self.slice_offsets(range.start(), range.end())
    }

    pub fn slice_offsets(&self, start: i32, end: i32) -> &str {
        let start = self
            .utf8_byte_index_at(start)
            .expect("source slice start must lie on a Unicode scalar boundary");
        let end = self
            .utf8_byte_index_at(end)
            .expect("source slice end must lie on a Unicode scalar boundary");
        &self.inner.utf8[start..end]
    }

    fn index(&self) -> &Utf16Index {
        self.inner.index.get_or_init(|| {
            let units: Vec<u16> = self.inner.utf8.encode_utf16().collect();
            let mut utf16_to_utf8 = vec![None; units.len() + 1];
            let mut utf8_to_utf16 = Vec::new();
            let mut utf16_offset = 0_i32;

            for (byte_offset, character) in self.inner.utf8.char_indices() {
                utf16_to_utf8[utf16_offset as usize] = Some(byte_offset);
                utf8_to_utf16.push((byte_offset, utf16_offset));
                utf16_offset += character.len_utf16() as i32;
            }

            utf16_to_utf8[utf16_offset as usize] = Some(self.inner.utf8.len());
            utf8_to_utf16.push((self.inner.utf8.len(), utf16_offset));
            Utf16Index {
                units,
                utf16_to_utf8,
                utf8_to_utf16,
            }
        })
    }
}

impl Clone for Text {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
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
            inner: Arc::new(TextInner {
                utf8,
                index: OnceLock::new(),
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

impl Deref for Text {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

fn supplementary_code_point(high: i32, low: i32) -> i32 {
    0x10000 + ((high - HIGH_SURROGATE_START) << 10) + (low - LOW_SURROGATE_START)
}

const HIGH_SURROGATE_START: i32 = 0xD800;
const HIGH_SURROGATE_END: i32 = 0xDBFF;
const LOW_SURROGATE_START: i32 = 0xDC00;
const LOW_SURROGATE_END: i32 = 0xDFFF;
