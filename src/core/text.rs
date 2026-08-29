use std::borrow::Borrow;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::sync::{Arc, OnceLock};

use super::geometry::TextRange;

/// A view over shared UTF-8 storage. Slicing clones the `Arc` and narrows the range,
/// so cluster-sized substrings allocate nothing and reuse the parent index.
pub struct Text {
    inner: Arc<TextInner>,
    byte_start: u32,
    byte_end: u32,
    utf16_start: u32,
    /// [`FULL_VIEW`] when the view runs to the end of `inner`, whose UTF-16 length is
    /// only known once the index exists.
    utf16_end: u32,
}

struct TextInner {
    utf8: String,
    index: OnceLock<Utf16Index>,
}

const FULL_VIEW: u32 = u32::MAX;

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
        &self.inner.utf8[self.byte_start as usize..self.byte_end as usize]
    }

    #[inline]
    pub fn utf16_len(&self) -> i32 {
        (self.resolved_utf16_end() - self.utf16_start) as i32
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.byte_start == self.byte_end
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
        let absolute = (self.utf16_start + offset as u32) as usize;
        if absolute >= self.resolved_utf16_end() as usize {
            return None;
        }
        match self.index() {
            Utf16Index::Ascii => Some(i32::from(self.inner.utf8.as_bytes()[absolute])),
            Utf16Index::Wide(wide) => wide.unit_at(absolute),
        }
    }

    pub fn code_point_at_compat(&self, offset: i32, end: i32) -> i32 {
        debug_assert!(
            offset >= 0 && offset < end && end <= self.utf16_len(),
            "UTF-16 code point range must lie within the text"
        );
        let absolute = (self.utf16_start + offset as u32) as usize;
        let Utf16Index::Wide(wide) = self.index() else {
            return i32::from(self.inner.utf8.as_bytes()[absolute]);
        };
        let high = i32::from(wide.units[absolute]);
        if !(HIGH_SURROGATE_START..=HIGH_SURROGATE_END).contains(&high) || offset + 1 >= end {
            return high;
        }
        let low = i32::from(wide.units[absolute + 1]);
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
        debug_assert!(
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

    /// Byte index into [`Self::as_str`], not into the shared storage.
    #[inline]
    pub fn utf8_byte_index_at(&self, utf16_offset: i32) -> Option<usize> {
        if utf16_offset < 0 || utf16_offset > self.utf16_len() {
            return None;
        }
        self.absolute_byte_index_at(utf16_offset)
            .map(|byte| byte - self.byte_start as usize)
    }

    /// Accepts a byte offset into [`Self::as_str`].
    pub fn utf16_offset_at(&self, byte_offset: usize) -> Option<i32> {
        let absolute = self.byte_start as usize + byte_offset;
        if absolute > self.byte_end as usize {
            return None;
        }
        match self.index() {
            Utf16Index::Ascii => Some(byte_offset as i32),
            Utf16Index::Wide(wide) => wide
                .utf8_to_utf16
                .binary_search_by_key(&(absolute as u32), |(byte, _)| *byte)
                .ok()
                .map(|index| (wide.utf8_to_utf16[index].1 - self.utf16_start) as i32),
        }
    }

    pub fn slice(&self, range: TextRange) -> &str {
        self.slice_offsets(range.start(), range.end())
    }

    pub fn slice_offsets(&self, start: i32, end: i32) -> &str {
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
            utf16_start: self.utf16_start + range.start() as u32,
            utf16_end: self.utf16_start + range.end() as u32,
        }
    }

    #[inline]
    fn absolute_byte_bounds(&self, start: i32, end: i32) -> (usize, usize) {
        (
            self.absolute_byte_index_at(start)
                .expect("source slice start must lie on a Unicode scalar boundary"),
            self.absolute_byte_index_at(end)
                .expect("source slice end must lie on a Unicode scalar boundary"),
        )
    }

    #[inline]
    fn absolute_byte_index_at(&self, utf16_offset: i32) -> Option<usize> {
        if utf16_offset < 0 {
            return None;
        }
        let absolute = (self.utf16_start + utf16_offset as u32) as i32;
        match self.index() {
            Utf16Index::Ascii => Some(absolute as usize),
            Utf16Index::Wide(wide) => wide.byte_index_at(absolute),
        }
    }

    #[inline]
    fn resolved_utf16_end(&self) -> u32 {
        if self.utf16_end != FULL_VIEW {
            return self.utf16_end;
        }
        match self.index() {
            Utf16Index::Ascii => self.inner.utf8.len() as u32,
            Utf16Index::Wide(wide) => wide.units.len() as u32,
        }
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
            byte_start: self.byte_start,
            byte_end: self.byte_end,
            utf16_start: self.utf16_start,
            utf16_end: self.utf16_end,
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
            utf16_start: 0,
            utf16_end: FULL_VIEW,
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
