// 对应 Kotlin 源文件：engine/src/commonMain/kotlin/org/tiqian/layout/PunctuationModel.kt

use super::super::clreq::clreq_profile::{
    GlueSide, PunctuationClass, PunctuationGluePlacement, PunctuationWidthPolicy,
    clreq_punctuation_policies,
};
use super::super::core::geometry::{Rect, TextRange};
use super::super::core::text::Text;

#[derive(Clone, Debug, PartialEq)]
pub struct PunctuationAtom {
    pub range: TextRange,
    pub character: char,
    pub punctuation_class: PunctuationClass,
    pub advance: f32,
    pub ink_bounds: Option<Rect>,
    pub body_width: f32,
    /// 字体测得的 `halt` advance；None 表示走 ink/policy 路径。
    pub halt_advance: Option<f32>,
    /// 默认 glyph ink bounds 无法安全重放字体请求的 `halt` trim 时的原因。
    pub halt_validation: Option<String>,
    pub leading_glue: Glue,
    pub trailing_glue: Glue,
    pub anchor: PunctuationAnchor,
    pub geometry_source: String,
    pub policy_body_floor: f32,
    pub ink_width: Option<f32>,
    pub ink_center: Option<f32>,
    /// 所有可移除 sidebearing 消耗后的最小 body left。
    pub ink_containment_body_floor: Option<f32>,
    /// 具名 `InkContainmentBodyFloor` 决策；policy/halt 已充分时为 false。
    pub ink_containment_applied: bool,
    /// `MissingInkBoundsFallback` 原因；见 [`PunctuationInkInput::bounds_fallback_reason`]。
    pub ink_bounds_fallback: Option<String>,
    /// `UnderwidthPunctuationAdvanceExpansion` 添加的 layout advance。
    pub advance_expansion: f32,
    /// underwidth font-owned glyph box 在合成全宽 cell 内的位置。
    pub glyph_inline_shift: f32,
    /// glyph_inline_shift 非零时的具名 placement heuristic。
    pub glyph_placement_reason: Option<String>,
    /// 固定半宽标点在断行前已消耗的 leading glue。
    pub leading_glue_initially_consumed: f32,
    /// 固定半宽标点在断行前已消耗的 trailing glue。
    pub trailing_glue_initially_consumed: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PunctuationInkInput {
    pub advance: f32,
    pub ink_bounds: Option<Rect>,
    /// 字体测得的 OpenType `halt` advance，即字体设计者为该 mark 请求的压缩 advance。
    pub halt_advance: Option<f32>,
    /// 字体在 `halt` 下施加的 x placement shift；与 halt_advance 一起直接决定两侧压缩预算。
    pub halt_placement_x: Option<f32>,
    /// `MissingInkBoundsFallback` 的具名原因。仅在 shaping 已运行但没有 ink_bounds 时非 None。
    pub bounds_fallback_reason: Option<String>,
}

impl PunctuationInkInput {
    pub fn new(advance: f32) -> Self {
        Self {
            advance,
            ink_bounds: None,
            halt_advance: None,
            halt_placement_x: None,
            bounds_fallback_reason: None,
        }
    }

    pub fn builder(advance: f32) -> PunctuationInkInputBuilder {
        PunctuationInkInputBuilder {
            input: Self::new(advance),
        }
    }
}

pub struct PunctuationInkInputBuilder {
    input: PunctuationInkInput,
}
impl PunctuationInkInputBuilder {
    pub fn ink_bounds(mut self, value: Option<Rect>) -> Self {
        self.input.ink_bounds = value;
        self
    }
    pub fn halt_advance(mut self, value: Option<f32>) -> Self {
        self.input.halt_advance = value;
        self
    }
    pub fn halt_placement_x(mut self, value: Option<f32>) -> Self {
        self.input.halt_placement_x = value;
        self
    }
    pub fn bounds_fallback_reason(mut self, value: Option<String>) -> Self {
        self.input.bounds_fallback_reason = value;
        self
    }
    pub fn build(self) -> PunctuationInkInput {
        self.input
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PunctuationAnchor {
    Leading,
    Center,
    Trailing,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Glue {
    pub kind: GlueKind,
    pub min: f32,
    pub natural: f32,
    pub max: f32,
    pub priority: i32,
    pub penalty: i32,
}

impl Glue {
    pub fn new(
        kind: GlueKind,
        min: f32,
        natural: f32,
        max: f32,
        priority: i32,
        penalty: i32,
    ) -> Self {
        assert!(min <= natural, "Glue min must not exceed natural.");
        assert!(natural <= max, "Glue natural must not exceed max.");
        Self {
            kind,
            min,
            natural,
            max,
            priority,
            penalty,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GlueKind {
    PunctuationLeading,
    PunctuationTrailing,
    CjkLatinSpace,
    WordSpace,
    CjkInterChar,
    ProgressiveTechnical,
    EmergencyGraphemeTracking,
    InlineObjectPunctuationTrailing,
    InlineObjectRelation,
    InlineObjectBinaryOperator,
    InlineObjectBoundary,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AdjustmentOpportunity {
    pub range: TextRange,
    pub glue: Glue,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PunctuationSpacingAdjustment {
    pub range: TextRange,
    pub reduction_target_range: TextRange,
    pub left_char: char,
    pub right_char: char,
    pub natural_inner_glue: f32,
    pub adjusted_inner_glue: f32,
    pub reduction: f32,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PunctuationSpacingCompressionResult {
    pub adjustments: Vec<PunctuationSpacingAdjustment>,
}
impl PunctuationSpacingCompressionResult {
    pub fn new(adjustments: Vec<PunctuationSpacingAdjustment>) -> Self {
        Self { adjustments }
    }
    pub fn total_reduction(&self) -> f32 {
        self.adjustments
            .iter()
            .map(|adjustment| adjustment.reduction)
            .sum()
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct PunctuationSpacingCompressor;

impl PunctuationSpacingCompressor {
    /**
     * `CollapseAdjacentPunctuationInnerGlue`：相邻半宽标点的可见内部间距减去半 em（最低为零）。
     * 该规则不是将 natural glue 减半：`」。` 和 `「（` 的 body 接触，而 `。「` 仍保留半 em。
     * 连续 PauseOrStop mark（`！！`、`？！`）也按相邻 pair 压缩。
     */
    pub fn compress(
        &self,
        atoms: &[PunctuationAtom],
        em: f32,
    ) -> PunctuationSpacingCompressionResult {
        if atoms.len() < 2 {
            return PunctuationSpacingCompressionResult::new(Vec::new());
        }
        let em_half = em / 2.0;
        let adjustments = atoms
            .windows(2)
            .filter_map(|pair| {
                let left = &pair[0];
                let right = &pair[1];
                if left.range.end() != right.range.start() {
                    return None;
                }
                let left_trailing =
                    (left.trailing_glue.natural - left.trailing_glue_initially_consumed).max(0.0);
                let right_leading =
                    (right.leading_glue.natural - right.leading_glue_initially_consumed).max(0.0);
                let natural_inner_glue = left_trailing + right_leading;
                if natural_inner_glue <= 0.0 {
                    return None;
                }
                let adjusted_inner_glue = (natural_inner_glue - em_half).max(0.0);
                let reduction = natural_inner_glue - adjusted_inner_glue;
                if reduction <= 0.0 {
                    return None;
                }
                Some(PunctuationSpacingAdjustment {
                    range: TextRange::new(left.range.start(), right.range.end()),
                    reduction_target_range: if left_trailing >= right_leading {
                        left.range
                    } else {
                        right.range
                    },
                    left_char: left.character,
                    right_char: right.character,
                    natural_inner_glue,
                    adjusted_inner_glue,
                    reduction,
                    reason: "collapse-adjacent-punctuation-inner-glue".to_owned(),
                })
            })
            .collect();
        PunctuationSpacingCompressionResult::new(adjustments)
    }

    /**
     * `CollapseCjkClosingBeforeAsciiPointMark`：ASCII 点号使用 Latin face，因而不是
     * PunctuationAtom；但在 `」,` 中，前一 CJK closing mark 的 trailing 半 em 必须消耗。
     */
    pub fn compress_cjk_closing_before_ascii_point_mark(
        &self,
        atoms: &[PunctuationAtom],
        text: &Text,
        em: f32,
    ) -> PunctuationSpacingCompressionResult {
        let em_half = em / 2.0;
        let adjustments = atoms
            .iter()
            .filter_map(|left| {
                if left.punctuation_class != PunctuationClass::Closing {
                    return None;
                }
                let right_char = utf16_char_at_or_none(text, left.range.end())?;
                if !clreq_punctuation_policies::is_ascii_point_mark(right_char) {
                    return None;
                }
                let natural_inner_glue =
                    (left.trailing_glue.natural - left.trailing_glue_initially_consumed).max(0.0);
                if natural_inner_glue <= 0.0 {
                    return None;
                }
                let adjusted_inner_glue = (natural_inner_glue - em_half).max(0.0);
                let reduction = natural_inner_glue - adjusted_inner_glue;
                if reduction <= 0.0 {
                    return None;
                }
                Some(PunctuationSpacingAdjustment {
                    range: TextRange::new(left.range.start(), left.range.end() + 1),
                    reduction_target_range: left.range,
                    left_char: left.character,
                    right_char,
                    natural_inner_glue,
                    adjusted_inner_glue,
                    reduction,
                    reason: "collapse-cjk-closing-before-ascii-point-mark".to_owned(),
                })
            })
            .collect();
        PunctuationSpacingCompressionResult::new(adjustments)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PunctuationAtomBuilder {
    glue_placement: PunctuationGluePlacement,
    width_policy: PunctuationWidthPolicy,
}

impl Default for PunctuationAtomBuilder {
    fn default() -> Self {
        Self::new(
            PunctuationGluePlacement::MainlandSimplified,
            PunctuationWidthPolicy::default(),
        )
    }
}

impl PunctuationAtomBuilder {
    pub fn new(
        glue_placement: PunctuationGluePlacement,
        width_policy: PunctuationWidthPolicy,
    ) -> Self {
        Self {
            glue_placement,
            width_policy,
        }
    }

    pub fn build_at(&self, text: &Text, index: i32, em: f32) -> Option<PunctuationAtom> {
        let character = utf16_char_at_or_none(text, index)?;
        self.build(
            character,
            TextRange::new(index, index + 1),
            em,
            None,
            self.glue_placement,
            self.width_policy,
        )
    }

    /**
     * 字体证据优先于 regional fallback 构建标点 box。
     * `FontHaltFittedBodyCompression` 需要 halt advance 与 placement；否则
     * `InkBoundsFittedBodyCompression` 在三种 policy-width body box 中选择可容纳原始 ink 的最小者。
     * 只有两者都不能给出 side 时才采用 `ProfileGlueFallbackWithoutFontGeometry`。
     */
    pub fn build(
        &self,
        character: char,
        range: TextRange,
        em: f32,
        ink_input: Option<PunctuationInkInput>,
        glue_placement: PunctuationGluePlacement,
        width_policy: PunctuationWidthPolicy,
    ) -> Option<PunctuationAtom> {
        let policy = clreq_punctuation_policies::policy_for(character);
        if policy.punctuation_class == PunctuationClass::Other {
            return None;
        }
        let policy_advance = policy.default_advance_em * em;
        let shaped_advance = ink_input
            .as_ref()
            .map(|input| input.advance)
            .filter(|advance| *advance > 0.0);
        let raw_glyph_advance = shaped_advance.unwrap_or(policy_advance);
        let raw_ink_bounds = ink_input.as_ref().and_then(|input| input.ink_bounds);
        let policy_expansion = (policy_advance - raw_glyph_advance).max(0.0);
        // `UnderwidthPunctuationFullWidthBoxPlacement`：即使 fwid 后仍为 underwidth，也将原 glyph box
        // 放进合成的全宽 cell；opening 的缺失宽度位于 box 前，closing 位于后，居中标点平分。
        let synthesized_full_width_placement = if shaped_advance.is_some() && policy_expansion > 0.0
        {
            match glue_placement.glue_side_for(policy.punctuation_class) {
                GlueSide::LeadingOnly => policy_expansion,
                GlueSide::BothSides => policy_expansion / 2.0,
                GlueSide::TrailingOnly => 0.0,
            }
        } else {
            0.0
        };
        let ink_bounds =
            raw_ink_bounds.map(|bounds| shift_inline(bounds, synthesized_full_width_placement));
        let ink_width = ink_bounds.map(|bounds| bounds.width().max(0.0));
        let ink_center = ink_bounds.map(|bounds| (bounds.left + bounds.right) / 2.0);
        let advance = raw_glyph_advance
            .max(policy_advance)
            .max(ink_bounds.map_or(0.0, |bounds| bounds.right));
        let advance_expansion = (advance - raw_glyph_advance).max(0.0);
        let policy_body_floor = policy.default_body_em * em;
        // 已为比例 glyph 合成全宽 cell 时，halt 不是该 cell 的半宽 form，不能接受。
        let halt_body = ink_input
            .as_ref()
            .and_then(|input| input.halt_advance)
            .filter(|halt_advance| {
                policy_expansion <= PLACEMENT_EPSILON
                    && *halt_advance > 0.0
                    && *halt_advance < advance
            });
        let forced_half = clreq_punctuation_policies::forced_half_width(character, width_policy);
        let compression = self.compression_geometry(
            advance,
            raw_glyph_advance,
            halt_body.unwrap_or(if forced_half {
                policy_body_floor.min(0.5 * em)
            } else {
                policy_body_floor
            }),
            ink_bounds,
            halt_body,
            ink_input.as_ref().and_then(|input| input.halt_placement_x),
            policy.punctuation_class,
            glue_placement,
        );
        let leading_glue_initially_consumed = if forced_half {
            compression.leading_trim
        } else {
            0.0
        };
        let trailing_glue_initially_consumed = if forced_half {
            compression.trailing_trim
        } else {
            0.0
        };
        Some(PunctuationAtom {
            range,
            character,
            punctuation_class: policy.punctuation_class,
            advance,
            ink_bounds,
            body_width: compression.body_width,
            halt_advance: halt_body,
            halt_validation: compression.halt_validation,
            leading_glue: Glue::new(
                GlueKind::PunctuationLeading,
                0.0,
                compression.leading_trim,
                compression.leading_trim,
                0,
                0,
            ),
            trailing_glue: Glue::new(
                GlueKind::PunctuationTrailing,
                0.0,
                compression.trailing_trim,
                compression.trailing_trim,
                0,
                0,
            ),
            anchor: compression.anchor,
            geometry_source: if forced_half {
                format!("{}FixedHalfWidth", compression.source)
            } else {
                compression.source.to_owned()
            },
            policy_body_floor,
            ink_width,
            ink_center,
            ink_containment_body_floor: compression.ink_body_floor,
            ink_containment_applied: compression.ink_containment_applied,
            ink_bounds_fallback: if ink_bounds.is_none() {
                ink_input
                    .as_ref()
                    .and_then(|input| input.bounds_fallback_reason.clone())
            } else {
                None
            },
            advance_expansion,
            glyph_inline_shift: synthesized_full_width_placement,
            glyph_placement_reason: (synthesized_full_width_placement != 0.0)
                .then(|| "UnderwidthPunctuationFullWidthBoxPlacement".to_owned()),
            leading_glue_initially_consumed,
            trailing_glue_initially_consumed,
        })
    }

    fn compression_geometry(
        &self,
        advance: f32,
        raw_glyph_advance: f32,
        target_body: f32,
        ink_bounds: Option<Rect>,
        halt_body: Option<f32>,
        halt_placement_x: Option<f32>,
        punctuation_class: PunctuationClass,
        glue_placement: PunctuationGluePlacement,
    ) -> CompressionGeometry {
        let requested_reduction = (advance - target_body).max(0.0);
        if let (Some(halt_body), Some(halt_placement_x)) = (halt_body, halt_placement_x)
            && halt_placement_x.is_finite()
        {
            let raw_reduction = (raw_glyph_advance - halt_body).max(0.0);
            let requested_leading = (-halt_placement_x).clamp(0.0, raw_reduction);
            let requested_leading = if requested_leading > PLACEMENT_EPSILON {
                requested_leading
            } else {
                0.0
            };
            let requested_trailing = (requested_reduction - requested_leading).max(0.0);
            let leading = ink_bounds.map_or(requested_leading, |bounds| {
                requested_leading.min(bounds.left.max(0.0))
            });
            let trailing = ink_bounds.map_or(requested_trailing, |bounds| {
                requested_trailing.min((advance - bounds.right).max(0.0))
            });
            let limited = leading + PLACEMENT_EPSILON < requested_leading
                || trailing + PLACEMENT_EPSILON < requested_trailing;
            return CompressionGeometry {
                leading_trim: leading,
                trailing_trim: trailing,
                body_width: advance - leading - trailing,
                anchor: anchor_for(leading, trailing),
                source: "FontHaltFittedBodyCompression",
                ink_body_floor: ink_bounds.map(|_| advance - leading - trailing),
                ink_containment_applied: limited,
                halt_validation: limited
                    .then(|| "halt-trim-limited-by-default-ink-bounds".to_owned()),
            };
        }
        if let Some(ink_bounds) = ink_bounds {
            let frame = fitted_body_frame(advance, target_body.clamp(0.0, advance), ink_bounds);
            return CompressionGeometry {
                leading_trim: frame.start,
                trailing_trim: (advance - frame.start - frame.width).max(0.0),
                body_width: frame.width,
                anchor: frame.anchor,
                source: if halt_body.is_some() {
                    "FontHaltAdvanceWithInkBoundsFittedPlacement"
                } else {
                    "InkBoundsFittedBodyCompression"
                },
                ink_body_floor: Some(frame.width),
                ink_containment_applied: frame.width > target_body + PLACEMENT_EPSILON,
                halt_validation: None,
            };
        }
        let (leading, trailing) =
            class_based_glue(punctuation_class, requested_reduction, glue_placement);
        CompressionGeometry {
            leading_trim: leading,
            trailing_trim: trailing,
            body_width: advance - leading - trailing,
            anchor: anchor_for(leading, trailing),
            source: if halt_body.is_some() {
                "FontHaltAdvanceWithProfileFallback"
            } else {
                "ProfileGlueFallbackWithoutFontGeometry"
            },
            ink_body_floor: None,
            ink_containment_applied: false,
            halt_validation: None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct BodyFrame {
    anchor: PunctuationAnchor,
    start: f32,
    width: f32,
}
#[derive(Clone, Debug)]
struct CompressionGeometry {
    leading_trim: f32,
    trailing_trim: f32,
    body_width: f32,
    anchor: PunctuationAnchor,
    source: &'static str,
    ink_body_floor: Option<f32>,
    ink_containment_applied: bool,
    halt_validation: Option<String>,
}

fn shift_inline(bounds: Rect, amount: f32) -> Rect {
    if amount == 0.0 {
        bounds
    } else {
        Rect {
            left: bounds.left + amount,
            top: bounds.top,
            right: bounds.right + amount,
            bottom: bounds.bottom,
        }
    }
}

fn fitted_body_frame(advance: f32, target_body: f32, ink_bounds: Rect) -> BodyFrame {
    let leading_width = target_body
        .max(ink_bounds.right)
        .clamp(target_body, advance);
    let trailing_width = target_body
        .max(advance - ink_bounds.left)
        .clamp(target_body, advance);
    let centered_width = target_body
        .max(advance - 2.0 * ink_bounds.left)
        .max(2.0 * ink_bounds.right - advance)
        .clamp(target_body, advance);
    let candidates = [
        BodyFrame {
            anchor: PunctuationAnchor::Leading,
            start: 0.0,
            width: leading_width,
        },
        BodyFrame {
            anchor: PunctuationAnchor::Center,
            start: (advance - centered_width) / 2.0,
            width: centered_width,
        },
        BodyFrame {
            anchor: PunctuationAnchor::Trailing,
            start: advance - trailing_width,
            width: trailing_width,
        },
    ];
    let ink_center = (ink_bounds.left + ink_bounds.right) / 2.0;
    *candidates
        .iter()
        .min_by(|left, right| {
            left.width
                .total_cmp(&right.width)
                .then_with(|| {
                    ((left.start + left.width / 2.0) - ink_center)
                        .abs()
                        .total_cmp(&((right.start + right.width / 2.0) - ink_center).abs())
                })
                .then_with(|| anchor_ordinal(left.anchor).cmp(&anchor_ordinal(right.anchor)))
        })
        .expect("three body-frame candidates are always present")
}

fn anchor_ordinal(anchor: PunctuationAnchor) -> i32 {
    match anchor {
        PunctuationAnchor::Leading => 0,
        PunctuationAnchor::Center => 1,
        PunctuationAnchor::Trailing => 2,
    }
}

fn anchor_for(leading_trim: f32, trailing_trim: f32) -> PunctuationAnchor {
    if leading_trim > PLACEMENT_EPSILON && trailing_trim > PLACEMENT_EPSILON {
        PunctuationAnchor::Center
    } else if leading_trim > PLACEMENT_EPSILON {
        PunctuationAnchor::Trailing
    } else if trailing_trim > PLACEMENT_EPSILON {
        PunctuationAnchor::Leading
    } else {
        PunctuationAnchor::Center
    }
}

fn class_based_glue(
    punctuation_class: PunctuationClass,
    total_glue: f32,
    glue_placement: PunctuationGluePlacement,
) -> (f32, f32) {
    match glue_placement.glue_side_for(punctuation_class) {
        GlueSide::LeadingOnly => (total_glue, 0.0),
        GlueSide::TrailingOnly => (0.0, total_glue),
        GlueSide::BothSides => (total_glue / 2.0, total_glue / 2.0),
    }
}

fn utf16_char_at_or_none(text: &Text, index: i32) -> Option<char> {
    text.code_point_at_or_none(index)
        .and_then(|code_point| char::from_u32(code_point as u32))
}

const PLACEMENT_EPSILON: f32 = 0.001;
