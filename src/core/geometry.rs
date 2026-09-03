// 对应 Kotlin 源文件：engine/src/commonMain/kotlin/org/tiqian/core/Geometry.kt

use std::ops::{Add, AddAssign, Sub, SubAssign};
use std::fmt;

/// 从 source text 起点开始的 Unicode scalar 数量。
///
/// 此类型不表示 UTF-8 byte offset 或容器下标。转换到 Rust 容器边界时应显式使用
/// [`Self::value`] 并转换为 `usize`。
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ScalarOffset(i32);

impl ScalarOffset {
    pub const ZERO: Self = Self(0);

    pub const fn new(value: i32) -> Self {
        assert!(value >= 0, "ScalarOffset must be non-negative.");
        Self(value)
    }

    pub const fn value(self) -> i32 {
        self.0
    }

    pub const fn checked_sub(self, amount: i32) -> Option<Self> {
        if amount < 0 || self.0 < amount {
            None
        } else {
            Some(Self(self.0 - amount))
        }
    }
}

/// 从裸整数构造 Unicode scalar source offset。
#[inline]
pub const fn scalar_offset(value: i32) -> ScalarOffset {
    ScalarOffset::new(value)
}

impl fmt::Display for ScalarOffset {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl Add<i32> for ScalarOffset {
    type Output = Self;

    fn add(self, right: i32) -> Self::Output {
        Self::new(self.0 + right)
    }
}

impl AddAssign<i32> for ScalarOffset {
    fn add_assign(&mut self, right: i32) {
        *self = *self + right;
    }
}

impl Sub<i32> for ScalarOffset {
    type Output = Self;

    fn sub(self, right: i32) -> Self::Output {
        Self::new(self.0 - right)
    }
}

impl SubAssign<i32> for ScalarOffset {
    fn sub_assign(&mut self, right: i32) {
        *self = *self - right;
    }
}

impl Sub for ScalarOffset {
    type Output = i32;

    fn sub(self, right: Self) -> Self::Output {
        self.0 - right.0
    }
}

/// Source text 的半开 scalar 区间 `[start, end)`。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TextRange {
    start: ScalarOffset,
    end: ScalarOffset,
}

impl TextRange {
    pub fn new(start: ScalarOffset, end: ScalarOffset) -> Self {
        assert!(
            start <= end,
            "TextRange start must not be greater than end."
        );

        Self { start, end }
    }

    pub fn start(self) -> ScalarOffset {
        self.start
    }

    pub fn end(self) -> ScalarOffset {
        self.end
    }

    pub fn length(self) -> i32 {
        self.end - self.start
    }

    pub fn is_empty(self) -> bool {
        self.length() == 0
    }
}

/// 从裸整数构造半开 scalar source range `[start, end)`。
#[inline]
pub fn text_range(start: i32, end: i32) -> TextRange {
    TextRange::new(scalar_offset(start), scalar_offset(end))
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl Rect {
    pub fn width(self) -> f32 {
        self.right - self.left
    }

    pub fn height(self) -> f32 {
        self.bottom - self.top
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LayoutConstraints {
    max_width: f32,
    max_height: f32,
    max_lines: i32,
}

impl LayoutConstraints {
    /// 最大行盒数（`MaxLinesLineTruncation`）。
    ///
    /// 布局在完整文本上运行（断行/两端对齐不受影响——被截断的中间行保持两端对齐）；
    /// 仅限制输出行数，并将截断记录在 `LayoutDebugInfo.maxLinesDecision` 中。
    ///
    /// Kotlin 默认参数的 Rust 映射：`maxHeight = Float.POSITIVE_INFINITY`、
    /// `maxLines = Int.MAX_VALUE`。
    pub fn with_defaults(max_width: f32) -> Self {
        Self::new(max_width, f32::INFINITY, i32::MAX)
    }

    pub fn with_max_height(max_width: f32, max_height: f32) -> Self {
        Self::new(max_width, max_height, i32::MAX)
    }

    pub fn with_max_lines(max_width: f32, max_lines: i32) -> Self {
        Self::new(max_width, f32::INFINITY, max_lines)
    }

    pub fn new(max_width: f32, max_height: f32, max_lines: i32) -> Self {
        assert!(max_width > 0.0, "maxWidth must be positive.");
        assert!(max_height > 0.0, "maxHeight must be positive.");
        assert!(max_lines > 0, "maxLines must be positive.");

        Self {
            max_width,
            max_height,
            max_lines,
        }
    }

    pub fn max_width(self) -> f32 {
        self.max_width
    }

    pub fn max_height(self) -> f32 {
        self.max_height
    }

    pub fn max_lines(self) -> i32 {
        self.max_lines
    }
}
