// 对应 Kotlin 源文件：engine/src/commonMain/kotlin/org/tiqian/core/Units.kt

/// `ic` —— 提椠的 CJK 原生长度单位（ADR 0034）：N 个**字身框**进格。直接采用 W3C CSS
/// Values L4 的 `ic` 单位（「表意字身的 advance」），即字身框宽。CSS 用探测 '水'(U+6C34)
/// 字形定义它；提椠用字体声明的字身框（ADR 0002 的 BASE `ideo/idtp`）解析它——同一单位，
/// 来源更稳。
///
/// `ic` 只是个计数；解析成 px 时按上下文的字身框进格 [`Ic::to_px`]：段级锚段落基准字号、
/// 行内锚该 gap owner 的字号（与 ADR 0030 per-gap-owner 一致）。横排全宽 CJK 的字身框宽 =
/// 1em = 字号，故 `Ic(n).to_px(font_size) = n × font_size`——数值同旧「em」，价值在语义 +
/// 类型安全 + 锚点明确。`font_size` 自身**不**用 `ic`（它定义了一个字身框）。
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Ic {
    pub count: f32,
}

impl Ic {
    pub const ZERO: Self = Self { count: 0.0 };

    /// 解析成 px：`em_px` = 该上下文的字身框进格（横排全宽 CJK = 字号）。
    pub fn to_px(self, em_px: f32) -> f32 {
        self.count * em_px
    }
}

impl std::ops::Add for Ic {
    type Output = Self;

    fn add(self, other: Self) -> Self::Output {
        Self {
            count: self.count + other.count,
        }
    }
}

impl std::ops::Neg for Ic {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self { count: -self.count }
    }
}

/// Kotlin `Float.ic` 与 `Int.ic` 扩展属性的 Rust 映射。
pub trait IcLiteral {
    fn ic(self) -> Ic;
}

impl IcLiteral for f32 {
    fn ic(self) -> Ic {
        Ic { count: self }
    }
}

impl IcLiteral for i32 {
    fn ic(self) -> Ic {
        Ic { count: self as f32 }
    }
}
