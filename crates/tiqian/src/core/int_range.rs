// Rust 原生辅助文件：映射 Kotlin IntRange 的通用区间语义。
//
// Kotlin IntRange 的两个端点均包含。EMPTY 使用 start = 1、end = 0 表示，
// 与 Kotlin IntRange.EMPTY 的空区间语义一致。

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IntRange {
    pub start: i32,
    pub end_inclusive: i32,
}

impl IntRange {
    pub const EMPTY: Self = Self {
        start: 1,
        end_inclusive: 0,
    };

    pub const fn new(start: i32, end_inclusive: i32) -> Self {
        Self {
            start,
            end_inclusive,
        }
    }

    pub const fn first(self) -> i32 {
        self.start
    }

    pub const fn last(self) -> i32 {
        self.end_inclusive
    }

    pub const fn is_empty(self) -> bool {
        self.start > self.end_inclusive
    }

    pub const fn contains(self, value: i32) -> bool {
        !self.is_empty() && value >= self.start && value <= self.end_inclusive
    }
}

impl IntoIterator for IntRange {
    type Item = i32;
    type IntoIter = std::ops::RangeInclusive<i32>;

    fn into_iter(self) -> Self::IntoIter {
        self.start..=self.end_inclusive
    }
}
