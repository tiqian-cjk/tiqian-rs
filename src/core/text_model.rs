// 对应 Kotlin 源文件：engine/src/commonMain/kotlin/org/tiqian/core/TextModel.kt

use crate::common::HashSet;

use super::geometry::{ScalarOffset, TextRange};
use super::text::Text;
use super::units::Ic;

#[derive(Clone, Debug, PartialEq)]
pub struct TiqianTextContent {
    pub text: Text,
    pub spans: Vec<TextSpan>,
    /// 即使不携带影响布局的样式，也必须成为 cluster 边界的 scalar source offset。链接、颜色、
    /// 下划线等仅渲染范围需要精确的占用几何；否则，拉丁 cluster 中在尾随标点前结束的范围
    /// （`template|.`）会退化为按比例切片。
    pub source_boundaries: HashSet<ScalarOffset>,
    /// 内部西文 token 使用显式断行策略的、影响布局的 scalar source range。
    pub line_break_spans: Vec<LineBreakSpan>,
    /// `VerbatimRangeAutoSpace`：逐字范围（行内代码、技术文本）内部的 CJK↔Western 边界
    /// 不接收自动间距。严格位于范围内的边界被抑制；范围外缘保留周围正文的契约。
    pub auto_space_suppressed_ranges: Vec<TextRange>,
}

impl TiqianTextContent {
    pub fn new(text: Text) -> Self {
        Self {
            text,
            spans: Vec::new(),
            source_boundaries: HashSet::new(),
            line_break_spans: Vec::new(),
            auto_space_suppressed_ranges: Vec::new(),
        }
    }

    pub fn builder(text: Text) -> TiqianTextContentBuilder {
        TiqianTextContentBuilder {
            content: Self::new(text),
        }
    }
}

pub struct TiqianTextContentBuilder {
    content: TiqianTextContent,
}

impl TiqianTextContentBuilder {
    pub fn spans(mut self, spans: Vec<TextSpan>) -> Self {
        self.content.spans = spans;
        self
    }

    pub fn source_boundaries(mut self, source_boundaries: HashSet<ScalarOffset>) -> Self {
        self.content.source_boundaries = source_boundaries;
        self
    }

    pub fn line_break_spans(mut self, line_break_spans: Vec<LineBreakSpan>) -> Self {
        self.content.line_break_spans = line_break_spans;
        self
    }

    pub fn auto_space_suppressed_ranges(mut self, ranges: Vec<TextRange>) -> Self {
        self.content.auto_space_suppressed_ranges = ranges;
        self
    }

    pub fn build(self) -> TiqianTextContent {
        self.content
    }
}

/// `LinkAddressDisplayGate`：链接可见文字是否为其自身地址——与 target 相同，或 target
/// 去掉 `https://` / `http://` / `mailto:` 前缀。只有这类链接使用
/// `LineBreakPolicy::ProgressiveTechnical`；其他链接文字保留正文断行。
pub mod link_address_display {
    use super::Text;

    pub fn displays_address(display: &Text, target: &str) -> bool {
        if display.is_empty() || target.is_empty() {
            return false;
        }
        if display.as_str() == target {
            return true;
        }
        target == format!("https://{display}")
            || target == format!("http://{display}")
            || target == format!("mailto:{display}")
    }
}

/// 选择加入具名、前端无关断行策略的 source range。
#[derive(Clone, Debug, PartialEq)]
pub struct LineBreakSpan {
    pub range: TextRange,
    pub policy: LineBreakPolicy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineBreakPolicy {
    /// 技术行内文本：优先 structural-symbol/camel 边界，然后是不显示连字符的语言音节边界，
    /// 最后是安全的 source-grapheme 紧急边界。
    ProgressiveTechnical,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextSpan {
    pub range: TextRange,
    pub style: TextStyle,
}

/// 行内文本范围如何为边界间距归属到周围正文。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum InlineAttachment {
    #[default]
    None,
    /// 范围属于前面的文本，如行内脚注引用。生成的前导 CJK/non-CJK gap 移到范围尾缘；
    /// 正常的行边修剪会在该边结束一行时将其丢弃。
    Previous,
}

/// 围绕 source text range 的、由布局拥有的行内盒边缘。
///
/// text shaper 只测量 glyph。具有原生行内语义的前端（例如 DOM padding、border、margin，
/// 或生成的 `::before` / `::after` 内容）使用此 span，在断行中保留相同的前导和尾随 advance。
/// `inline_start` 与 `inline_end` 是当前横排 writing mode 中的物理 px，因此允许负 margin。
#[derive(Clone, Debug, PartialEq)]
pub struct InlineBoxSpan {
    pub range: TextRange,
    pub inline_start: f32,
    pub inline_end: f32,
    /// 独立行内盒在其两个真实外缘呈现的东亚间距类。`Narrow` 使每个 boxed inline 都具有相同的
    /// 面向 CJK 的 autospace 契约，无论其第一个 source 字符是字母、`.`、`/` 或另一种符号。
    /// `Source` 将两个边缘都留给 source 字符，供仅为测量包装的 span 使用。
    pub outer_spacing: InlineBoxOuterSpacing,
}

impl InlineBoxSpan {
    pub fn new(range: TextRange) -> Self {
        Self {
            range,
            inline_start: 0.0,
            inline_end: 0.0,
            outer_spacing: InlineBoxOuterSpacing::Narrow,
        }
    }

    pub fn with_edges(range: TextRange, inline_start: f32, inline_end: f32) -> Self {
        Self {
            range,
            inline_start,
            inline_end,
            outer_spacing: InlineBoxOuterSpacing::Narrow,
        }
    }

    pub fn with_all(
        range: TextRange,
        inline_start: f32,
        inline_end: f32,
        outer_spacing: InlineBoxOuterSpacing,
    ) -> Self {
        Self {
            range,
            inline_start,
            inline_end,
            outer_spacing,
        }
    }

    pub fn builder(range: TextRange) -> InlineBoxSpanBuilder {
        InlineBoxSpanBuilder {
            span: Self::new(range),
        }
    }
}

pub struct InlineBoxSpanBuilder {
    span: InlineBoxSpan,
}

impl InlineBoxSpanBuilder {
    pub fn inline_start(mut self, value: f32) -> Self {
        self.span.inline_start = value;
        self
    }

    pub fn inline_end(mut self, value: f32) -> Self {
        self.span.inline_end = value;
        self
    }

    pub fn outer_spacing(mut self, value: InlineBoxOuterSpacing) -> Self {
        self.span.outer_spacing = value;
        self
    }

    pub fn build(self) -> InlineBoxSpan {
        self.span
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum InlineBoxOuterSpacing {
    #[default]
    Narrow,
    Source,
}

/// 没有行内对象 source fallback 的文本投影使用的结构 token。
pub const INLINE_OBJECT_REPLACEMENT_CHAR: char = '\u{FFFC}';

/// 由 provider 提供的、具有上限并优先分配的行内对象边界拉伸语义类。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum InlineObjectPreferredStretchKind {
    PunctuationTrailing,
    Relation,
    BinaryOperator,
}

#[derive(Clone, Copy, Debug, PartialEq)]
/// 禁止手工构造：必须使用 [`InlineObjectPreferredStretch::new`]，以保留 Kotlin 的构造校验。
pub struct InlineObjectPreferredStretch {
    pub kind: InlineObjectPreferredStretchKind,
    /// 已包含在 provider 测量的对象 advance 中的空白。
    pub natural_width: f32,
    /// 在下一个优先类开始前达到的绝对最终宽度。
    pub target_width: f32,
}

impl InlineObjectPreferredStretch {
    pub fn new(
        kind: InlineObjectPreferredStretchKind,
        natural_width: f32,
        target_width: f32,
    ) -> Self {
        assert!(
            natural_width.is_finite() && natural_width >= 0.0,
            "Inline-object preferred stretch natural width must be finite and non-negative"
        );
        assert!(
            target_width.is_finite() && target_width > natural_width,
            "Inline-object preferred stretch target must be finite and exceed its natural width"
        );
        Self {
            kind,
            natural_width,
            target_width,
        }
    }

    pub fn capacity(self) -> f32 {
        self.target_width - self.natural_width
    }
}

/// 在行内对象一个边缘允许的段落调整与断行。
///
/// `preferred_stretch` 将测得的自然空白和绝对目标暴露给由 provider 定义的优先 pass：
/// 标点尾随空白优先，关系空白第二，二元运算符空白第三。`participates_in_uniform_stretch`
/// 还将边缘暴露给最终的等间距 pass；优先 pass 不排除其获得最终均分。`prevents_line_break`
/// 关闭仅用于调整的边界，而不把它与真正的公式断点混淆。
///
/// 在尾随边界，`shrink_capacity` 是已经包含在对象测量 advance 中的物理空白，可作为最后手段的
/// 压缩资源移除。`line_end_discardable_advance` 是该边界成为自动行尾时必须消失的子集；未断行时仍
/// 保留。因为移除前导 shrink 或 discard 还需要移动对象 paint origin，故不支持它们。Opaque 对象
/// 默认使用 `Fixed`。
#[derive(Clone, Debug, PartialEq)]
/// 禁止手工构造：使用 [`InlineObjectBoundaryAdjustment::FIXED`] 或 builder，以保留 Kotlin 的构造校验。
pub struct InlineObjectBoundaryAdjustment {
    pub participates_in_uniform_stretch: bool,
    pub preferred_stretch: Option<InlineObjectPreferredStretch>,
    pub shrink_capacity: f32,
    pub line_end_discardable_advance: f32,
    pub prevents_line_break: bool,
}

impl InlineObjectBoundaryAdjustment {
    pub const FIXED: Self = Self {
        participates_in_uniform_stretch: false,
        preferred_stretch: None,
        shrink_capacity: 0.0,
        line_end_discardable_advance: 0.0,
        prevents_line_break: false,
    };

    pub fn builder() -> InlineObjectBoundaryAdjustmentBuilder {
        InlineObjectBoundaryAdjustmentBuilder {
            adjustment: Self::FIXED,
        }
    }

    fn new(
        participates_in_uniform_stretch: bool,
        preferred_stretch: Option<InlineObjectPreferredStretch>,
        shrink_capacity: f32,
        line_end_discardable_advance: f32,
        prevents_line_break: bool,
    ) -> Self {
        assert!(
            shrink_capacity.is_finite() && shrink_capacity >= 0.0,
            "Inline-object boundary shrink capacity must be finite and non-negative"
        );
        assert!(
            line_end_discardable_advance.is_finite() && line_end_discardable_advance >= 0.0,
            "Inline-object line-end discardable advance must be finite and non-negative"
        );
        Self {
            participates_in_uniform_stretch,
            preferred_stretch,
            shrink_capacity,
            line_end_discardable_advance,
            prevents_line_break,
        }
    }
}

pub struct InlineObjectBoundaryAdjustmentBuilder {
    adjustment: InlineObjectBoundaryAdjustment,
}

impl InlineObjectBoundaryAdjustmentBuilder {
    pub fn participates_in_uniform_stretch(mut self, value: bool) -> Self {
        self.adjustment.participates_in_uniform_stretch = value;
        self
    }

    pub fn preferred_stretch(mut self, value: InlineObjectPreferredStretch) -> Self {
        self.adjustment.preferred_stretch = Some(value);
        self
    }

    pub fn shrink_capacity(mut self, value: f32) -> Self {
        self.adjustment.shrink_capacity = value;
        self
    }

    pub fn line_end_discardable_advance(mut self, value: f32) -> Self {
        self.adjustment.line_end_discardable_advance = value;
        self
    }

    pub fn prevents_line_break(mut self, value: bool) -> Self {
        self.adjustment.prevents_line_break = value;
        self
    }

    pub fn build(self) -> InlineObjectBoundaryAdjustment {
        InlineObjectBoundaryAdjustment::new(
            self.adjustment.participates_in_uniform_stretch,
            self.adjustment.preferred_stretch,
            self.adjustment.shrink_capacity,
            self.adjustment.line_end_discardable_advance,
            self.adjustment.prevents_line_break,
        )
    }
}

/// 占据 `range` 的一个不可分割行内对象。
///
/// `range` 保持 source-faithful：它可覆盖对象的非空 alternate text，或在 host 没有文本 fallback
/// 时覆盖一个 `INLINE_OBJECT_REPLACEMENT_CHAR`。被覆盖的 source 不经 font shape；`advance` 是测得的
/// margin-box 宽度，`ascent` 与 `descent` 是它相对周围 text baseline 上下的可见范围。布局使用这三个
/// 值进行断行与逐行度量。对象先消耗相邻 text face 间的既有空间；baseline grid 只为剩余碰撞不足扩张。
/// 平台 renderer 拥有实际对象。
#[derive(Clone, Debug, PartialEq)]
pub struct InlineObjectSpan {
    pub range: TextRange,
    pub advance: f32,
    pub ascent: f32,
    pub descent: f32,
    pub leading_boundary: InlineObjectBoundaryAdjustment,
    pub trailing_boundary: InlineObjectBoundaryAdjustment,
}

impl InlineObjectSpan {
    pub fn with_fixed_boundaries(
        range: TextRange,
        advance: f32,
        ascent: f32,
        descent: f32,
    ) -> Self {
        Self {
            range,
            advance,
            ascent,
            descent,
            leading_boundary: InlineObjectBoundaryAdjustment::FIXED,
            trailing_boundary: InlineObjectBoundaryAdjustment::FIXED,
        }
    }

    pub fn new(
        range: TextRange,
        advance: f32,
        ascent: f32,
        descent: f32,
        leading_boundary: InlineObjectBoundaryAdjustment,
        trailing_boundary: InlineObjectBoundaryAdjustment,
    ) -> Self {
        Self {
            range,
            advance,
            ascent,
            descent,
            leading_boundary,
            trailing_boundary,
        }
    }

    pub fn with_leading_boundary(
        range: TextRange,
        advance: f32,
        ascent: f32,
        descent: f32,
        leading_boundary: InlineObjectBoundaryAdjustment,
    ) -> Self {
        Self {
            range,
            advance,
            ascent,
            descent,
            leading_boundary,
            trailing_boundary: InlineObjectBoundaryAdjustment::FIXED,
        }
    }

    pub fn with_trailing_boundary(
        range: TextRange,
        advance: f32,
        ascent: f32,
        descent: f32,
        trailing_boundary: InlineObjectBoundaryAdjustment,
    ) -> Self {
        Self {
            range,
            advance,
            ascent,
            descent,
            leading_boundary: InlineObjectBoundaryAdjustment::FIXED,
            trailing_boundary,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextStyle {
    pub font_families: Vec<String>,
    pub font_size: f32,
    pub locale: String,
    /// OpenType weight axis (400 = Regular, 700 = Bold); drives the shaped typeface.
    pub font_weight: i32,
    /// Slant axis: italic/oblique typeface when the family offers one (ADR 0030 B 档).
    pub italic: bool,
    /// 显式作者/样式 baseline offset，单位 px（+down）。它独立于引擎的 script/size metric alignment
    /// shift 并叠加其上，例如由 Compose `SpanStyle.baselineShift` 降低的 reference superscript。
    pub baseline_shift: f32,
    /// 语义边界归属；独立于 font metrics 与 baseline geometry。
    pub inline_attachment: InlineAttachment,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            font_families: Vec::new(),
            font_size: 16.0,
            locale: "zh-Hans".to_owned(),
            font_weight: 400,
            italic: false,
            baseline_shift: 0.0,
            inline_attachment: InlineAttachment::None,
        }
    }
}

impl TextStyle {
    pub fn builder() -> TextStyleBuilder {
        TextStyleBuilder {
            style: Self::default(),
        }
    }
}

pub struct TextStyleBuilder {
    style: TextStyle,
}

impl TextStyleBuilder {
    pub fn font_families(mut self, value: Vec<String>) -> Self {
        self.style.font_families = value;
        self
    }
    pub fn font_size(mut self, value: f32) -> Self {
        self.style.font_size = value;
        self
    }
    pub fn locale(mut self, value: String) -> Self {
        self.style.locale = value;
        self
    }
    pub fn font_weight(mut self, value: i32) -> Self {
        self.style.font_weight = value;
        self
    }
    pub fn italic(mut self, value: bool) -> Self {
        self.style.italic = value;
        self
    }
    pub fn baseline_shift(mut self, value: f32) -> Self {
        self.style.baseline_shift = value;
        self
    }
    pub fn inline_attachment(mut self, value: InlineAttachment) -> Self {
        self.style.inline_attachment = value;
        self
    }
    pub fn build(self) -> TextStyle {
        self.style
    }
}

/// 对一个 SOURCE text range 的行内装饰（ADR 0018）。display substitution 不影响 span 语义。
/// decoration 纯属 render-geometry：从不参与 metrics、line breaking 或 justification。
#[derive(Clone, Debug, PartialEq)]
pub struct DecorationSpan {
    pub range: TextRange,
    pub kind: DecorationKind,
}

/// SOURCE range 上每个 span 的文本颜色（ARGB）——rich-text 颜色（ADR 0030 A 档）。
/// 仅渲染：与 `DecorationSpan` 一样，绝不影响 metrics、breaking 或 justification，因此它与 layout
/// model 并列而不在其中。平台无关（`argb` Int），所以 frontend contract 不携带 Skia type。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ColorSpan {
    pub start: ScalarOffset,
    pub end: ScalarOffset,
    pub argb: i32,
}

/// SOURCE range 上的 render/semantic rich-text role。这些 span 不添加 metrics、breaking penalty 或
/// justification rule：它们在 layout 后复用 `LayoutResult` geometry，source text 与 CJK paragraph
/// decision 因而仍由 core pipeline 拥有。它们的 boundary 仍可经由
/// `TiqianTextContent.source_boundaries` 传入，使引擎暴露精确 range geometry，而非切过合并 cluster。
#[derive(Clone, Debug, PartialEq)]
pub struct RichTextSpan {
    pub range: TextRange,
    pub role: RichTextRole,
    pub paint: RichTextPaint,
}

impl RichTextSpan {
    pub fn new(range: TextRange, role: RichTextRole) -> Self {
        Self {
            range,
            role,
            paint: RichTextPaint::default(),
        }
    }

    pub fn with_paint(range: TextRange, role: RichTextRole, paint: RichTextPaint) -> Self {
        Self { range, role, paint }
    }
}

#[derive(Clone, Debug, PartialEq)]
/// 禁止手工构造：必须使用 [`RichTextPaint::builder`]，以保留 Kotlin 的构造校验。
pub struct RichTextPaint {
    /// 可选 ARGB paint。None 表示“继承当前 text color/default role paint”。
    pub argb: Option<i32>,
    /// 行装饰的 stroke pattern。background role 忽略此字段。
    pub line_pattern: RichTextLinePattern,
    /// background role 的 box geometry 与 paint。行装饰忽略此字段。
    pub background: RichTextBackgroundPaint,
    /// 两个紧邻且具有相同 paint 的 range 共同分摊的总视觉空隙。
    pub adjacent_same_style_clearance: f32,
}

impl Default for RichTextPaint {
    fn default() -> Self {
        Self {
            argb: None,
            line_pattern: RichTextLinePattern::Solid,
            background: RichTextBackgroundPaint::default(),
            adjacent_same_style_clearance: 0.0,
        }
    }
}

impl RichTextPaint {
    pub fn builder() -> RichTextPaintBuilder {
        RichTextPaintBuilder {
            paint: Self::default(),
        }
    }

    fn checked(self) -> Self {
        assert!(
            self.adjacent_same_style_clearance.is_finite()
                && self.adjacent_same_style_clearance >= 0.0
        );
        self
    }
}

pub struct RichTextPaintBuilder {
    paint: RichTextPaint,
}

impl RichTextPaintBuilder {
    pub fn argb(mut self, value: i32) -> Self {
        self.paint.argb = Some(value);
        self
    }
    pub fn line_pattern(mut self, value: RichTextLinePattern) -> Self {
        self.paint.line_pattern = value;
        self
    }
    pub fn background(mut self, value: RichTextBackgroundPaint) -> Self {
        self.paint.background = value;
        self
    }
    pub fn adjacent_same_style_clearance(mut self, value: f32) -> Self {
        self.paint.adjacent_same_style_clearance = value;
        self
    }
    pub fn build(self) -> RichTextPaint {
        self.paint.checked()
    }
}

#[derive(Clone, Debug, PartialEq)]
/// 禁止手工构造：必须使用 [`RichTextBackgroundPaint::builder`]，以保留 Kotlin 的构造校验。
pub struct RichTextBackgroundPaint {
    /// glyph edge 与每个水平 box edge 间的固定空间，单位 layout unit。
    pub horizontal_padding: f32,
    /// 所选 typographic box 上下的额外空间，单位物理 layout unit。
    pub vertical_padding: f32,
    /// frontend 重放最终 background rectangle 时使用的 radius。
    pub corner_radius: f32,
    /// source range 在另一 visual line 延续的边缘使用的 radius。默认 `corner_radius`，保留现有的
    /// per-line rounded-box appearance，除非 authored role 显式区分 continuation edge。
    pub continuation_corner_radius: f32,
    pub metric_policy: RichTextBackgroundMetricPolicy,
    /// 最终 box 是填充还是描边；两种模式复用相同的测量 geometry。
    pub draw_style: RichTextBackgroundDrawStyle,
}

impl Default for RichTextBackgroundPaint {
    fn default() -> Self {
        Self {
            horizontal_padding: 0.0,
            vertical_padding: 0.0,
            corner_radius: 0.0,
            continuation_corner_radius: 0.0,
            metric_policy: RichTextBackgroundMetricPolicy::MarkedFaces,
            draw_style: RichTextBackgroundDrawStyle::Fill,
        }
    }
}

impl RichTextBackgroundPaint {
    pub fn builder() -> RichTextBackgroundPaintBuilder {
        RichTextBackgroundPaintBuilder {
            paint: Self::default(),
            continuation_corner_radius_was_set: false,
        }
    }

    fn checked(self) -> Self {
        assert!(self.horizontal_padding.is_finite() && self.horizontal_padding >= 0.0);
        assert!(self.vertical_padding.is_finite() && self.vertical_padding >= 0.0);
        assert!(self.corner_radius.is_finite() && self.corner_radius >= 0.0);
        assert!(
            self.continuation_corner_radius.is_finite() && self.continuation_corner_radius >= 0.0
        );
        self
    }
}

pub struct RichTextBackgroundPaintBuilder {
    paint: RichTextBackgroundPaint,
    continuation_corner_radius_was_set: bool,
}

impl RichTextBackgroundPaintBuilder {
    pub fn horizontal_padding(mut self, value: f32) -> Self {
        self.paint.horizontal_padding = value;
        self
    }
    pub fn vertical_padding(mut self, value: f32) -> Self {
        self.paint.vertical_padding = value;
        self
    }
    pub fn corner_radius(mut self, value: f32) -> Self {
        self.paint.corner_radius = value;
        self
    }
    pub fn continuation_corner_radius(mut self, value: f32) -> Self {
        self.paint.continuation_corner_radius = value;
        self.continuation_corner_radius_was_set = true;
        self
    }
    pub fn metric_policy(mut self, value: RichTextBackgroundMetricPolicy) -> Self {
        self.paint.metric_policy = value;
        self
    }
    pub fn draw_style(mut self, value: RichTextBackgroundDrawStyle) -> Self {
        self.paint.draw_style = value;
        self
    }
    pub fn build(mut self) -> RichTextBackgroundPaint {
        if !self.continuation_corner_radius_was_set {
            self.paint.continuation_corner_radius = self.paint.corner_radius;
        }
        self.paint.checked()
    }
}

#[derive(Clone, Debug, PartialEq)]
/// 禁止手工构造带字段变体：使用 [`RichTextBackgroundDrawStyle::border`]，以保留 Kotlin 的构造校验。
pub enum RichTextBackgroundDrawStyle {
    Fill,
    /// 由 frontend 保持在 resolved box 内的物理 layout-unit stroke。
    Border {
        stroke_width: f32,
    },
}

impl RichTextBackgroundDrawStyle {
    pub fn border(stroke_width: f32) -> Self {
        assert!(stroke_width.is_finite() && stroke_width > 0.0);
        Self::Border { stroke_width }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RichTextBackgroundMetricPolicy {
    /// 合并 marked range 覆盖的实际 face 的声明 metric box。
    #[default]
    MarkedFaces,
    /// 对整个 run 的 resolved text style 使用一个 ideographic/reference metric box。
    UniformTextStyle,
    /// 即使 marked run 有更小的 inline font，也使用 paragraph text style。
    UniformParagraphStyle,
}

#[derive(Clone, Debug, PartialEq)]
/// 禁止手工构造带字段变体：使用 [`RichTextLinePattern::dashed`] 或
/// [`RichTextLinePattern::dotted`]，以保留 Kotlin 的构造校验。
pub enum RichTextLinePattern {
    Solid,
    /// 由 frontend 提供的物理 layout-unit dash geometry。
    Dashed {
        stroke_width: f32,
        dash_length: f32,
        gap_length: f32,
    },
    /// 具有 frontend 提供 diameter 与可见 edge-to-edge gap 的圆点。
    Dotted {
        dot_diameter: f32,
        gap_length: f32,
    },
}

impl RichTextLinePattern {
    pub fn dashed(stroke_width: f32, dash_length: f32, gap_length: f32) -> Self {
        assert!(stroke_width.is_finite() && stroke_width > 0.0);
        assert!(dash_length.is_finite() && dash_length > 0.0);
        assert!(gap_length.is_finite() && gap_length > 0.0);
        Self::Dashed {
            stroke_width,
            dash_length,
            gap_length,
        }
    }

    pub fn dotted(dot_diameter: f32, gap_length: f32) -> Self {
        assert!(dot_diameter.is_finite() && dot_diameter > 0.0);
        assert!(gap_length.is_finite() && gap_length > 0.0);
        Self::Dotted {
            dot_diameter,
            gap_length,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum RichTextRole {
    /// Compose `SpanStyle.background`，每 visual line 绘制一个连续 typographic box。
    Background,
    /// Compose `TextDecoration.Underline`，以 Tiqian line geometry + skip-ink 绘制。
    Underline,
    /// Compose `TextDecoration.LineThrough`，以 Tiqian line geometry 绘制。
    LineThrough,
    /// Link source range。URL/click tag 保存在 model 中；link action 属于 frontend/accessibility slice，
    /// 所以此 role 不隐含 visual fallback 或 navigation。
    Link { target: String },
    /// Renderer-owned technical inline range。它参与共享 progressive technical break policy 但自身不携带
    /// paint，因此 adapter 可提供 code box、border 或 fallback presentation 而不重复 geometry。
    TechnicalInline,
    /// 经 Tiqian builder 创作的 inline code role。其 source 不变；Compose bridge 也经由 `TextSpan`
    /// lowering 其 generic monospace font family。
    InlineCode,
}

/// 行间注（ruby, ADR 0032）：在 base SOURCE `base_range` 上方的、小字号 annotation `text`——本 slice
/// 中是基字上方的拼音。与 `DecorationSpan` 不同，ruby 确实影响布局：它保留行高并使 base 不可断行。
/// `text` 不是 source 的一部分（拼音不进源；复制/搜索保真）——仅存在于此。
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RubySpan {
    pub base_range: TextRange,
    pub text: Text,
    /// 注文专用字体（family 名优先列表）。注音需含 ㄅㄆㄇ 字形的字体、拼音/释义可用各自字体——
    /// 注文字体本就应独立于正文（ADR 0032）。空 = 渲染器默认。
    pub font_families: Vec<String>,
    /// 拼音（上方，ADR 0032）或注音（右侧竖排 ㄅㄆㄇ，ADR 0033）。
    pub kind: RubyKind,
    /// 注文自己的 BCP-47 language。注音默认 `zh-TW`；拼音 None 时继承正文 locale。调用方无需为了
    /// 注音重复声明 language，这也不会改变简体横排正文的 locale/profile。
    pub locale: Option<String>,
}

impl RubySpan {
    pub fn new(base_range: TextRange, text: Text) -> Self {
        Self {
            base_range,
            text,
            font_families: Vec::new(),
            kind: RubyKind::Pinyin,
            locale: None,
        }
    }

    pub fn with_kind(base_range: TextRange, text: Text, kind: RubyKind) -> Self {
        let locale = if kind == RubyKind::Bopomofo {
            Some("zh-TW".to_owned())
        } else {
            None
        };
        Self {
            base_range,
            text,
            font_families: Vec::new(),
            kind,
            locale,
        }
    }

    pub fn builder(base_range: TextRange, text: Text) -> RubySpanBuilder {
        RubySpanBuilder {
            span: Self::new(base_range, text),
            locale_was_set: false,
        }
    }
}

pub struct RubySpanBuilder {
    span: RubySpan,
    locale_was_set: bool,
}

impl RubySpanBuilder {
    pub fn font_families(mut self, value: Vec<String>) -> Self {
        self.span.font_families = value;
        self
    }
    pub fn kind(mut self, value: RubyKind) -> Self {
        self.span.kind = value;
        self
    }
    pub fn locale(mut self, value: Option<String>) -> Self {
        self.span.locale = value;
        self.locale_was_set = true;
        self
    }
    pub fn build(mut self) -> RubySpan {
        if !self.locale_was_set && self.span.kind == RubyKind::Bopomofo {
            self.span.locale = Some("zh-TW".to_owned());
        }
        self.span
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum RubyKind {
    /// 罗马拼音：注文在基字上方、水平居中（ADR 0032）。
    #[default]
    Pinyin,
    /// 注音符号：注文在基字右侧、ㄅㄆㄇ 竖排 + 调号、纵横对齐（ADR 0033）。
    Bopomofo,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecorationKind {
    /// CLREQ 着重号——每个着重 Han character 下的实心圆点。
    Emphasis,
    /// 示亡号——围绕（已故者）姓名的实心黑框。只要一行能放下，span 跨断行保持不分割；
    /// 无法放下时分割为逐行开口的 segment。
    Mourning,
    /// 专名号——proper noun 下方的直线（横排）。CLREQ 行间线之一：每个 annotated item 每行一条连续
    /// segment，长度匹配 text 的 outer frame，不在一行内拆开或拼接；相邻 item 只缩短它们相邻的边
    /// （≤1/8 em），使两条 mark 可分辨。
    ProperNoun,
    /// 书名号（甲式）——work title 下方的波浪线（横排）。与 `ProperNoun` 使用相同的行间线 segment 规则。
    BookTitle,
}

/// 拼音 ruby 的行间空间不足时如何扩张 baseline grid。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RubyLineHeightMode {
    /// 默认：仅在携带拼音 ruby 的行之前补足缺失空间。
    #[default]
    PerLine,
    /// 在段落中每一行之前添加相同缺失空间。
    UniformParagraph,
}

/// ADR 0018：着重号与被注文字字面底边之间的默认净空，单位 em。
pub const DEFAULT_EMPHASIS_DOT_GAP_EM: f32 = 0.1;

/// InlineObjectMinimumClearance：超大行内墨迹保持的行间净空，单位 em。
pub const DEFAULT_INLINE_OBJECT_MINIMUM_CLEARANCE_EM: f32 = 0.1;

#[derive(Clone, Debug, PartialEq)]
pub struct ParagraphStyle {
    /// 段落仅最后一行的 alignment。CLREQ：“与西文排版不同，中文排版特别是书籍正文排版极少使用
    /// 左齐右不齐，原则上应该进行两端对齐”——justification 是基线行为而非 option：每个 non-last line
    /// 始终 justified（挤压/拉伸已使行长一致）。唯一自由度是 last line：start（默认）、centered 或
    /// end-aligned（落款、引文出处等特殊用法）。single-line paragraph 自身即 last line，因此 heading 和
    /// label 从不拉伸。
    pub last_line_alignment: LastLineAlignment,
    pub writing_mode: WritingMode,
    /// baseline 到 baseline 的 line height，单位与 `TextStyle.fontSize` 相同（engine pixel），不是倍率。
    /// None = `CjkBodyLineHeightDefault`（1.5em——中文正文 leading）。数值可向任一方向 override 默认值，
    /// 但仍会钳至 no-overlap minimum（字面 + 任意 `InterlinearMarkLineSpacingFloor`）：小于内容的 line 会
    /// overlap glyph/mark，所以低于约 1em 的值没有效果（resolution 记录在 `LineSpacingDecisionInfo`）。
    /// 要将 16px font 设为 1.5×，传入 `24.0`，而非 `1.5`。
    pub line_height: Option<f32>,
    /// 段首缩进的显式覆盖，单位 `ic`（字身框，ADR 0034）。`0.ic` 禁用 indent；任何非 None 值无论
    /// measure 都将其固定。None（默认）表示“不指定”→ 由 `first_line_indent_policy` 按行长自适应决定
    /// （CLREQ“段首缩排以两个汉字的空间为标准”，窄行缩一字）。indent 只内缩 first line 的 start edge
    /// （vertical writing 中变为第一 column 的 block-start inset）。首行以 bracket 或 quote 开头无需特殊
    /// 处理：additive glue model 已在每个 line start 修剪 opening punctuation 的 leading blank，这正是
    /// CLREQ“缩减该符号始侧二分之一个汉字大小的空白”。
    pub first_line_indent: Option<Ic>,
    /// 整段缩进（CLREQ §6.2.1.2 段落缩排），单位 `ic`：所有行的始端都内移此值（引用、诗词、标题块）。
    /// `first_line_indent` 叠加其上且相对它，可为负——`block_indent = H.ic, first_line_indent = (-H).ic`
    /// 即“凸排”（首行齐头、次行起缩 H）。每行有效缩进 = `block_indent + 该行 firstLine 部分`，钳到 ≥0。
    pub block_indent: Ic,
    /// 段首缩进随行长自适应的默认策略（仅当 `first_line_indent` 为 None 时生效）。
    pub first_line_indent_policy: MeasureAdaptiveFirstLineIndent,
    /// 行长字号整数倍量化（grid-first，ADR 0007 的完整形态）。
    pub line_length_grid: LineLengthGrid,
    /// 拼音 ruby 的条件式行高策略（ADR 0032）。引擎先用现有行距（`line_height - 基文字面高`）容纳注文；
    /// 能放下时两种模式都不改变行盒。空间不足时，`PerLine`（默认）只加高含注文的行，`UniformParagraph`
    /// 则把同样的增量应用到整段每一行。右侧注音不使用此项。
    pub ruby_line_height_mode: RubyLineHeightMode,
    /// 超出正文字面的行内对象与相邻行可见内容之间必须保留的最小净空，单位 em。
    ///
    /// 行内公式先使用正文行高已经提供的空白，但不能把空白吃到上下墨迹刚好相切；不足时
    /// 只增加“侵入量 + 此净空”尚缺的部分。默认 0.1em，取 TeX `\lineskip` 的同量级
    /// 安全间隙；显式设为 0 可关闭。
    pub inline_object_minimum_clearance_em: f32,
    /// 着重号圆点墨迹上缘与被注文字字面底边之间的显式净空，单位 em。
    ///
    /// CLREQ 规定横排着重号位于文字底端，但没有规定点与字面的精确距离；
    /// 因此距离由排版样式显式决定。引擎以每个 cluster 的真实字面度量定位
    /// 圆点，不从 baseline 或 `line_height` 猜位置。更宽的行高只提供更多容纳
    /// 空间，不会暗中移动着重号。默认值见 `DEFAULT_EMPHASIS_DOT_GAP_EM`
    /// （ADR 0018）。
    pub emphasis_dot_gap_em: f32,
}

impl Default for ParagraphStyle {
    fn default() -> Self {
        Self {
            last_line_alignment: LastLineAlignment::Start,
            writing_mode: WritingMode::HorizontalTb,
            line_height: None,
            first_line_indent: None,
            block_indent: Ic::ZERO,
            first_line_indent_policy: MeasureAdaptiveFirstLineIndent::default(),
            line_length_grid: LineLengthGrid::default(),
            ruby_line_height_mode: RubyLineHeightMode::PerLine,
            inline_object_minimum_clearance_em: DEFAULT_INLINE_OBJECT_MINIMUM_CLEARANCE_EM,
            emphasis_dot_gap_em: DEFAULT_EMPHASIS_DOT_GAP_EM,
        }
    }
}

impl ParagraphStyle {
    pub fn builder() -> ParagraphStyleBuilder {
        ParagraphStyleBuilder {
            style: Self::default(),
        }
    }
}

pub struct ParagraphStyleBuilder {
    style: ParagraphStyle,
}

impl ParagraphStyleBuilder {
    pub fn last_line_alignment(mut self, value: LastLineAlignment) -> Self {
        self.style.last_line_alignment = value;
        self
    }
    pub fn writing_mode(mut self, value: WritingMode) -> Self {
        self.style.writing_mode = value;
        self
    }
    pub fn line_height(mut self, value: Option<f32>) -> Self {
        self.style.line_height = value;
        self
    }
    pub fn first_line_indent(mut self, value: Option<Ic>) -> Self {
        self.style.first_line_indent = value;
        self
    }
    pub fn block_indent(mut self, value: Ic) -> Self {
        self.style.block_indent = value;
        self
    }
    pub fn first_line_indent_policy(mut self, value: MeasureAdaptiveFirstLineIndent) -> Self {
        self.style.first_line_indent_policy = value;
        self
    }
    pub fn line_length_grid(mut self, value: LineLengthGrid) -> Self {
        self.style.line_length_grid = value;
        self
    }
    pub fn ruby_line_height_mode(mut self, value: RubyLineHeightMode) -> Self {
        self.style.ruby_line_height_mode = value;
        self
    }
    pub fn inline_object_minimum_clearance_em(mut self, value: f32) -> Self {
        self.style.inline_object_minimum_clearance_em = value;
        self
    }
    pub fn emphasis_dot_gap_em(mut self, value: f32) -> Self {
        self.style.emphasis_dot_gap_em = value;
        self
    }
    pub fn build(self) -> ParagraphStyle {
        self.style
    }
}

/// `MeasureAdaptiveFirstLineIndent`（ADR 0021 amendment）：段首缩进随行长自适应——窄行
/// （measure < `short_below_em` 字）缩 `short_em` 字，宽行缩 `long_em` 字。窄栏（多栏杂志、手机
/// 正文）里 2 字缩进占比过重，CLREQ 也记多栏常缩一字，故默认窄行缩一字。
///
/// 阈值默认 14 字，与 `MeasureAdaptiveKinsoku` 的悬挂阈值同值但独立——两者回答不同问题（悬挂：整字
/// 下移是否过松；缩进：2 字是否过重），可分别调，且本策略在 `KinsokuMode::Fixed` 下仍生效（不依赖
/// 悬挂信号）。
///
/// 与行长无关地固定缩进，用 `ParagraphStyle.first_line_indent`（显式值，含 0 关闭）覆盖。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeasureAdaptiveFirstLineIndent {
    pub short_below_em: f32,
    pub short_em: f32,
    pub long_em: f32,
}

impl Default for MeasureAdaptiveFirstLineIndent {
    fn default() -> Self {
        Self {
            short_below_em: 14.0,
            short_em: 1.0,
            long_em: 2.0,
        }
    }
}

impl MeasureAdaptiveFirstLineIndent {
    pub fn new(short_below_em: f32, short_em: f32, long_em: f32) -> Self {
        Self {
            short_below_em,
            short_em,
            long_em,
        }
    }

    pub fn resolve_em(self, measure_em: f32) -> f32 {
        if measure_em < self.short_below_em {
            self.short_em
        } else {
            self.long_em
        }
    }
}

/// 将可用行长向下取整到字号的整数倍（N 字宽），使正文严格落在字格上（grid-first，ADR 0007）。响应式/
/// 实际容器宽度几乎不会恰好是字号的整数倍；引擎不能要求调用方在排版前就给出对齐字格的精确值，因此默认
/// 向下取整得到版心，余下不足一字的空白（slack ∈ [0, fontSize)）按 `body_alignment` 在容器内左右摆放
/// 整块正文。
///
/// 某些边缘情形——已知精确像素行长、非中文正文、或调用方自己做了字格对齐——可 `enabled = false` 绕过，
/// 直接用原始 maxWidth。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LineLengthGrid {
    pub enabled: bool,
    /// 正文块在容器内（量化后余量上）的横向对齐。CLREQ 双齐正文的唯一对齐自由度是末行
    /// （`ParagraphStyle.last_line_alignment`）；正文块在容器内的摆放默认跟随该末行对齐（None），
    /// 也可在此独立 override。
    pub body_alignment: Option<LastLineAlignment>,
}

impl Default for LineLengthGrid {
    fn default() -> Self {
        Self {
            enabled: true,
            body_alignment: None,
        }
    }
}

impl LineLengthGrid {
    pub fn new(enabled: bool, body_alignment: Option<LastLineAlignment>) -> Self {
        Self {
            enabled,
            body_alignment,
        }
    }
    pub fn with_enabled(enabled: bool) -> Self {
        Self {
            enabled,
            body_alignment: None,
        }
    }
    pub fn with_body_alignment(body_alignment: Option<LastLineAlignment>) -> Self {
        Self {
            enabled: true,
            body_alignment,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LastLineAlignment {
    #[default]
    Start,
    Center,
    End,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WritingMode {
    #[default]
    HorizontalTb,
    VerticalRl,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LayoutProfileId {
    pub value: String,
}

pub mod built_in_layout_profiles {
    use super::LayoutProfileId;

    pub fn clreq_horizontal() -> LayoutProfileId {
        LayoutProfileId {
            value: "clreq-horizontal".to_owned(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LayoutInput {
    pub content: TiqianTextContent,
    pub text_style: TextStyle,
    pub paragraph_style: ParagraphStyle,
    pub constraints: super::geometry::LayoutConstraints,
    pub profile_id: LayoutProfileId,
    pub decorations: Vec<DecorationSpan>,
    pub ruby_spans: Vec<RubySpan>,
    pub inline_boxes: Vec<InlineBoxSpan>,
    pub inline_objects: Vec<InlineObjectSpan>,
}

impl LayoutInput {
    pub fn builder(
        content: TiqianTextContent,
        constraints: super::geometry::LayoutConstraints,
    ) -> LayoutInputBuilder {
        LayoutInputBuilder {
            input: Self {
                content,
                text_style: TextStyle::default(),
                paragraph_style: ParagraphStyle::default(),
                constraints,
                profile_id: built_in_layout_profiles::clreq_horizontal(),
                decorations: Vec::new(),
                ruby_spans: Vec::new(),
                inline_boxes: Vec::new(),
                inline_objects: Vec::new(),
            },
        }
    }
}

pub struct LayoutInputBuilder {
    input: LayoutInput,
}

impl LayoutInputBuilder {
    pub fn text_style(mut self, value: TextStyle) -> Self {
        self.input.text_style = value;
        self
    }
    pub fn paragraph_style(mut self, value: ParagraphStyle) -> Self {
        self.input.paragraph_style = value;
        self
    }
    pub fn profile_id(mut self, value: LayoutProfileId) -> Self {
        self.input.profile_id = value;
        self
    }
    pub fn decorations(mut self, value: Vec<DecorationSpan>) -> Self {
        self.input.decorations = value;
        self
    }
    pub fn ruby_spans(mut self, value: Vec<RubySpan>) -> Self {
        self.input.ruby_spans = value;
        self
    }
    pub fn inline_boxes(mut self, value: Vec<InlineBoxSpan>) -> Self {
        self.input.inline_boxes = value;
        self
    }
    pub fn inline_objects(mut self, value: Vec<InlineObjectSpan>) -> Self {
        self.input.inline_objects = value;
        self
    }
    pub fn build(self) -> LayoutInput {
        self.input
    }
}
