// 对应 Kotlin 源文件：engine/src/commonMain/kotlin/org/tiqian/core/Geometry.kt

/// FIXME(UTF-16)：Kotlin 的 source range 以 UTF-16 code unit 计数。`start` 与 `end`
/// 必须保持该语义，不能直接用作 Rust UTF-8 byte index；实际索引转换使用 Rust 原生
/// `Text` (UTF-16 优化)。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TextRange {
    start: i32,
    end: i32,
}

impl TextRange {
    pub fn new(start: i32, end: i32) -> Self {
        assert!(
            start <= end,
            "TextRange start must not be greater than end."
        );
        assert!(start >= 0, "TextRange start must be non-negative.");

        Self { start, end }
    }

    pub fn start(self) -> i32 {
        self.start
    }

    pub fn end(self) -> i32 {
        self.end
    }

    pub fn length(self) -> i32 {
        self.end - self.start
    }

    pub fn is_empty(self) -> bool {
        self.length() == 0
    }
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
