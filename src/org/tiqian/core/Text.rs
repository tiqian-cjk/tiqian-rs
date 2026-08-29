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

/// All-ASCII text needs no side tables: UTF-16 offsets equal byte offsets and every
/// code unit is a byte. Other text pays for explicit tables.
enum Utf16Index {
    Ascii,
    Wide(WideIndex),
}

/// `utf16_to_utf8` maps every UTF-16 offset to its UTF-8 byte offset, using
/// [`SURROGATE_HALF`] for offsets that land inside a surrogate pair.
/// `utf8_to_utf16` holds one entry per scalar, ascending by byte offset.
struct WideIndex {
    units: Vec<u16>,
    utf16_to_utf8: Vec<u32>,
    utf8_to_utf16: Vec<(u32, u32)>,
}

const SURROGATE_HALF: u32 = u32::MAX;

impl WideIndex {
    #[inline]
    fn unit_at(&self, offset: usize) -> Option<i32> {
        self.units.get(offset).copied().map(i32::from)
    }

    #[inline]
    fn byte_index_at(&self, utf16_offset: i32) -> Option<usize> {
        if utf16_offset < 0 {
            return None;
        }
        self.utf16_to_utf8
            .get(utf16_offset as usize)
            .copied()
            .filter(|byte| *byte != SURROGATE_HALF)
            .map(|byte| byte as usize)
    }
}

impl Text {
    pub fn new() -> Self {
        Self::default()
    }

    #[inline]
    pub fn as_str(&self) -> &str {
        &self.inner.utf8
    }

    #[inline]
    pub fn utf16_len(&self) -> i32 {
        match self.index() {
            Utf16Index::Ascii => self.inner.utf8.len() as i32,
            Utf16Index::Wide(wide) => wide.units.len() as i32,
        }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.inner.utf8.is_empty()
    }

    #[inline]
    pub fn utf16_code_unit_at(&self, offset: i32) -> i32 {
        self.utf16_code_unit_at_or_none(offset)
            .expect("UTF-16 offset must address a code unit")
    }

    #[inline]
    pub fn utf16_code_unit_at_or_none(&self, offset: i32) -> Option<i32> {
        if offset < 0 {
            return None;
        }
        match self.index() {
            Utf16Index::Ascii => self
                .inner
                .utf8
                .as_bytes()
                .get(offset as usize)
                .map(|byte| i32::from(*byte)),
            Utf16Index::Wide(wide) => wide.unit_at(offset as usize),
        }
    }

    pub fn code_point_at_compat(&self, offset: i32, end: i32) -> i32 {
        let Utf16Index::Wide(wide) = self.index() else {
            debug_assert!(
                offset >= 0 && offset < end && end <= self.inner.utf8.len() as i32,
                "UTF-16 code point range must lie within the text"
            );
            return i32::from(self.inner.utf8.as_bytes()[offset as usize]);
        };
        debug_assert!(
            offset >= 0 && offset < end && end <= wide.units.len() as i32,
            "UTF-16 code point range must lie within the text"
        );
        let high = i32::from(wide.units[offset as usize]);
        if !(HIGH_SURROGATE_START..=HIGH_SURROGATE_END).contains(&high) || offset + 1 >= end {
            return high;
        }
        let low = i32::from(wide.units[(offset + 1) as usize]);
        if !(LOW_SURROGATE_START..=LOW_SURROGATE_END).contains(&low) {
            return high;
        }
        supplementary_code_point(high, low)
    }

    pub fn code_point_at_or_none(&self, offset: i32) -> Option<i32> {
        if offset < 0 {
            return None;
        }
        let Utf16Index::Wide(wide) = self.index() else {
            return self.utf16_code_unit_at_or_none(offset);
        };
        let high = wide.unit_at(offset as usize)?;
        if !(HIGH_SURROGATE_START..=HIGH_SURROGATE_END).contains(&high) {
            return Some(high);
        }
        let Some(low) = wide.unit_at(offset as usize + 1) else {
            return Some(high);
        };
        if !(LOW_SURROGATE_START..=LOW_SURROGATE_END).contains(&low) {
            return Some(high);
        }
        Some(supplementary_code_point(high, low))
    }

    pub fn code_point_before(&self, offset: i32) -> Option<i32> {
        let Utf16Index::Wide(wide) = self.index() else {
            debug_assert!(
                offset <= self.inner.utf8.len() as i32,
                "UTF-16 offset must not exceed the text length"
            );
            return (offset > 0).then(|| i32::from(self.inner.utf8.as_bytes()[offset as usize - 1]));
        };
        debug_assert!(
            offset <= wide.units.len() as i32,
            "UTF-16 offset must not exceed the text length"
        );
        if offset <= 0 {
            return None;
        }
        let low = i32::from(wide.units[(offset - 1) as usize]);
        if !(LOW_SURROGATE_START..=LOW_SURROGATE_END).contains(&low) || offset < 2 {
            return Some(low);
        }
        let high = i32::from(wide.units[(offset - 2) as usize]);
        if !(HIGH_SURROGATE_START..=HIGH_SURROGATE_END).contains(&high) {
            return Some(low);
        }
        Some(supplementary_code_point(high, low))
    }

    #[inline]
    pub fn utf8_byte_index_at(&self, utf16_offset: i32) -> Option<usize> {
        if utf16_offset < 0 {
            return None;
        }
        match self.index() {
            Utf16Index::Ascii => {
                (utf16_offset as usize <= self.inner.utf8.len()).then_some(utf16_offset as usize)
            }
            Utf16Index::Wide(wide) => wide.byte_index_at(utf16_offset),
        }
    }

    pub fn utf16_offset_at(&self, byte_offset: usize) -> Option<i32> {
        match self.index() {
            Utf16Index::Ascii => {
                (byte_offset <= self.inner.utf8.len()).then_some(byte_offset as i32)
            }
            Utf16Index::Wide(wide) => wide
                .utf8_to_utf16
                .binary_search_by_key(&(byte_offset as u32), |(byte, _)| *byte)
                .ok()
                .map(|index| wide.utf8_to_utf16[index].1 as i32),
        }
    }

    pub fn slice(&self, range: TextRange) -> &str {
        self.slice_offsets(range.start(), range.end())
    }

    pub fn slice_offsets(&self, start: i32, end: i32) -> &str {
        let (start, end) = match self.index() {
            Utf16Index::Ascii => (start as usize, end as usize),
            Utf16Index::Wide(wide) => (
                wide.byte_index_at(start)
                    .expect("source slice start must lie on a Unicode scalar boundary"),
                wide.byte_index_at(end)
                    .expect("source slice end must lie on a Unicode scalar boundary"),
            ),
        };
        &self.inner.utf8[start..end]
    }

    fn index(&self) -> &Utf16Index {
        self.inner.index.get_or_init(|| {
            let utf8 = self.inner.utf8.as_str();
            if utf8.is_ascii() {
                return Utf16Index::Ascii;
            }

            let scalar_count = bytecount::num_chars(utf8.as_bytes());
            let mut units: Vec<u16> = Vec::with_capacity(utf8.len());
            let mut utf16_to_utf8 = Vec::with_capacity(utf8.len() + 1);
            let mut utf8_to_utf16 = Vec::with_capacity(scalar_count + 1);

            for (byte_offset, character) in utf8.char_indices() {
                utf8_to_utf16.push((byte_offset as u32, units.len() as u32));
                utf16_to_utf8.push(byte_offset as u32);
                let before = units.len();
                let mut buffer = [0_u16; 2];
                units.extend_from_slice(character.encode_utf16(&mut buffer));
                if units.len() - before == 2 {
                    utf16_to_utf8.push(SURROGATE_HALF);
                }
            }

            utf8_to_utf16.push((utf8.len() as u32, units.len() as u32));
            utf16_to_utf8.push(utf8.len() as u32);
            Utf16Index::Wide(WideIndex {
                units,
                utf16_to_utf8,
                utf8_to_utf16,
            })
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
