// 对应 Kotlin 源文件：engine/src/commonMain/kotlin/org/tiqian/core/LayoutModel.kt

use super::geometry::{Rect, ScalarOffset, Size, TextRange};
use super::int_range::IntRange;
use super::text::Text;
use super::text_model::LayoutInput;

#[derive(Clone, Debug, PartialEq)]
pub struct Cluster {
    pub range: TextRange,
    pub text: Text,
    pub display_text: Text,
    pub font_key: String,
    pub advance: f32,
    /// 绘制此 cluster 时添加到 line baseline 的垂直偏移（px，+down），使 non-Roman mixed font/size
    /// 按其**字身框底部**对齐。Roman cluster 保留来自 base CJK metrics 的共享 alphabetic baseline，
    /// 因而 CJK + Latin text 位于同一 roman baseline。0 = 无偏移（base CJK 与 Roman run 为通常情况）。
    pub baseline_shift: f32,
    /// 此 cluster glyph origin 前的结构性行内 advance。它属于 `advance`，但 renderer 在绘制 glyph 前将
    /// 它添加到 occupied left edge。DOM inline-start padding 与生成内容使用此通道，使 measure 与 paint
    /// 共享一个 box model。
    pub leading_layout_advance: f32,
    /// 从 cluster pen 到 glyph origin 的、由 layout 拥有的水平偏移。
    pub glyph_inline_shift: f32,
}

impl Cluster {
    pub fn new(range: TextRange, text: Text, font_key: String, advance: f32) -> Self {
        Self {
            display_text: text.clone(),
            range,
            text,
            font_key,
            advance,
            baseline_shift: 0.0,
            leading_layout_advance: 0.0,
            glyph_inline_shift: 0.0,
        }
    }
    pub fn with_display_text(
        range: TextRange,
        text: Text,
        display_text: Text,
        font_key: String,
        advance: f32,
    ) -> Self {
        Self {
            range,
            text,
            display_text,
            font_key,
            advance,
            baseline_shift: 0.0,
            leading_layout_advance: 0.0,
            glyph_inline_shift: 0.0,
        }
    }
    pub fn with_baseline_shift(
        range: TextRange,
        text: Text,
        display_text: Text,
        font_key: String,
        advance: f32,
        baseline_shift: f32,
    ) -> Self {
        Self {
            range,
            text,
            display_text,
            font_key,
            advance,
            baseline_shift,
            leading_layout_advance: 0.0,
            glyph_inline_shift: 0.0,
        }
    }
    pub fn builder(range: TextRange, text: Text, font_key: String, advance: f32) -> ClusterBuilder {
        ClusterBuilder {
            cluster: Self::new(range, text, font_key, advance),
        }
    }
}

pub struct ClusterBuilder {
    cluster: Cluster,
}
impl ClusterBuilder {
    pub fn display_text(mut self, value: Text) -> Self {
        self.cluster.display_text = value;
        self
    }
    pub fn baseline_shift(mut self, value: f32) -> Self {
        self.cluster.baseline_shift = value;
        self
    }
    pub fn leading_layout_advance(mut self, value: f32) -> Self {
        self.cluster.leading_layout_advance = value;
        self
    }
    pub fn glyph_inline_shift(mut self, value: f32) -> Self {
        self.cluster.glyph_inline_shift = value;
        self
    }
    pub fn build(self) -> Cluster {
        self.cluster
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct GlyphRun {
    pub range: TextRange,
    pub font_key: String,
    pub glyphs: Vec<Glyph>,
    pub advance: f32,
    /// 当前端从 source text 而不是 glyph id 绘制此 run 时必须重放的 OpenType feature。这是 shaping output，
    /// 而不是 renderer-side guess：measurement 与 DOM paint 必须使用同一个列表。
    pub open_type_features: Vec<String>,
}
impl GlyphRun {
    pub fn new(range: TextRange, font_key: String, glyphs: Vec<Glyph>, advance: f32) -> Self {
        Self {
            range,
            font_key,
            glyphs,
            advance,
            open_type_features: Vec::new(),
        }
    }
    pub fn with_open_type_features(
        range: TextRange,
        font_key: String,
        glyphs: Vec<Glyph>,
        advance: f32,
        open_type_features: Vec<String>,
    ) -> Self {
        Self {
            range,
            font_key,
            glyphs,
            advance,
            open_type_features,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Glyph {
    pub id: u32,
    pub cluster_range: TextRange,
    /// font/shaper space 中 shaped glyph advance。由 layout 拥有的 trailing space（autospace、justification、
    /// ruby/bopomofo avoidance、punctuation glue）位于 `Cluster.advance`，不得折回 glyph position；精确的
    /// glyph replay 依赖保留 shaper 自身的 pair positioning。
    pub advance: f32,
    /// 相对 cluster pen origin 的 glyph origin，位于与 `advance` 相同的 shaper space。frontend 在
    /// `cluster_draw_x + x` 绘制；移向下一 cluster 使用 `Cluster.advance`，而不是 glyph advance 求和。
    pub x: f32,
    /// 相对 cluster baseline 的 glyph origin y。horizontal text 通常为 0；platform shaper 可提供非零 glyph placement。
    pub y: f32,
    /// 供能按 glyph id 绘制的 backend 使用的不透明、进程内 platform font key（例如 Android Canvas.drawGlyphs）。
    /// core/layout code 不解释它。
    pub render_font_key: Option<String>,
    pub bounds: Option<Rect>,
    /// 此 glyph 在 OpenType `halt`（alternate half-width metrics）下的 advance，由单独 feature-tagged shaping pass
    /// 测量。当 shaper 无法测量 feature（AWT、stub），或 font 不提供 alternate（`halt` advance == default advance）时为 None。
    pub halt_advance: Option<f32>,
    /// `halt` 施加的 x placement shift（例如 leading blank 被 trim 的 opening bracket 为 -0.5em）。
    /// punctuation geometry 直接使用它分配 leading/trailing compression budget。
    pub halt_placement_x: Option<f32>,
}
impl Glyph {
    pub fn builder(id: u32, cluster_range: TextRange, advance: f32) -> GlyphBuilder {
        GlyphBuilder {
            glyph: Self {
                id,
                cluster_range,
                advance,
                x: 0.0,
                y: 0.0,
                render_font_key: None,
                bounds: None,
                halt_advance: None,
                halt_placement_x: None,
            },
        }
    }
}
pub struct GlyphBuilder {
    glyph: Glyph,
}
impl GlyphBuilder {
    pub fn x(mut self, value: f32) -> Self {
        self.glyph.x = value;
        self
    }
    pub fn y(mut self, value: f32) -> Self {
        self.glyph.y = value;
        self
    }
    pub fn render_font_key(mut self, value: Option<String>) -> Self {
        self.glyph.render_font_key = value;
        self
    }
    pub fn bounds(mut self, value: Option<Rect>) -> Self {
        self.glyph.bounds = value;
        self
    }
    pub fn halt_advance(mut self, value: Option<f32>) -> Self {
        self.glyph.halt_advance = value;
        self
    }
    pub fn halt_placement_x(mut self, value: Option<f32>) -> Self {
        self.glyph.halt_placement_x = value;
        self
    }
    pub fn build(self) -> Glyph {
        self.glyph
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LineBox {
    pub range: TextRange,
    /// 此行拥有的 cluster index 的包含区间。renderer 必须使用它，而不是从 `range` 重推 membership：
    /// PushIn、CarryPrevious、hanging punctuation、hyphenation 等 repair pass 都是 cluster-level decision。
    pub cluster_range: IntRange,
    pub baseline: f32,
    pub top: f32,
    pub bottom: f32,
    pub natural_width: f32,
    pub adjusted_width: f32,
    pub visual_width: f32,
    /// 由 `LineEndHangingPunctuation`（CLREQ 行尾点号悬挂）选中的一个或多个 trailing mark 的总 advance。
    /// 它是 emitted line paint 的一部分而非 overflow text：当此值非零时，裁切 `TextOverflow.Clip` 的 frontend
    /// 必须允许最终 `visual_width`。ordinary profile path 悬挂一个 mark；只有具名 impossible-measure contextual run
    /// 可包含更多。未悬挂时为 0。
    pub hanging_punctuation_advance: f32,
    /// 此 line 沿 inline axis 的 start-edge inset（paragraph first line 的段首缩进；其余为 0）。renderer 必须从
    /// 此 offset 开始 pen；上方 width field 不包含它。
    pub indent: f32,
    /// 此行结束原因。只有 automatic wrap 有资格 paragraph justification；mandatory break 与 paragraph end
    /// 保留其 natural width（ADR 0037）。
    pub end_reason: LineEndReason,
    /// 悬挂于此行末尾的 hyphen 宽度（`LineEndHangingHyphen`，ADR 0029）：当行在 Western hyphenation point
    /// 处于词中结束时非零。hyphen 位于 `visual_width` 后（不计入 width field，模拟行尾点号悬挂）——renderer 在
    /// `indent + visual_width` 绘制 `-`。其余情况为 0。
    pub hyphen_advance: f32,
    /// 在 `indent + visual_width` 重放的 synthetic line-end hyphen shape-once glyph。
    pub hyphen_glyphs: Vec<Glyph>,
    pub debug: LineDebugInfo,
}
impl LineBox {
    pub fn builder(
        range: TextRange,
        cluster_range: IntRange,
        baseline: f32,
        top: f32,
        bottom: f32,
        natural_width: f32,
        adjusted_width: f32,
        visual_width: f32,
    ) -> LineBoxBuilder {
        LineBoxBuilder {
            line: Self {
                range,
                cluster_range,
                baseline,
                top,
                bottom,
                natural_width,
                adjusted_width,
                visual_width,
                hanging_punctuation_advance: 0.0,
                indent: 0.0,
                end_reason: LineEndReason::ParagraphEnd,
                hyphen_advance: 0.0,
                hyphen_glyphs: Vec::new(),
                debug: LineDebugInfo::default(),
            },
        }
    }
}
pub struct LineBoxBuilder {
    line: LineBox,
}
impl LineBoxBuilder {
    pub fn hanging_punctuation_advance(mut self, value: f32) -> Self {
        self.line.hanging_punctuation_advance = value;
        self
    }
    pub fn indent(mut self, value: f32) -> Self {
        self.line.indent = value;
        self
    }
    pub fn end_reason(mut self, value: LineEndReason) -> Self {
        self.line.end_reason = value;
        self
    }
    pub fn hyphen_advance(mut self, value: f32) -> Self {
        self.line.hyphen_advance = value;
        self
    }
    pub fn hyphen_glyphs(mut self, value: Vec<Glyph>) -> Self {
        self.line.hyphen_glyphs = value;
        self
    }
    pub fn debug(mut self, value: LineDebugInfo) -> Self {
        self.line.debug = value;
        self
    }
    pub fn build(self) -> LineBox {
        self.line
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineEndReason {
    AutoWrap,
    MandatoryBreak,
    ParagraphEnd,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LineDebugInfo {
    pub repair: Option<String>,
    pub notes: Vec<String>,
}
impl LineDebugInfo {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn with_repair(repair: Option<String>) -> Self {
        Self {
            repair,
            notes: Vec::new(),
        }
    }
    pub fn with_all(repair: Option<String>, notes: Vec<String>) -> Self {
        Self { repair, notes }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LayoutResult {
    pub input: LayoutInput,
    pub size: Size,
    pub clusters: Vec<Cluster>,
    pub glyph_runs: Vec<GlyphRun>,
    pub lines: Vec<LineBox>,
    pub debug: LayoutDebugInfo,
}
impl LayoutResult {
    pub fn new(
        input: LayoutInput,
        size: Size,
        clusters: Vec<Cluster>,
        glyph_runs: Vec<GlyphRun>,
        lines: Vec<LineBox>,
    ) -> Self {
        Self {
            input,
            size,
            clusters,
            glyph_runs,
            lines,
            debug: LayoutDebugInfo::default(),
        }
    }
    pub fn with_debug(
        input: LayoutInput,
        size: Size,
        clusters: Vec<Cluster>,
        glyph_runs: Vec<GlyphRun>,
        lines: Vec<LineBox>,
        debug: LayoutDebugInfo,
    ) -> Self {
        Self {
            input,
            size,
            clusters,
            glyph_runs,
            lines,
            debug,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LayoutDebugInfo {
    pub font_decisions: Vec<FontDecisionInfo>,
    pub shaping_decisions: Vec<ShapingDecisionInfo>,
    pub metric_decisions: Vec<MetricDecisionInfo>,
    pub punctuation_decisions: Vec<PunctuationDecisionInfo>,
    pub geometry_decisions: Vec<ClusterGeometryDecisionInfo>,
    pub spacing_decisions: Vec<SpacingDecisionInfo>,
    pub role_overrides: Vec<RoleOverrideInfo>,
    pub line_decisions: Vec<LineDecisionInfo>,
    pub justification_decisions: Vec<JustificationDecisionInfo>,
    pub auto_space_decisions: Vec<AutoSpaceDecisionInfo>,
    pub line_edge_trim_decisions: Vec<LineEdgeTrimDecisionInfo>,
    pub decoration_decisions: Vec<DecorationDecisionInfo>,
    pub decoration_segments: Vec<DecorationSegmentInfo>,
    pub ruby_decisions: Vec<RubyDecisionInfo>,
    pub bopomofo_decisions: Vec<BopomofoDecisionInfo>,
    pub mandatory_break_decisions: Vec<MandatoryBreakDecisionInfo>,
    pub max_lines_decision: Option<MaxLinesDecisionInfo>,
    pub line_spacing_decision: Option<LineSpacingDecisionInfo>,
    pub ruby_line_height_decision: Option<RubyLineHeightDecisionInfo>,
    pub inline_object_line_height_decision: Option<InlineObjectLineHeightDecisionInfo>,
    pub kinsoku_decision: Option<KinsokuDecisionInfo>,
    pub contextual_kinsoku_decisions: Vec<ContextualKinsokuDecisionInfo>,
    pub line_length_grid_decision: Option<LineLengthGridDecisionInfo>,
    pub first_line_indent_decision: Option<FirstLineIndentDecisionInfo>,
    pub inline_box_decisions: Vec<InlineBoxDecisionInfo>,
    pub inline_object_decisions: Vec<InlineObjectDecisionInfo>,
    pub inline_object_punctuation_attachment_decisions:
        Vec<InlineObjectPunctuationAttachmentDecisionInfo>,
    pub zero_width_break_decisions: Vec<ZeroWidthBreakDecisionInfo>,
    pub break_opportunity_decisions: Vec<BreakOpportunityDecisionInfo>,
    pub emergency_tracking_eligibility_decisions: Vec<EmergencyTrackingEligibilityDecisionInfo>,
}

impl LayoutDebugInfo {
    pub fn builder() -> LayoutDebugInfoBuilder {
        LayoutDebugInfoBuilder {
            debug: Self::default(),
        }
    }
}

pub struct LayoutDebugInfoBuilder {
    debug: LayoutDebugInfo,
}

impl LayoutDebugInfoBuilder {
    pub fn font_decisions(mut self, value: Vec<FontDecisionInfo>) -> Self {
        self.debug.font_decisions = value;
        self
    }
    pub fn shaping_decisions(mut self, value: Vec<ShapingDecisionInfo>) -> Self {
        self.debug.shaping_decisions = value;
        self
    }
    pub fn metric_decisions(mut self, value: Vec<MetricDecisionInfo>) -> Self {
        self.debug.metric_decisions = value;
        self
    }
    pub fn punctuation_decisions(mut self, value: Vec<PunctuationDecisionInfo>) -> Self {
        self.debug.punctuation_decisions = value;
        self
    }
    pub fn geometry_decisions(mut self, value: Vec<ClusterGeometryDecisionInfo>) -> Self {
        self.debug.geometry_decisions = value;
        self
    }
    pub fn spacing_decisions(mut self, value: Vec<SpacingDecisionInfo>) -> Self {
        self.debug.spacing_decisions = value;
        self
    }
    pub fn role_overrides(mut self, value: Vec<RoleOverrideInfo>) -> Self {
        self.debug.role_overrides = value;
        self
    }
    pub fn line_decisions(mut self, value: Vec<LineDecisionInfo>) -> Self {
        self.debug.line_decisions = value;
        self
    }
    pub fn justification_decisions(mut self, value: Vec<JustificationDecisionInfo>) -> Self {
        self.debug.justification_decisions = value;
        self
    }
    pub fn auto_space_decisions(mut self, value: Vec<AutoSpaceDecisionInfo>) -> Self {
        self.debug.auto_space_decisions = value;
        self
    }
    pub fn line_edge_trim_decisions(mut self, value: Vec<LineEdgeTrimDecisionInfo>) -> Self {
        self.debug.line_edge_trim_decisions = value;
        self
    }
    pub fn decoration_decisions(mut self, value: Vec<DecorationDecisionInfo>) -> Self {
        self.debug.decoration_decisions = value;
        self
    }
    pub fn decoration_segments(mut self, value: Vec<DecorationSegmentInfo>) -> Self {
        self.debug.decoration_segments = value;
        self
    }
    pub fn ruby_decisions(mut self, value: Vec<RubyDecisionInfo>) -> Self {
        self.debug.ruby_decisions = value;
        self
    }
    pub fn bopomofo_decisions(mut self, value: Vec<BopomofoDecisionInfo>) -> Self {
        self.debug.bopomofo_decisions = value;
        self
    }
    pub fn mandatory_break_decisions(mut self, value: Vec<MandatoryBreakDecisionInfo>) -> Self {
        self.debug.mandatory_break_decisions = value;
        self
    }
    pub fn max_lines_decision(mut self, value: Option<MaxLinesDecisionInfo>) -> Self {
        self.debug.max_lines_decision = value;
        self
    }
    pub fn line_spacing_decision(mut self, value: Option<LineSpacingDecisionInfo>) -> Self {
        self.debug.line_spacing_decision = value;
        self
    }
    pub fn ruby_line_height_decision(mut self, value: Option<RubyLineHeightDecisionInfo>) -> Self {
        self.debug.ruby_line_height_decision = value;
        self
    }
    pub fn inline_object_line_height_decision(
        mut self,
        value: Option<InlineObjectLineHeightDecisionInfo>,
    ) -> Self {
        self.debug.inline_object_line_height_decision = value;
        self
    }
    pub fn kinsoku_decision(mut self, value: Option<KinsokuDecisionInfo>) -> Self {
        self.debug.kinsoku_decision = value;
        self
    }
    pub fn contextual_kinsoku_decisions(
        mut self,
        value: Vec<ContextualKinsokuDecisionInfo>,
    ) -> Self {
        self.debug.contextual_kinsoku_decisions = value;
        self
    }
    pub fn line_length_grid_decision(mut self, value: Option<LineLengthGridDecisionInfo>) -> Self {
        self.debug.line_length_grid_decision = value;
        self
    }
    pub fn first_line_indent_decision(
        mut self,
        value: Option<FirstLineIndentDecisionInfo>,
    ) -> Self {
        self.debug.first_line_indent_decision = value;
        self
    }
    pub fn inline_box_decisions(mut self, value: Vec<InlineBoxDecisionInfo>) -> Self {
        self.debug.inline_box_decisions = value;
        self
    }
    pub fn inline_object_decisions(mut self, value: Vec<InlineObjectDecisionInfo>) -> Self {
        self.debug.inline_object_decisions = value;
        self
    }
    pub fn inline_object_punctuation_attachment_decisions(
        mut self,
        value: Vec<InlineObjectPunctuationAttachmentDecisionInfo>,
    ) -> Self {
        self.debug.inline_object_punctuation_attachment_decisions = value;
        self
    }
    pub fn zero_width_break_decisions(mut self, value: Vec<ZeroWidthBreakDecisionInfo>) -> Self {
        self.debug.zero_width_break_decisions = value;
        self
    }
    pub fn break_opportunity_decisions(mut self, value: Vec<BreakOpportunityDecisionInfo>) -> Self {
        self.debug.break_opportunity_decisions = value;
        self
    }
    pub fn emergency_tracking_eligibility_decisions(
        mut self,
        value: Vec<EmergencyTrackingEligibilityDecisionInfo>,
    ) -> Self {
        self.debug.emergency_tracking_eligibility_decisions = value;
        self
    }
    pub fn build(self) -> LayoutDebugInfo {
        self.debug
    }
}

/// 在原本不可分割的 shaping segment 内暴露干净 line-break offset 的具名 source-level policy。
/// offset 是绝对 scalar source offset；不插入 source character 或 synthetic glyph。
#[derive(Clone, Debug, PartialEq)]
pub struct BreakOpportunityDecisionInfo {
    pub range: TextRange,
    pub source_text: Text,
    pub break_offsets: Vec<ScalarOffset>,
    pub reason: String,
    /// 当此 decision 属于 progressive break span 时的有序 policy tier。
    pub tier: Option<String>,
}
impl BreakOpportunityDecisionInfo {
    pub fn new(
        range: TextRange,
        source_text: Text,
        break_offsets: Vec<ScalarOffset>,
        reason: String,
    ) -> Self {
        Self {
            range,
            source_text,
            break_offsets,
            reason,
            tier: None,
        }
    }
    pub fn with_tier(
        range: TextRange,
        source_text: Text,
        break_offsets: Vec<ScalarOffset>,
        reason: String,
        tier: Option<String>,
    ) -> Self {
        Self {
            range,
            source_text,
            break_offsets,
            reason,
            tier,
        }
    }
}

/// 显式允许以 grapheme-safe tracking 吸收原本无法填满的行 deficit 的 source range。普通 Western prose
/// 不在此列表中，因而默认绝不成为 tracking-eligible。
#[derive(Clone, Debug, PartialEq)]
pub struct EmergencyTrackingEligibilityDecisionInfo {
    pub range: TextRange,
    pub source_text: Text,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct InlineBoxDecisionInfo {
    pub range: TextRange,
    pub inline_start: f32,
    pub inline_end: f32,
    pub outer_spacing: String,
    pub first_cluster_index: i32,
    pub last_cluster_index: i32,
    pub reason: String,
}
impl InlineBoxDecisionInfo {
    pub fn new(
        range: TextRange,
        inline_start: f32,
        inline_end: f32,
        outer_spacing: String,
        first_cluster_index: i32,
        last_cluster_index: i32,
    ) -> Self {
        Self {
            range,
            inline_start,
            inline_end,
            outer_spacing,
            first_cluster_index,
            last_cluster_index,
            reason: "InlineBoxBoundaryAdvance".to_owned(),
        }
    }
    pub fn with_reason(
        range: TextRange,
        inline_start: f32,
        inline_end: f32,
        outer_spacing: String,
        first_cluster_index: i32,
        last_cluster_index: i32,
        reason: String,
    ) -> Self {
        Self {
            range,
            inline_start,
            inline_end,
            outer_spacing,
            first_cluster_index,
            last_cluster_index,
            reason,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct InlineObjectDecisionInfo {
    pub range: TextRange,
    pub advance: f32,
    pub ascent: f32,
    pub descent: f32,
    pub cluster_index: i32,
    pub line_index: i32,
    pub leading_uniform_stretch: bool,
    pub leading_preferred_stretch_kind: Option<String>,
    pub leading_preferred_stretch_natural_width: f32,
    pub leading_preferred_stretch_target_width: f32,
    pub leading_preferred_stretch_capacity: f32,
    pub leading_prevents_line_break: bool,
    pub leading_shrink_capacity: f32,
    pub leading_line_end_discardable_advance: f32,
    pub trailing_uniform_stretch: bool,
    pub trailing_preferred_stretch_kind: Option<String>,
    pub trailing_preferred_stretch_natural_width: f32,
    pub trailing_preferred_stretch_target_width: f32,
    pub trailing_preferred_stretch_capacity: f32,
    pub trailing_prevents_line_break: bool,
    pub trailing_shrink_capacity: f32,
    pub trailing_line_end_discardable_advance: f32,
    pub reason: String,
}
impl InlineObjectDecisionInfo {
    pub fn builder(
        range: TextRange,
        advance: f32,
        ascent: f32,
        descent: f32,
        cluster_index: i32,
        line_index: i32,
    ) -> InlineObjectDecisionInfoBuilder {
        InlineObjectDecisionInfoBuilder {
            decision: Self {
                range,
                advance,
                ascent,
                descent,
                cluster_index,
                line_index,
                leading_uniform_stretch: false,
                leading_preferred_stretch_kind: None,
                leading_preferred_stretch_natural_width: 0.0,
                leading_preferred_stretch_target_width: 0.0,
                leading_preferred_stretch_capacity: 0.0,
                leading_prevents_line_break: false,
                leading_shrink_capacity: 0.0,
                leading_line_end_discardable_advance: 0.0,
                trailing_uniform_stretch: false,
                trailing_preferred_stretch_kind: None,
                trailing_preferred_stretch_natural_width: 0.0,
                trailing_preferred_stretch_target_width: 0.0,
                trailing_preferred_stretch_capacity: 0.0,
                trailing_prevents_line_break: false,
                trailing_shrink_capacity: 0.0,
                trailing_line_end_discardable_advance: 0.0,
                reason: "MeasurableOpaqueInlineObject".to_owned(),
            },
        }
    }
}
pub struct InlineObjectDecisionInfoBuilder {
    decision: InlineObjectDecisionInfo,
}
impl InlineObjectDecisionInfoBuilder {
    pub fn leading_uniform_stretch(mut self, value: bool) -> Self {
        self.decision.leading_uniform_stretch = value;
        self
    }
    pub fn leading_preferred_stretch_kind(mut self, value: Option<String>) -> Self {
        self.decision.leading_preferred_stretch_kind = value;
        self
    }
    pub fn leading_preferred_stretch_natural_width(mut self, value: f32) -> Self {
        self.decision.leading_preferred_stretch_natural_width = value;
        self
    }
    pub fn leading_preferred_stretch_target_width(mut self, value: f32) -> Self {
        self.decision.leading_preferred_stretch_target_width = value;
        self
    }
    pub fn leading_preferred_stretch_capacity(mut self, value: f32) -> Self {
        self.decision.leading_preferred_stretch_capacity = value;
        self
    }
    pub fn leading_prevents_line_break(mut self, value: bool) -> Self {
        self.decision.leading_prevents_line_break = value;
        self
    }
    pub fn leading_shrink_capacity(mut self, value: f32) -> Self {
        self.decision.leading_shrink_capacity = value;
        self
    }
    pub fn leading_line_end_discardable_advance(mut self, value: f32) -> Self {
        self.decision.leading_line_end_discardable_advance = value;
        self
    }
    pub fn trailing_uniform_stretch(mut self, value: bool) -> Self {
        self.decision.trailing_uniform_stretch = value;
        self
    }
    pub fn trailing_preferred_stretch_kind(mut self, value: Option<String>) -> Self {
        self.decision.trailing_preferred_stretch_kind = value;
        self
    }
    pub fn trailing_preferred_stretch_natural_width(mut self, value: f32) -> Self {
        self.decision.trailing_preferred_stretch_natural_width = value;
        self
    }
    pub fn trailing_preferred_stretch_target_width(mut self, value: f32) -> Self {
        self.decision.trailing_preferred_stretch_target_width = value;
        self
    }
    pub fn trailing_preferred_stretch_capacity(mut self, value: f32) -> Self {
        self.decision.trailing_preferred_stretch_capacity = value;
        self
    }
    pub fn trailing_prevents_line_break(mut self, value: bool) -> Self {
        self.decision.trailing_prevents_line_break = value;
        self
    }
    pub fn trailing_shrink_capacity(mut self, value: f32) -> Self {
        self.decision.trailing_shrink_capacity = value;
        self
    }
    pub fn trailing_line_end_discardable_advance(mut self, value: f32) -> Self {
        self.decision.trailing_line_end_discardable_advance = value;
        self
    }
    pub fn reason(mut self, value: String) -> Self {
        self.decision.reason = value;
        self
    }
    pub fn build(self) -> InlineObjectDecisionInfo {
        self.decision
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ZeroWidthBreakDecisionInfo {
    pub range: TextRange,
    pub source_text: Text,
    pub cluster_index: i32,
    pub reason: String,
}
impl ZeroWidthBreakDecisionInfo {
    pub fn new(range: TextRange, source_text: Text, cluster_index: i32) -> Self {
        Self {
            range,
            source_text,
            cluster_index,
            reason: "ZeroWidthSpaceSoftBreakNoShape".to_owned(),
        }
    }
    pub fn with_reason(
        range: TextRange,
        source_text: Text,
        cluster_index: i32,
        reason: String,
    ) -> Self {
        Self {
            range,
            source_text,
            cluster_index,
            reason,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MandatoryBreakDecisionInfo {
    pub range: TextRange,
    pub source_text: Text,
    pub break_after_cluster_index: i32,
    pub reason: String,
}

/// `MaxLinesLineTruncation`：layout 在完整文本上运行（`laid_out_lines` 个 committed line，仍可见于
/// `line_decisions`），但结果按 `LayoutConstraints.maxLines` 仅输出前 `visible_lines` 个 line box。
#[derive(Clone, Debug, PartialEq)]
pub struct MaxLinesDecisionInfo {
    pub laid_out_lines: i32,
    pub visible_lines: i32,
    pub reason: String,
}
impl MaxLinesDecisionInfo {
    pub fn new(laid_out_lines: i32, visible_lines: i32) -> Self {
        Self {
            laid_out_lines,
            visible_lines,
            reason: "MaxLinesLineTruncation".to_owned(),
        }
    }
    pub fn with_reason(laid_out_lines: i32, visible_lines: i32, reason: String) -> Self {
        Self {
            laid_out_lines,
            visible_lines,
            reason,
        }
    }
}

/// 段首缩进的解析：`source` = "MeasureAdaptiveFirstLineIndent"（按 `measure_em` 字数自适应，
/// < `threshold_em` 字缩窄）或 "Explicit"（`first_line_indent` 显式覆盖）；`resolved_em` 是最终缩进字数。
#[derive(Clone, Debug, PartialEq)]
pub struct FirstLineIndentDecisionInfo {
    pub source: String,
    pub measure_em: f32,
    pub threshold_em: f32,
    pub resolved_em: f32,
}

/// `LineLengthGridQuantization`（grid-first，ADR 0007/0028）：container `container_width` 如何向下取整为
/// `font_size` 的 `cells`（字）整数倍以得到 layout measure，以及 leftover `slack` 如何按 `body_alignment`
/// 将整个正文置于 container 内（`body_offset`）。`enabled = false` 记录 bypass（measure == container，offset 0）。
#[derive(Clone, Debug, PartialEq)]
pub struct LineLengthGridDecisionInfo {
    pub enabled: bool,
    pub container_width: f32,
    pub font_size: f32,
    pub cells: i32,
    pub measure: f32,
    pub slack: f32,
    pub body_alignment: String,
    pub body_offset: f32,
    pub reason: String,
}

/// 段落解析出的 kinsoku level + hanging style 及其原因（`MeasureAdaptiveKinsoku` 按 measure 的字数决策；
/// `Fixed` 将其固定）。
#[derive(Clone, Debug, PartialEq)]
pub struct KinsokuDecisionInfo {
    pub measure_em: f32,
    pub level: String,
    pub hanging: String,
    pub reason: String,
}

/// 不改变 font role 或 punctuation geometry 的 source-context rule：它将 cluster 加入已解析的 kinsoku set。
///
/// 这与记录 paragraph profile level 的 `KinsokuDecisionInfo` 不同。例如 ASCII comma 可保持 Latin face 和
/// natural proportional advance，但 Chinese context 仍令其成为 line-start nonstarter。
#[derive(Clone, Debug, PartialEq)]
pub struct ContextualKinsokuDecisionInfo {
    pub range: TextRange,
    pub source_text: Text,
    pub cluster_index: i32,
    pub forbidden_position: String,
    pub reason: String,
    /// 只有实际为此 cluster 选择该 fallback 时才存在的具名 last-resort geometry。
    pub impossible_measure_fallback: Option<String>,
}
impl ContextualKinsokuDecisionInfo {
    pub fn new(
        range: TextRange,
        source_text: Text,
        cluster_index: i32,
        forbidden_position: String,
        reason: String,
    ) -> Self {
        Self {
            range,
            source_text,
            cluster_index,
            forbidden_position,
            reason,
            impossible_measure_fallback: None,
        }
    }
    pub fn with_impossible_measure_fallback(
        range: TextRange,
        source_text: Text,
        cluster_index: i32,
        forbidden_position: String,
        reason: String,
        impossible_measure_fallback: Option<String>,
    ) -> Self {
        Self {
            range,
            source_text,
            cluster_index,
            forbidden_position,
            reason,
            impossible_measure_fallback,
        }
    }
}

/// 跨 author-written separator space 附着到 inline object 的 point mark。
///
/// source space 保留给 copy 与 accessibility，而其 layout advance 被折叠，使 mark 视觉上附着。`protected_range`
/// 内每个 boundary 同时对 line breaking 与 justification 关闭。
#[derive(Clone, Debug, PartialEq)]
pub struct InlineObjectPunctuationAttachmentDecisionInfo {
    pub object_range: TextRange,
    pub separator_range: TextRange,
    pub punctuation_range: TextRange,
    pub punctuation_text: Text,
    pub protected_range: TextRange,
    pub collapsed_advance: f32,
    pub reason: String,
}
impl InlineObjectPunctuationAttachmentDecisionInfo {
    pub fn new(
        object_range: TextRange,
        separator_range: TextRange,
        punctuation_range: TextRange,
        punctuation_text: Text,
        protected_range: TextRange,
        collapsed_advance: f32,
    ) -> Self {
        Self {
            object_range,
            separator_range,
            punctuation_range,
            punctuation_text,
            protected_range,
            collapsed_advance,
            reason: "InlineObjectPunctuationSeparatorSpaceCollapse".to_owned(),
        }
    }
    pub fn with_reason(
        object_range: TextRange,
        separator_range: TextRange,
        punctuation_range: TextRange,
        punctuation_text: Text,
        protected_range: TextRange,
        collapsed_advance: f32,
        reason: String,
    ) -> Self {
        Self {
            object_range,
            separator_range,
            punctuation_range,
            punctuation_text,
            protected_range,
            collapsed_advance,
            reason,
        }
    }
}

/// `InterlinearMarkLineSpacingFloor`（CLREQ 5.6.1.1）：存在行间标记（着重号、示亡号等）时，行距不得低于
/// font size 的 1/2，以免紧缩 line height 使 mark 与下一行碰撞。paragraph 带此类 mark 时均记录；
/// `floor_applied` 说明 floor 是否实际增大（auto）或钳制（explicit）line height。
/// （CLREQ 双面装 5/8 floor 是 print-only——screen 没有透印类比——将随 print backend 返回。）
#[derive(Clone, Debug, PartialEq)]
pub struct LineSpacingDecisionInfo {
    pub natural_height: f32,
    pub requested_line_height: Option<f32>,
    pub resolved_height: f32,
    pub spacing_floor: f32,
    pub floor_applied: bool,
    pub reason: String,
}

/// `ConditionalRubyLineHeight`：pinyin ruby 先消耗 `available_interline_space`。只有超出的 `line_extras`
/// 按 `mode` 改变 line box；既有 line height 容纳时 `expanded_line_indices` 为空。
#[derive(Clone, Debug, PartialEq)]
pub struct RubyLineHeightDecisionInfo {
    pub mode: String,
    pub base_line_height: f32,
    pub base_face_height: f32,
    pub ruby_extent: f32,
    pub available_interline_space: f32,
    pub max_extra: f32,
    pub line_extras: Vec<f32>,
    pub expanded_line_indices: Vec<i32>,
    pub reason: String,
}

/// `InlineObjectInterlineCollision`：baseline-aligned inline object 可先使用 paragraph 既有 inter-line space，
/// 再改变 baseline grid，同时当 object ink 超出 base text face 时仍保留 `minimum_clearance`。`line_extras`
/// 仅记录 object 在每行之前强制增加的额外空间（index 0 为 paragraph-top containment）；
/// `boundary_shifts_after` 记录相邻 line box 间已存在空间的再分配，不改变 baseline distance。
#[derive(Clone, Debug, PartialEq)]
pub struct InlineObjectLineHeightDecisionInfo {
    pub base_line_height: f32,
    pub base_face_ascent: f32,
    pub base_face_descent: f32,
    pub available_interline_space: f32,
    pub minimum_clearance: f32,
    pub line_ascents: Vec<f32>,
    pub line_descents: Vec<f32>,
    pub line_extras: Vec<f32>,
    pub boundary_shifts_after: Vec<f32>,
    pub trailing_extra: f32,
    pub expanded_line_indices: Vec<i32>,
    pub reason: String,
}

/// 行间注 geometry（ruby，ADR 0032）：annotation `text` 位于 line `line_index` 上 `base_range` 的上方。
/// `center_x` 是 base range 的 horizontal centre（注文以其居中，CLREQ“横排注音注文整体水平向基字居中”）；
/// `baseline_y` 是 annotated base face 上方的 ruby text baseline。ruby 先占用既有 inter-line area，
/// 只有 deficit 按 `ParagraphStyle.ruby_line_height_mode` 扩张 line box。`font_size` 是 ruby size（≤ base），
/// `width` 是其自身 font 中测量的注文宽度。`overhang` > 0 表示注文宽于 base content，在应用 minimum-gap
/// avoidance 前向两侧悬出。
#[derive(Clone, Debug, PartialEq)]
pub struct RubyDecisionInfo {
    pub base_range: TextRange,
    pub text: Text,
    pub line_index: i32,
    pub center_x: f32,
    pub baseline_y: f32,
    pub font_size: f32,
    /// `baseline_y` 上方声明的 Latin font ascent，供 overflow 与 diagnostics 使用。
    pub ascent: f32,
    /// `baseline_y` 下方声明的 Latin font descent，供 overflow 与 diagnostics 使用。
    pub descent: f32,
    pub width: f32,
    pub overhang: f32,
    /// 注文专用字体（family 名优先列表）；空 = renderer 默认。
    pub font_families: Vec<String>,
    /// 注文字重：小字号下注文比基文重 100（OpenType weight），以保清晰。
    pub font_weight: i32,
    /// 本次注文 shaping 实际使用的 BCP-47 language。
    pub locale: String,
    /// 在 `center_x - width/2, baseline_y` 绘制的 shape-once annotation glyph。
    pub glyphs: Vec<Glyph>,
}
impl RubyDecisionInfo {
    pub fn builder(
        base_range: TextRange,
        text: Text,
        line_index: i32,
        center_x: f32,
        baseline_y: f32,
        font_size: f32,
        overhang: f32,
    ) -> RubyDecisionInfoBuilder {
        RubyDecisionInfoBuilder {
            decision: Self {
                base_range,
                text,
                line_index,
                center_x,
                baseline_y,
                font_size,
                ascent: 0.0,
                descent: 0.0,
                width: 0.0,
                overhang,
                font_families: Vec::new(),
                font_weight: 400,
                locale: "zh-Hans".to_owned(),
                glyphs: Vec::new(),
            },
        }
    }
}
pub struct RubyDecisionInfoBuilder {
    decision: RubyDecisionInfo,
}
impl RubyDecisionInfoBuilder {
    pub fn ascent(mut self, value: f32) -> Self {
        self.decision.ascent = value;
        self
    }
    pub fn descent(mut self, value: f32) -> Self {
        self.decision.descent = value;
        self
    }
    pub fn width(mut self, value: f32) -> Self {
        self.decision.width = value;
        self
    }
    pub fn font_families(mut self, value: Vec<String>) -> Self {
        self.decision.font_families = value;
        self
    }
    pub fn font_weight(mut self, value: i32) -> Self {
        self.decision.font_weight = value;
        self
    }
    pub fn locale(mut self, value: String) -> Self {
        self.decision.locale = value;
        self
    }
    pub fn glyphs(mut self, value: Vec<Glyph>) -> Self {
        self.decision.glyphs = value;
        self
    }
    pub fn build(self) -> RubyDecisionInfo {
        self.decision
    }
}

/// 注音 geometry（ADR 0033）：line `line_index` 上 `base_range` 右侧区域内的 ㄅㄆㄇ symbol + 调号。
/// 每个 `placement` 是一个 glyph + box（absolute px）+ role。symbol 使用 9×9 slot size；普通 tone mark
/// 使用 5×5 slot size，不按 glyph ink 重新缩放。
#[derive(Clone, Debug, PartialEq)]
pub struct BopomofoDecisionInfo {
    pub base_range: TextRange,
    pub text: Text,
    pub line_index: i32,
    pub placements: Vec<BopomofoGlyphPlacement>,
    /// 注文 font（必须携带 ㄅㄆㄇ glyph）；空 = renderer 的 CJK default。
    pub font_families: Vec<String>,
    /// 注文字重：比基文重 300（OpenType weight），小字号下保清晰。
    pub font_weight: i32,
    /// 本次注音 shaping 实际使用的 BCP-47 language。
    pub locale: String,
}
impl BopomofoDecisionInfo {
    pub fn new(
        base_range: TextRange,
        text: Text,
        line_index: i32,
        placements: Vec<BopomofoGlyphPlacement>,
    ) -> Self {
        Self {
            base_range,
            text,
            line_index,
            placements,
            font_families: Vec::new(),
            font_weight: 400,
            locale: "zh-Hans".to_owned(),
        }
    }
    pub fn builder(
        base_range: TextRange,
        text: Text,
        line_index: i32,
        placements: Vec<BopomofoGlyphPlacement>,
    ) -> BopomofoDecisionInfoBuilder {
        BopomofoDecisionInfoBuilder {
            decision: Self::new(base_range, text, line_index, placements),
        }
    }
}
pub struct BopomofoDecisionInfoBuilder {
    decision: BopomofoDecisionInfo,
}
impl BopomofoDecisionInfoBuilder {
    pub fn font_families(mut self, value: Vec<String>) -> Self {
        self.decision.font_families = value;
        self
    }
    pub fn font_weight(mut self, value: i32) -> Self {
        self.decision.font_weight = value;
        self
    }
    pub fn locale(mut self, value: String) -> Self {
        self.decision.locale = value;
        self
    }
    pub fn build(self) -> BopomofoDecisionInfo {
        self.decision
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct BopomofoGlyphPlacement {
    pub text: Text,
    pub left: f32,
    pub top: f32,
    pub width: f32,
    pub height: f32,
    pub role: BopomofoGlyphRole,
    /// 在 declared annotation size 且 `vert` substitution 后的最终 shape-once glyph。
    pub glyphs: Vec<Glyph>,
    pub draw_x: f32,
    pub baseline_y: f32,
    pub font_size: f32,
}
impl BopomofoGlyphPlacement {
    pub fn new(
        text: Text,
        left: f32,
        top: f32,
        width: f32,
        height: f32,
        role: BopomofoGlyphRole,
    ) -> Self {
        Self {
            text,
            left,
            top,
            width,
            height,
            role,
            glyphs: Vec::new(),
            draw_x: left,
            baseline_y: top + height,
            font_size: height,
        }
    }
    pub fn builder(
        text: Text,
        left: f32,
        top: f32,
        width: f32,
        height: f32,
        role: BopomofoGlyphRole,
    ) -> BopomofoGlyphPlacementBuilder {
        BopomofoGlyphPlacementBuilder {
            placement: Self::new(text, left, top, width, height, role),
        }
    }
}
pub struct BopomofoGlyphPlacementBuilder {
    placement: BopomofoGlyphPlacement,
}
impl BopomofoGlyphPlacementBuilder {
    pub fn glyphs(mut self, value: Vec<Glyph>) -> Self {
        self.placement.glyphs = value;
        self
    }
    pub fn draw_x(mut self, value: f32) -> Self {
        self.placement.draw_x = value;
        self
    }
    pub fn baseline_y(mut self, value: f32) -> Self {
        self.placement.baseline_y = value;
        self
    }
    pub fn font_size(mut self, value: f32) -> Self {
        self.placement.font_size = value;
        self
    }
    pub fn build(self) -> BopomofoGlyphPlacement {
        self.placement
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BopomofoGlyphRole {
    /// ㄅㄆㄇ——以 box font size（字身框）填充 9×9 box。
    Symbol,
    /// 平上去/入声调号——与注音字号相同；5×5 slot 定位 ink 但绝不改变 size。
    Tone,
    /// 轻声 ˙——其 vert-alt 已验证为 FULL-WIDTH。以 box-WIDTH font size（不缩放）绘制，按 vert advance
    /// h-centre，垂直 ink-position 使 dot 落入 box（neutral row）。该 box 是 dot 的 target rect。
    Neutral,
}

/// box-style decoration（示亡号，ADR 0018）的逐行矩形 segment。vertical bound 沿真实 baseline 紧贴 CJK
/// character face（字面）——即 line metric 使用的相同 font-declared box（ADR 0002 amendment）。
/// `open_start`/`open_end` 标记从/向另一行延续的 segment，renderer 不绘制该 frame edge。
#[derive(Clone, Debug, PartialEq)]
pub struct DecorationSegmentInfo {
    pub source_range: TextRange,
    pub kind: String,
    pub line_index: i32,
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub open_start: bool,
    pub open_end: bool,
    pub reason: String,
}

/// 逐 cluster decoration resolution（ADR 0018）。对 `Emphasis`，`applied` cluster 获得一个 dot，其 INK
/// CENTRE 必须落在 layout canvas coordinate 的（`anchor_x`，`anchor_y`）；skipped cluster 记录原因
/// （CLREQ：punctuation 从不带 dot；western text 改用 italic）。
#[derive(Clone, Debug, PartialEq)]
pub struct DecorationDecisionInfo {
    pub cluster_range: TextRange,
    pub source_text: Text,
    pub kind: String,
    pub applied: bool,
    pub reason: String,
    pub anchor_x: f32,
    pub anchor_y: f32,
    /// 着重号 dot diameter（px）。renderer 以 engine gap geometry 所用的精确尺寸绘制 filled circle，
    /// 不得再施加第二个 scale factor。non-dot decoration 为 0。
    pub dot_diameter: f32,
}
impl DecorationDecisionInfo {
    pub fn new(
        cluster_range: TextRange,
        source_text: Text,
        kind: String,
        applied: bool,
        reason: String,
    ) -> Self {
        Self {
            cluster_range,
            source_text,
            kind,
            applied,
            reason,
            anchor_x: 0.0,
            anchor_y: 0.0,
            dot_diameter: 0.0,
        }
    }
    pub fn builder(
        cluster_range: TextRange,
        source_text: Text,
        kind: String,
        applied: bool,
        reason: String,
    ) -> DecorationDecisionInfoBuilder {
        DecorationDecisionInfoBuilder {
            decision: Self::new(cluster_range, source_text, kind, applied, reason),
        }
    }
}
pub struct DecorationDecisionInfoBuilder {
    decision: DecorationDecisionInfo,
}
impl DecorationDecisionInfoBuilder {
    pub fn anchor_x(mut self, value: f32) -> Self {
        self.decision.anchor_x = value;
        self
    }
    pub fn anchor_y(mut self, value: f32) -> Self {
        self.decision.anchor_y = value;
        self
    }
    pub fn dot_diameter(mut self, value: f32) -> Self {
        self.decision.dot_diameter = value;
        self
    }
    pub fn build(self) -> DecorationDecisionInfo {
        self.decision
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LineEdgeTrimDecisionInfo {
    pub line_range: TextRange,
    pub cluster_range: TextRange,
    pub side: String,
    pub trim_amount: f32,
    pub consumed_before: f32,
    pub natural_glue: f32,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AutoSpaceDecisionInfo {
    pub cluster_range: TextRange,
    pub side: String,
    pub boundary_role: String,
    pub mode: String,
    pub characters_affected: i32,
    pub reduction_per_char: f32,
    pub total_reduction: f32,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FontDecisionInfo {
    pub range: TextRange,
    pub source_text: Text,
    pub display_text: Text,
    pub role: String,
    pub font_key: String,
    pub reason: String,
    pub substitution_reason: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ShapingDecisionInfo {
    pub range: TextRange,
    pub source_text: Text,
    pub display_text: Text,
    pub font_key: String,
    pub glyph_count: i32,
    pub advance: f32,
    pub source: String,
    pub reason: String,
    /// shaper 未返回 ink bound 的 `glyph_count` glyph 数。非零值将下游的 `MissingInkBoundsFallback`
    /// heuristic 送入——punctuation geometry 退化为仅 shaped-advance。
    pub glyphs_without_ink_bounds: i32,
    /// 解析为 font `.notdef` glyph 的 `glyph_count` glyph 数。CLREQ-substituted cluster 上的非零值触发
    /// `SubstitutionRollbackOnMissingGlyph`：engine 改以 source text shape 而非显示 tofu（例如 `⸺` 在
    /// PingFang SC / Hiragino / Heiti 中不存在）。
    pub missing_glyphs: i32,
    /// shaper 可观察该 identity 时选择的精确 platform face。
    pub resolved_face: Option<String>,
    /// 显式时提供给 shaping engine 的 script 与 language。
    pub script: Option<String>,
    pub language: Option<String>,
    /// 具名 glyph-selection strategy，例如 `PairedEmDash`。
    pub strategy: Option<String>,
    /// OpenType feature decision 的 evidence，而不只是 requested tag。
    pub feature_evidence: Option<String>,
    /// frontend 必须保持 source content native 的具名原因。
    pub capability_issue: Option<String>,
}
impl ShapingDecisionInfo {
    pub fn builder(
        range: TextRange,
        source_text: Text,
        display_text: Text,
        font_key: String,
        glyph_count: i32,
        advance: f32,
        source: String,
        reason: String,
    ) -> ShapingDecisionInfoBuilder {
        ShapingDecisionInfoBuilder {
            decision: Self {
                range,
                source_text,
                display_text,
                font_key,
                glyph_count,
                advance,
                source,
                reason,
                glyphs_without_ink_bounds: 0,
                missing_glyphs: 0,
                resolved_face: None,
                script: None,
                language: None,
                strategy: None,
                feature_evidence: None,
                capability_issue: None,
            },
        }
    }
}
pub struct ShapingDecisionInfoBuilder {
    decision: ShapingDecisionInfo,
}
impl ShapingDecisionInfoBuilder {
    pub fn glyphs_without_ink_bounds(mut self, value: i32) -> Self {
        self.decision.glyphs_without_ink_bounds = value;
        self
    }
    pub fn missing_glyphs(mut self, value: i32) -> Self {
        self.decision.missing_glyphs = value;
        self
    }
    pub fn resolved_face(mut self, value: Option<String>) -> Self {
        self.decision.resolved_face = value;
        self
    }
    pub fn script(mut self, value: Option<String>) -> Self {
        self.decision.script = value;
        self
    }
    pub fn language(mut self, value: Option<String>) -> Self {
        self.decision.language = value;
        self
    }
    pub fn strategy(mut self, value: Option<String>) -> Self {
        self.decision.strategy = value;
        self
    }
    pub fn feature_evidence(mut self, value: Option<String>) -> Self {
        self.decision.feature_evidence = value;
        self
    }
    pub fn capability_issue(mut self, value: Option<String>) -> Self {
        self.decision.capability_issue = value;
        self
    }
    pub fn build(self) -> ShapingDecisionInfo {
        self.decision
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MetricDecisionInfo {
    pub range: TextRange,
    pub source_text: Text,
    pub role: String,
    pub font_key: String,
    pub raw_ascent: f32,
    pub raw_descent: f32,
    pub raw_leading: f32,
    pub raw_source: String,
    pub layout_ascent: f32,
    pub layout_descent: f32,
    pub baseline_class: String,
    pub metric_box: String,
    pub layout_source: String,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PunctuationDecisionInfo {
    pub range: TextRange,
    pub ch: char,
    pub punctuation_class: String,
    pub advance: f32,
    pub body_width: f32,
    pub leading_glue_natural: f32,
    pub trailing_glue_natural: f32,
    pub anchor: String,
    pub ink_bounds: Option<Rect>,
    pub geometry_source: String,
    pub policy_body_floor: f32,
    pub ink_width: Option<f32>,
    pub ink_center: Option<f32>,
    /// 所有可移除 sidebearing 被消耗后允许的最小 body left。
    pub ink_containment_body_floor: Option<f32>,
    /// `InkContainmentBodyFloor` 降低 mark 的 compressible glue 时为 true。
    pub ink_containment_applied: bool,
    /// shaping 运行但 ink bound 不能归于该 punctuation character 时的 `MissingInkBoundsFallback` reason code；
    /// bound 存在，或完全不存在 shaping information（pure policy path）时为 None。missing bound 禁用
    /// `InkContainmentBodyFloor`；profile/halt geometry 仍是具名 fallback。
    pub ink_bounds_fallback: Option<String>,
    /// `FontHalt*` geometry 所用的 font-measured `halt` advance；None = ink/policy path。
    pub halt_advance: Option<f32>,
    /// default ink 阻止忠实重放 measured halt trim 时的 warning。
    pub halt_validation: Option<String>,
    /// shaped glyph 窄于 CLREQ box 时添加的 layout advance。
    pub advance_expansion: f32,
    /// synthesized full-width cell 内 underwidth font-owned glyph box 的 placement。
    pub glyph_inline_shift: f32,
    /// 具名 placement heuristic；不需要 glyph shift 时为 None。
    pub glyph_placement_reason: Option<String>,
    pub leading_glue_initially_consumed: f32,
    pub trailing_glue_initially_consumed: f32,
}
impl PunctuationDecisionInfo {
    pub fn builder(
        range: TextRange,
        ch: char,
        punctuation_class: String,
        advance: f32,
        body_width: f32,
        leading_glue_natural: f32,
        trailing_glue_natural: f32,
        anchor: String,
    ) -> PunctuationDecisionInfoBuilder {
        PunctuationDecisionInfoBuilder {
            decision: Self {
                range,
                ch,
                punctuation_class,
                advance,
                body_width,
                leading_glue_natural,
                trailing_glue_natural,
                anchor,
                ink_bounds: None,
                geometry_source: "PolicyDerived".to_owned(),
                policy_body_floor: body_width,
                ink_width: None,
                ink_center: None,
                ink_containment_body_floor: None,
                ink_containment_applied: false,
                ink_bounds_fallback: None,
                halt_advance: None,
                halt_validation: None,
                advance_expansion: 0.0,
                glyph_inline_shift: 0.0,
                glyph_placement_reason: None,
                leading_glue_initially_consumed: 0.0,
                trailing_glue_initially_consumed: 0.0,
            },
        }
    }
}
pub struct PunctuationDecisionInfoBuilder {
    decision: PunctuationDecisionInfo,
}
impl PunctuationDecisionInfoBuilder {
    pub fn ink_bounds(mut self, value: Option<Rect>) -> Self {
        self.decision.ink_bounds = value;
        self
    }
    pub fn geometry_source(mut self, value: String) -> Self {
        self.decision.geometry_source = value;
        self
    }
    pub fn policy_body_floor(mut self, value: f32) -> Self {
        self.decision.policy_body_floor = value;
        self
    }
    pub fn ink_width(mut self, value: Option<f32>) -> Self {
        self.decision.ink_width = value;
        self
    }
    pub fn ink_center(mut self, value: Option<f32>) -> Self {
        self.decision.ink_center = value;
        self
    }
    pub fn ink_containment_body_floor(mut self, value: Option<f32>) -> Self {
        self.decision.ink_containment_body_floor = value;
        self
    }
    pub fn ink_containment_applied(mut self, value: bool) -> Self {
        self.decision.ink_containment_applied = value;
        self
    }
    pub fn ink_bounds_fallback(mut self, value: Option<String>) -> Self {
        self.decision.ink_bounds_fallback = value;
        self
    }
    pub fn halt_advance(mut self, value: Option<f32>) -> Self {
        self.decision.halt_advance = value;
        self
    }
    pub fn halt_validation(mut self, value: Option<String>) -> Self {
        self.decision.halt_validation = value;
        self
    }
    pub fn advance_expansion(mut self, value: f32) -> Self {
        self.decision.advance_expansion = value;
        self
    }
    pub fn glyph_inline_shift(mut self, value: f32) -> Self {
        self.decision.glyph_inline_shift = value;
        self
    }
    pub fn glyph_placement_reason(mut self, value: Option<String>) -> Self {
        self.decision.glyph_placement_reason = value;
        self
    }
    pub fn leading_glue_initially_consumed(mut self, value: f32) -> Self {
        self.decision.leading_glue_initially_consumed = value;
        self
    }
    pub fn trailing_glue_initially_consumed(mut self, value: f32) -> Self {
        self.decision.trailing_glue_initially_consumed = value;
        self
    }
    pub fn build(self) -> PunctuationDecisionInfo {
        self.decision
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ClusterGeometryDecisionInfo {
    pub range: TextRange,
    pub source_text: Text,
    pub display_text: Text,
    pub base_advance: f32,
    pub body_width: f32,
    pub leading_glue_natural: f32,
    pub leading_glue_consumed: f32,
    pub trailing_glue_natural: f32,
    pub trailing_glue_consumed: f32,
    pub justification_delta: f32,
    /// ruby/注音 avoidance 增加的结构性 advance。selection geometry 可将其重分配给 owning annotation projection，
    /// 而不盲目赋给前一 cluster trailing box。
    pub ruby_spread: f32,
    /// 应用于此 cluster 的、由 layout 拥有的 glyph-origin shift。
    pub glyph_inline_shift: f32,
    /// `glyph_inline_shift` 的具名来源；未施加 shift 时为 None。
    pub glyph_placement_reason: Option<String>,
    pub resolved_advance: f32,
    pub source: String,
    pub reason: String,
}
impl ClusterGeometryDecisionInfo {
    pub fn builder(
        range: TextRange,
        source_text: Text,
        display_text: Text,
        base_advance: f32,
        body_width: f32,
        leading_glue_natural: f32,
        leading_glue_consumed: f32,
        trailing_glue_natural: f32,
        trailing_glue_consumed: f32,
        justification_delta: f32,
        resolved_advance: f32,
        source: String,
        reason: String,
    ) -> ClusterGeometryDecisionInfoBuilder {
        ClusterGeometryDecisionInfoBuilder {
            decision: Self {
                range,
                source_text,
                display_text,
                base_advance,
                body_width,
                leading_glue_natural,
                leading_glue_consumed,
                trailing_glue_natural,
                trailing_glue_consumed,
                justification_delta,
                ruby_spread: 0.0,
                glyph_inline_shift: 0.0,
                glyph_placement_reason: None,
                resolved_advance,
                source,
                reason,
            },
        }
    }
}
pub struct ClusterGeometryDecisionInfoBuilder {
    decision: ClusterGeometryDecisionInfo,
}
impl ClusterGeometryDecisionInfoBuilder {
    pub fn ruby_spread(mut self, value: f32) -> Self {
        self.decision.ruby_spread = value;
        self
    }
    pub fn glyph_inline_shift(mut self, value: f32) -> Self {
        self.decision.glyph_inline_shift = value;
        self
    }
    pub fn glyph_placement_reason(mut self, value: Option<String>) -> Self {
        self.decision.glyph_placement_reason = value;
        self
    }
    pub fn build(self) -> ClusterGeometryDecisionInfo {
        self.decision
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SpacingDecisionInfo {
    pub range: TextRange,
    pub left_char: char,
    pub right_char: char,
    pub natural_inner_glue: f32,
    pub adjusted_inner_glue: f32,
    pub reduction: f32,
    pub reduction_target_range: TextRange,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RoleOverrideInfo {
    pub range: TextRange,
    pub source_text: Text,
    pub original_role: String,
    pub overridden_role: String,
    pub source: String,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LineDecisionInfo {
    pub range: TextRange,
    pub kind: String,
    pub repair: Option<String>,
    pub repair_penalty: i32,
    pub repair_decision: Option<LineRepairDecisionInfo>,
    pub repair_candidates: Vec<LineRepairCandidateInfo>,
    pub notes: Vec<String>,
}
impl LineDecisionInfo {
    pub fn new(range: TextRange, kind: String) -> Self {
        Self {
            range,
            kind,
            repair: None,
            repair_penalty: 0,
            repair_decision: None,
            repair_candidates: Vec::new(),
            notes: Vec::new(),
        }
    }
    pub fn builder(range: TextRange, kind: String) -> LineDecisionInfoBuilder {
        LineDecisionInfoBuilder {
            decision: Self::new(range, kind),
        }
    }
}
pub struct LineDecisionInfoBuilder {
    decision: LineDecisionInfo,
}
impl LineDecisionInfoBuilder {
    pub fn repair(mut self, value: Option<String>) -> Self {
        self.decision.repair = value;
        self
    }
    pub fn repair_penalty(mut self, value: i32) -> Self {
        self.decision.repair_penalty = value;
        self
    }
    pub fn repair_decision(mut self, value: Option<LineRepairDecisionInfo>) -> Self {
        self.decision.repair_decision = value;
        self
    }
    pub fn repair_candidates(mut self, value: Vec<LineRepairCandidateInfo>) -> Self {
        self.decision.repair_candidates = value;
        self
    }
    pub fn notes(mut self, value: Vec<String>) -> Self {
        self.decision.notes = value;
        self
    }
    pub fn build(self) -> LineDecisionInfo {
        self.decision
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LineRepairDecisionInfo {
    pub kind: String,
    pub reason_code: String,
    pub offender_range: TextRange,
    pub penalty: i32,
    pub target_cluster_index: Option<i32>,
    pub carried_cluster_index: Option<i32>,
    /// 所有 allocation 的 PushIn total shrink；其他 repair kind 为 0。
    pub shrink: f32,
    /// decision 时 PushIn 的聚合 line-wide capacity。
    pub available_capacity: f32,
    /// CLREQ 推入的逐 cluster PushIn distribution。non-PushIn repair 为空；按 cluster order 列出，
    /// `shrink` value 求和为 `shrink`。
    pub push_in_allocations: Vec<LineRepairAllocationInfo>,
}
impl LineRepairDecisionInfo {
    pub fn new(kind: String, reason_code: String, offender_range: TextRange, penalty: i32) -> Self {
        Self {
            kind,
            reason_code,
            offender_range,
            penalty,
            target_cluster_index: None,
            carried_cluster_index: None,
            shrink: 0.0,
            available_capacity: 0.0,
            push_in_allocations: Vec::new(),
        }
    }
    pub fn builder(
        kind: String,
        reason_code: String,
        offender_range: TextRange,
        penalty: i32,
    ) -> LineRepairDecisionInfoBuilder {
        LineRepairDecisionInfoBuilder {
            decision: Self::new(kind, reason_code, offender_range, penalty),
        }
    }
}
pub struct LineRepairDecisionInfoBuilder {
    decision: LineRepairDecisionInfo,
}
impl LineRepairDecisionInfoBuilder {
    pub fn target_cluster_index(mut self, value: Option<i32>) -> Self {
        self.decision.target_cluster_index = value;
        self
    }
    pub fn carried_cluster_index(mut self, value: Option<i32>) -> Self {
        self.decision.carried_cluster_index = value;
        self
    }
    pub fn shrink(mut self, value: f32) -> Self {
        self.decision.shrink = value;
        self
    }
    pub fn available_capacity(mut self, value: f32) -> Self {
        self.decision.available_capacity = value;
        self
    }
    pub fn push_in_allocations(mut self, value: Vec<LineRepairAllocationInfo>) -> Self {
        self.decision.push_in_allocations = value;
        self
    }
    pub fn build(self) -> LineRepairDecisionInfo {
        self.decision
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LineRepairAllocationInfo {
    pub cluster_range: TextRange,
    pub shrink: f32,
    pub available_capacity: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LineRepairCandidateInfo {
    pub kind: String,
    pub reason_code: String,
    pub offender_range: TextRange,
    pub penalty: i32,
    pub accepted: bool,
    pub rejection_reason: Option<String>,
    pub target_cluster_index: Option<i32>,
    pub carried_cluster_index: Option<i32>,
    pub shrink: f32,
    pub required_shrink: f32,
    pub available_capacity: f32,
}
impl LineRepairCandidateInfo {
    pub fn new(
        kind: String,
        reason_code: String,
        offender_range: TextRange,
        penalty: i32,
        accepted: bool,
    ) -> Self {
        Self {
            kind,
            reason_code,
            offender_range,
            penalty,
            accepted,
            rejection_reason: None,
            target_cluster_index: None,
            carried_cluster_index: None,
            shrink: 0.0,
            required_shrink: 0.0,
            available_capacity: 0.0,
        }
    }
    pub fn builder(
        kind: String,
        reason_code: String,
        offender_range: TextRange,
        penalty: i32,
        accepted: bool,
    ) -> LineRepairCandidateInfoBuilder {
        LineRepairCandidateInfoBuilder {
            candidate: Self::new(kind, reason_code, offender_range, penalty, accepted),
        }
    }
}
pub struct LineRepairCandidateInfoBuilder {
    candidate: LineRepairCandidateInfo,
}
impl LineRepairCandidateInfoBuilder {
    pub fn rejection_reason(mut self, value: Option<String>) -> Self {
        self.candidate.rejection_reason = value;
        self
    }
    pub fn target_cluster_index(mut self, value: Option<i32>) -> Self {
        self.candidate.target_cluster_index = value;
        self
    }
    pub fn carried_cluster_index(mut self, value: Option<i32>) -> Self {
        self.candidate.carried_cluster_index = value;
        self
    }
    pub fn shrink(mut self, value: f32) -> Self {
        self.candidate.shrink = value;
        self
    }
    pub fn required_shrink(mut self, value: f32) -> Self {
        self.candidate.required_shrink = value;
        self
    }
    pub fn available_capacity(mut self, value: f32) -> Self {
        self.candidate.available_capacity = value;
        self
    }
    pub fn build(self) -> LineRepairCandidateInfo {
        self.candidate
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct JustificationDecisionInfo {
    pub line_range: TextRange,
    pub deficit_before: f32,
    pub deficit_after: f32,
    pub allocations: Vec<JustificationAllocationInfo>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct JustificationAllocationInfo {
    pub cluster_range: TextRange,
    pub kind: String,
    pub priority: i32,
    pub delta: f32,
    pub reason: String,
}
