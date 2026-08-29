// 对应 Kotlin 源文件：engine/src/commonMain/kotlin/org/tiqian/shaping/TextShaper.kt

use super::super::core::Geometry::TextRange;
use super::super::core::LayoutModel::{Cluster, Glyph, GlyphRun, ShapingDecisionInfo};
use super::super::core::Text::Text;
use super::super::core::TextModel::TextStyle;
use super::super::font::FontPolicy::FontDecision;

/// 缺少 face evidence 的 display substitution 所使用的跨模块 capability contract。
pub const UNVERIFIED_DISPLAY_SUBSTITUTION_COVERAGE_ISSUE: &str =
    "UnverifiedDisplaySubstitutionCoverage";

/**
 * 平台将一个 shaping segment itemize 到多个 physical face 时使用的跨模块 capability contract：
 * 例如 CJK 基字带有其 face 缺失的 combining mark、非 CJK script run（Thai、Arabic 等），或跨越
 * fallback 边界的 Latin word。单个 controlled-byte face 无法重放这样的 segment，因此 backend
 * 通过平台 text stack 测量并绘制，而不是进行 outline replay，从而保持 source range 与 layout 正确。
 */
pub const PLATFORM_MULTI_FACE_STRING_DRAW_ISSUE: &str = "PlatformMultiFaceStringDraw";

#[derive(Clone, Debug, PartialEq)]
pub struct ShapingInput {
    pub text: Text,
    pub range: TextRange,
    pub style: TextStyle,
    pub font_decision: FontDecision,
    pub display_text: Text,
    /// 此 replay run 明确要求的非默认 OpenType feature。
    pub open_type_features: Vec<String>,
}

impl ShapingInput {
    pub fn new(
        text: Text,
        range: TextRange,
        style: TextStyle,
        font_decision: FontDecision,
    ) -> Self {
        let display_text = Text::from(text.slice(range));
        Self {
            text,
            range,
            style,
            font_decision,
            display_text,
            open_type_features: Vec::new(),
        }
    }

    pub fn builder(
        text: Text,
        range: TextRange,
        style: TextStyle,
        font_decision: FontDecision,
    ) -> ShapingInputBuilder {
        ShapingInputBuilder {
            input: Self::new(text, range, style, font_decision),
        }
    }
}

pub struct ShapingInputBuilder {
    input: ShapingInput,
}

impl ShapingInputBuilder {
    pub fn display_text(mut self, value: Text) -> Self {
        self.input.display_text = value;
        self
    }

    pub fn open_type_features(mut self, value: Vec<String>) -> Self {
        self.input.open_type_features = value;
        self
    }

    pub fn build(self) -> ShapingInput {
        self.input
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ShapingResult {
    pub clusters: Vec<Cluster>,
    pub glyph_runs: Vec<GlyphRun>,
    pub decisions: Vec<ShapingDecisionInfo>,
}

impl ShapingResult {
    pub fn new(clusters: Vec<Cluster>, glyph_runs: Vec<GlyphRun>) -> Self {
        Self {
            clusters,
            glyph_runs,
            decisions: Vec::new(),
        }
    }

    pub fn with_decisions(
        clusters: Vec<Cluster>,
        glyph_runs: Vec<GlyphRun>,
        decisions: Vec<ShapingDecisionInfo>,
    ) -> Self {
        Self {
            clusters,
            glyph_runs,
            decisions,
        }
    }
}

pub trait TextShaper {
    fn shape(&self, input: &ShapingInput) -> ShapingResult;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapingSource {
    Stub,
    JvmAwt,
    AndroidPaint,
    Skia,
    HarfBuzz,
    CoreText,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ExplainableStubTextShaper;

impl TextShaper for ExplainableStubTextShaper {
    fn shape(&self, input: &ShapingInput) -> ShapingResult {
        let source_text = Text::from(input.text.slice(input.range));
        let glyph_count = input.display_text.chars().count().max(1) as i32;
        let advance = input.style.font_size * nominal_advance_em(&source_text, &input.display_text);
        let cluster = Cluster::with_display_text(
            input.range,
            source_text.clone(),
            input.display_text.clone(),
            input.font_decision.candidate.key.clone(),
            advance,
        );
        let glyph_advance = advance / glyph_count as f32;
        let glyphs = (0..glyph_count)
            .map(|glyph_id| {
                Glyph::builder(glyph_id as u32, input.range, glyph_advance)
                    .x(glyph_advance * glyph_id as f32)
                    .build()
            })
            .collect();
        let run = GlyphRun::new(
            input.range,
            input.font_decision.candidate.key.clone(),
            glyphs,
            advance,
        );
        let decision = ShapingDecisionInfo::builder(
            input.range,
            source_text,
            input.display_text.clone(),
            input.font_decision.candidate.key.clone(),
            glyph_count,
            advance,
            "Stub".to_owned(),
            "ExplainableStubTextShaper:nominal-em-advance".to_owned(),
        )
        // stub 从不测量 ink；将所有 glyph 报告为无 bounds，使 dump 如实表明此路径不提供 ink diagnostics。
        .glyphs_without_ink_bounds(glyph_count)
        .build();
        ShapingResult::with_decisions(vec![cluster], vec![run], vec![decision])
    }
}

fn nominal_advance_em(source_text: &Text, display_text: &Text) -> f32 {
    if source_text == "⸺" || display_text == "⸺" {
        2.0
    // 简体中文网格中的 U+0020 是二分空（半 em），而不是全宽空白。deterministic stub 将每个
    // 空格建模为 0.5em，使 word space 与 sino-western gap 处于现实的不足/等于 em 宽度，
    // 而不是人为的 1em。
    } else if !source_text.is_empty() && source_text.chars().all(|character| character == ' ') {
        0.5 * source_text.utf16_len() as f32
    } else {
        source_text
            .chars()
            .count()
            .max(display_text.chars().count()) as f32
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct UnimplementedTextShaper;

impl TextShaper for UnimplementedTextShaper {
    fn shape(&self, _input: &ShapingInput) -> ShapingResult {
        panic!("Text shaping is platform-specific and has not been wired for this target yet.")
    }
}
