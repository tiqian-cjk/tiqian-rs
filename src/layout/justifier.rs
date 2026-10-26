// 对应 Kotlin 源文件：engine/src/commonMain/kotlin/org/tiqian/layout/Justifier.kt

use alloc::{borrow::ToOwned, format, string::String, vec::Vec};
use crate::common::{HashMap, HashSet};

use super::super::core::east_asian_spacing::{EastAsianSpacingEdges, EastAsianSpacingValue};
use super::super::core::int_range::IntRange;
use super::super::core::layout_model::{Cluster, EmergencyTrackingReason};
use super::super::core::text_model::{
    InlineObjectPreferredStretch, InlineObjectPreferredStretchKind,
};
use super::super::font::font_policy::FontRole;
use super::line_optimization::PushInAllocation;
use super::progressive_break_decisions::{ProgressiveBreakTier, ShrinkOpportunity};
use super::punctuation_model::GlueKind;

const MISSING_EAST_ASIAN_SPACING_EDGES: EastAsianSpacingEdges = EastAsianSpacingEdges {
    leading: EastAsianSpacingValue::Other,
    trailing: EastAsianSpacingValue::Other,
    contains_wide: false,
};

#[derive(Clone, Debug)]
pub struct JustificationRequest<'a> {
    pub adjusted_clusters: &'a [Cluster],
    pub cluster_roles: &'a [FontRole],
    pub east_asian_spacing_edges: &'a [EastAsianSpacingEdges],
    pub line_cluster_range: IntRange,
    pub max_width: f32,
    pub font_size: f32,
    pub skip: bool,
    pub skip_reason: Option<String>,
    pub allow_sino_western_gap_stretch: bool,
    pub cjk_latin_space_base_em: f32,
    pub cjk_latin_space_max_em: f32,
    pub no_stretch_boundary_clusters: &'a HashSet<i32>,
    pub no_stretch_boundary_after_clusters: &'a HashSet<i32>,
    pub western_bracket_cjk_inter_char_boundary_after_clusters: &'a HashSet<i32>,
    pub attached_inline_physical_boundary_after_clusters: &'a HashSet<i32>,
    pub attached_inline_virtual_boundary_after_clusters: &'a HashMap<i32, i32>,
    pub attached_inline_virtual_sino_western_boundary_after_clusters: &'a HashSet<i32>,
    pub uniform_inline_object_boundary_after_clusters: &'a HashSet<i32>,
    pub preferred_inline_object_boundary_after_clusters:
        &'a HashMap<i32, InlineObjectPreferredStretch>,
    pub technical_boundary_after_clusters: &'a HashMap<i32, ProgressiveBreakTier>,
    pub emergency_tracking_boundary_after_clusters: &'a HashMap<i32, EmergencyTrackingReason>,
    pub preferred_emergency_tracking_boundary_after_clusters:
        &'a HashMap<i32, EmergencyTrackingReason>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JustificationReason {
    /// attached inline 虚拟自动间距边界。
    AttachedInlineVirtualAutoSpace,
    /// 西文括号与 CJK 之间的 inter-char 边界。
    WesternBracketCjkInterChar,
    /// attached inline 虚拟边界两侧的 inter-char 伸展。
    AttachedInlineVirtualInterChar,
    /// 行内对象 preferred stretch 的子类，与阶段对应。
    InlineObjectPreferredStretch(InlineObjectPreferredStretchKind),
    /// 当前行已选 emergency technical break 时的首选 tracking 边界。
    TerminalTechnicalEmergencyTracking(EmergencyTrackingReason),
    /// 普通的 emergency grapheme tracking 边界。
    EmergencyGraphemeTracking(EmergencyTrackingReason),
}

impl JustificationReason {
    /// dump 使用的原有文本。
    pub fn name(self) -> String {
        match self {
            Self::AttachedInlineVirtualAutoSpace => {
                "AttachedInlineVirtualAutoSpace".to_owned()
            }
            Self::WesternBracketCjkInterChar => "WesternBracketCjkInterChar".to_owned(),
            Self::AttachedInlineVirtualInterChar => {
                "AttachedInlineVirtualInterChar".to_owned()
            }
            Self::InlineObjectPreferredStretch(kind) => preferred_reason(kind).to_owned(),
            Self::TerminalTechnicalEmergencyTracking(reason) => {
                format!("TerminalTechnicalEmergencyTracking:{}", reason.name())
            }
            Self::EmergencyGraphemeTracking(reason) => {
                format!("EmergencyGraphemeTracking:{}", reason.name())
            }
        }
    }
}

/// 最终 allocation 的原因：调用点显式给出的修饰，或整阶段默认。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JustificationAllocationReason {
    Stage(JustificationStage),
    Opportunity(JustificationReason),
}

impl JustificationAllocationReason {
    /// dump 使用的原有文本。
    pub fn name(self) -> String {
        match self {
            Self::Stage(stage) => stage.name().to_owned(),
            Self::Opportunity(reason) => reason.name(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct JustificationOpportunity {
    pub target_cluster_index: i32,
    pub kind: GlueKind,
    pub priority: i32,
    pub capacity: f32,
    pub reason: Option<JustificationReason>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct JustificationAllocation {
    pub target_cluster_index: i32,
    pub kind: GlueKind,
    pub priority: i32,
    pub delta: f32,
    pub reason: JustificationAllocationReason,
}
/// justifier 的阶段身份。
///
/// 阶段既用于 debug 说明，也决定哪些阶段容量视为无穷（`TerminalTechnicalEmergencyTracking`、
/// `CjkInterChar`、`EmergencyGraphemeTracking`），因此必须是可匹配的类型而不是文本。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JustificationStage {
    ProgressiveTechnicalWhitespaceStretch,
    WordSpace,
    CjkLatinSpace,
    /// 行内对象 preferred stretch 的三个子阶段。
    InlineObjectPreferredStretch(InlineObjectPreferredStretchKind),
    TerminalTechnicalEmergencyTracking,
    CjkInterChar,
    EmergencyGraphemeTracking,
}

impl JustificationStage {
    /// 该阶段的容量是否视为无穷。
    pub fn has_unbounded_capacity(self) -> bool {
        matches!(
            self,
            Self::TerminalTechnicalEmergencyTracking
                | Self::CjkInterChar
                | Self::EmergencyGraphemeTracking
        )
    }

    /// debug 输出使用的固定名称。
    pub fn name(self) -> &'static str {
        match self {
            Self::ProgressiveTechnicalWhitespaceStretch => "ProgressiveTechnicalWhitespaceStretch",
            Self::WordSpace => "WordSpace",
            Self::CjkLatinSpace => "CjkLatinSpace",
            Self::InlineObjectPreferredStretch(kind) => preferred_reason(kind),
            Self::TerminalTechnicalEmergencyTracking => "TerminalTechnicalEmergencyTracking",
            Self::CjkInterChar => "CjkInterChar",
            Self::EmergencyGraphemeTracking => "EmergencyGraphemeTracking",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct JustificationPlan {
    pub line_cluster_range: IntRange,
    pub allocations: Vec<JustificationAllocation>,
    pub deficit_before: f32,
    pub unfilled_deficit: f32,
    pub fallback_reason: Option<String>,
}
/// 含 Tab 的最终行只收集一次机会，数值试算复用阶段与容量。
pub struct JustificationStages {
    pub line_cluster_range: IntRange,
    pub deficit_before: f32,
    pub fallback_reason: Option<String>,
    stages: Vec<(Vec<JustificationOpportunity>, JustificationStage)>,
}

impl JustificationStages {
    /// 按既有阶段顺序填充分配缓冲，试算不分配机会或原因字符串。
    pub fn fill_deltas(&self, budget: f32, deltas: &mut [f32]) -> f32 {
        deltas.fill(0.0);
        let mut remaining = budget.max(0.0);
        for (opportunities, _) in &self.stages {
            if remaining <= 0.0 {
                break;
            }
            let infinite_count = opportunities
                .iter()
                .filter(|opportunity| opportunity.capacity == f32::INFINITY)
                .count();
            if infinite_count > 0 {
                let share = remaining / infinite_count as f32;
                for opportunity in opportunities {
                    if opportunity.capacity == f32::INFINITY {
                        let index = (opportunity.target_cluster_index
                            - self.line_cluster_range.first()) as usize;
                        if let Some(delta) = deltas.get_mut(index) {
                            *delta += share;
                        }
                    }
                }
                remaining = 0.0;
                continue;
            }
            let total: f32 = opportunities
                .iter()
                .map(|opportunity| opportunity.capacity.max(0.0))
                .sum();
            if total <= 0.0 {
                continue;
            }
            let allocated = remaining.min(total);
            let factor = allocated / total;
            for opportunity in opportunities {
                let delta = opportunity.capacity.max(0.0) * factor;
                if delta > 0.0 {
                    let index = (opportunity.target_cluster_index
                        - self.line_cluster_range.first()) as usize;
                    if let Some(value) = deltas.get_mut(index) {
                        *value += delta;
                    }
                }
            }
            remaining -= allocated;
        }
        remaining
    }

    /// 预算确定后一次性生成可解释的最终分配。
    pub fn plan(&self, budget: f32) -> JustificationPlan {
        let mut remaining = budget.max(0.0);
        let mut allocations = Vec::new();
        for (opportunities, stage) in &self.stages {
            if remaining <= 0.0 {
                break;
            }
            let infinite_count = opportunities
                .iter()
                .filter(|opportunity| opportunity.capacity == f32::INFINITY)
                .count();
            if infinite_count > 0 {
                let share = remaining / infinite_count as f32;
                for opportunity in opportunities {
                    if opportunity.capacity == f32::INFINITY && share > 0.0 {
                        allocations.push(JustificationAllocation {
                            target_cluster_index: opportunity.target_cluster_index,
                            kind: opportunity.kind,
                            priority: opportunity.priority,
                            delta: share,
                            reason: opportunity
                                .reason
                                .map(JustificationAllocationReason::Opportunity)
                                .unwrap_or(JustificationAllocationReason::Stage(*stage)),
                        });
                    }
                }
                remaining = 0.0;
                continue;
            }
            let total: f32 = opportunities
                .iter()
                .map(|opportunity| opportunity.capacity.max(0.0))
                .sum();
            if total <= 0.0 {
                continue;
            }
            let allocated = remaining.min(total);
            let factor = allocated / total;
            for opportunity in opportunities {
                let delta = opportunity.capacity.max(0.0) * factor;
                if delta > 0.0 {
                    allocations.push(JustificationAllocation {
                        target_cluster_index: opportunity.target_cluster_index,
                        kind: opportunity.kind,
                        priority: opportunity.priority,
                        delta,
                        reason: opportunity
                            .reason
                            .map(JustificationAllocationReason::Opportunity)
                            .unwrap_or(JustificationAllocationReason::Stage(*stage)),
                    });
                }
            }
            remaining -= allocated;
        }
        JustificationPlan {
            line_cluster_range: self.line_cluster_range,
            allocations,
            deficit_before: self.deficit_before,
            unfilled_deficit: remaining.max(0.0),
            fallback_reason: if remaining > 0.0 { self.fallback_reason.clone() } else { None },
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct CompressionPlan {
    pub allocations: Vec<PushInAllocation>,
    pub surplus_before: f32,
    pub unfilled_surplus: f32,
}

pub struct Justifier {
    word_space_max_em: f32,
    progressive_technical_whitespace_stretch_max_em: f32,
}

impl Default for Justifier {
    fn default() -> Self {
        Self::new(0.5, 0.25)
    }
}

impl Justifier {
    pub fn new(
        word_space_max_em: f32,
        progressive_technical_whitespace_stretch_max_em: f32,
    ) -> Self {
        Self {
            word_space_max_em,
            progressive_technical_whitespace_stretch_max_em,
        }
    }

    pub fn progressive_technical_whitespace_stretch_capacity(&self, font_size: f32) -> f32 {
        self.progressive_technical_whitespace_stretch_max_em * font_size
    }

    pub fn justify(&self, request: JustificationRequest<'_>) -> JustificationPlan {
        self.justify_with_allocator(request, |remaining, opportunities, reason, out| {
            self.allocate(remaining, opportunities, reason, out)
        })
    }

    /// 复用同一机会收集路径，完整记录每阶段，保持内部顺序与容量上限。
    pub fn prepare_stages(&self, request: JustificationRequest<'_>) -> JustificationStages {
        let mut stages = Vec::new();
        let plan = self.justify_with_allocator(
            request,
            |remaining, mut opportunities, stage, _| {
                if stage.has_unbounded_capacity() {
                    for opportunity in &mut opportunities {
                        opportunity.capacity = f32::INFINITY;
                    }
                }
                stages.push((opportunities, stage));
                remaining
            },
        );
        JustificationStages {
            line_cluster_range: plan.line_cluster_range,
            deficit_before: plan.deficit_before,
            fallback_reason: plan.fallback_reason,
            stages,
        }
    }

    fn justify_with_allocator(
        &self,
        request: JustificationRequest<'_>,
        mut allocate: impl FnMut(
            f32,
            Vec<JustificationOpportunity>,
            JustificationStage,
            &mut Vec<JustificationAllocation>,
        ) -> f32,
    ) -> JustificationPlan {
        let adjusted_width: f32 = request
            .line_cluster_range
            .into_iter()
            .map(|index| request.adjusted_clusters[index as usize].advance)
            .sum();
        let deficit_before = (request.max_width - adjusted_width).max(0.0);
        if request.skip || deficit_before <= 0.0 {
            return self.finalize(
                request.line_cluster_range,
                deficit_before,
                deficit_before,
                Vec::new(),
                if request.skip {
                    request.skip_reason
                } else {
                    None
                },
            );
        }
        let boundary_is_closed = |left: i32, right: i32| {
            request.no_stretch_boundary_after_clusters.contains(&left)
                || request.no_stretch_boundary_clusters.contains(&left)
                || request.no_stretch_boundary_clusters.contains(&right)
        };
        let space_gap_is_closed = |space: i32| {
            request
                .no_stretch_boundary_after_clusters
                .contains(&(space - 1))
                || request.no_stretch_boundary_after_clusters.contains(&space)
                || request.no_stretch_boundary_clusters.contains(&(space - 1))
                || request.no_stretch_boundary_clusters.contains(&(space + 1))
        };
        let mut remaining = deficit_before;
        let mut allocations = Vec::new();

        let technical = self.boundary_opportunities(
            &request,
            GlueKind::ProgressiveTechnical,
            ProgressiveBreakTier::Whitespace.priority(),
            self.progressive_technical_whitespace_stretch_capacity(request.font_size),
            None,
            |left, _| {
                request.technical_boundary_after_clusters.get(&left)
                    == Some(&ProgressiveBreakTier::Whitespace)
                    && request.adjusted_clusters[left as usize]
                        .text
                        .chars()
                        .all(char::is_whitespace)
            },
        );
        remaining = allocate(
            remaining,
            technical,
            JustificationStage::ProgressiveTechnicalWhitespaceStretch,
            &mut allocations,
        );
        if remaining <= 0.0 {
            return self.finalize(
                request.line_cluster_range,
                deficit_before,
                remaining,
                allocations,
                None,
            );
        }

        let mut word_spaces = Vec::new();
        for index in request.line_cluster_range {
            if !is_word_space_between_narrow(
                request.adjusted_clusters,
                index,
                request.east_asian_spacing_edges,
            ) || space_gap_is_closed(index)
            {
                continue;
            }
            let natural = request.adjusted_clusters[index as usize].advance;
            let capacity = (self.word_space_max_em * request.font_size - natural).max(0.0);
            if natural > 0.0 && capacity > 0.0 {
                word_spaces.push(JustificationOpportunity {
                    target_cluster_index: index,
                    kind: GlueKind::WordSpace,
                    priority: 0,
                    capacity,
                    reason: None,
                });
            }
        }
        remaining = allocate(
            remaining,
            word_spaces,
            JustificationStage::WordSpace,
            &mut allocations,
        );
        if remaining <= 0.0 {
            return self.finalize(
                request.line_cluster_range,
                deficit_before,
                remaining,
                allocations,
                None,
            );
        }

        if request.allow_sino_western_gap_stretch {
            let cap = ((request.cjk_latin_space_max_em - request.cjk_latin_space_base_em)
                * request.font_size)
                .max(0.0);
            let mut sino = self.boundary_opportunities(
                &request,
                GlueKind::CjkLatinSpace,
                1,
                cap,
                None,
                |left, right| {
                    is_wide_narrow_boundary(left, right, request.east_asian_spacing_edges)
                        && !request
                            .attached_inline_physical_boundary_after_clusters
                            .contains(&left)
                        && !boundary_is_closed(left, right)
                        && !request.adjusted_clusters[left as usize]
                            .text
                            .as_str()
                            .ends_with(' ')
                        && !request.adjusted_clusters[right as usize]
                            .text
                            .as_str()
                            .starts_with(' ')
                },
            );
            for target in request.line_cluster_range {
                if !request
                    .attached_inline_virtual_sino_western_boundary_after_clusters
                    .contains(&target)
                {
                    continue;
                }
                let Some(previous) = request
                    .attached_inline_virtual_boundary_after_clusters
                    .get(&target)
                else {
                    continue;
                };
                let next = target + 1;
                if !request.line_cluster_range.contains(next)
                    || request.no_stretch_boundary_clusters.contains(previous)
                    || request.no_stretch_boundary_clusters.contains(&next)
                {
                    continue;
                }
                sino.push(JustificationOpportunity {
                    target_cluster_index: target,
                    kind: GlueKind::CjkLatinSpace,
                    priority: 1,
                    capacity: cap,
                    reason: Some(JustificationReason::AttachedInlineVirtualAutoSpace),
                });
            }
            for index in request.line_cluster_range {
                if !is_wide_narrow_typed_space(
                    request.adjusted_clusters,
                    index,
                    request.east_asian_spacing_edges,
                ) || space_gap_is_closed(index)
                {
                    continue;
                }
                let width = request.adjusted_clusters[index as usize].advance;
                let capacity =
                    (request.cjk_latin_space_max_em * request.font_size - width).max(0.0);
                if width > 0.0 && capacity > 0.0 {
                    sino.push(JustificationOpportunity {
                        target_cluster_index: index,
                        kind: GlueKind::CjkLatinSpace,
                        priority: 1,
                        capacity,
                        reason: None,
                    });
                }
            }
            remaining = allocate(
                remaining,
                sino,
                JustificationStage::CjkLatinSpace,
                &mut allocations,
            );
            if remaining <= 0.0 {
                return self.finalize(
                    request.line_cluster_range,
                    deficit_before,
                    remaining,
                    allocations,
                    None,
                );
            }
        }

        for kind in [
            InlineObjectPreferredStretchKind::PunctuationTrailing,
            InlineObjectPreferredStretchKind::Relation,
            InlineObjectPreferredStretchKind::BinaryOperator,
        ] {
            let mut opportunities = Vec::new();
            for left in request.line_cluster_range.first()..request.line_cluster_range.last() {
                let Some(preferred) = request
                    .preferred_inline_object_boundary_after_clusters
                    .get(&left)
                else {
                    continue;
                };
                let right = left + 1;
                if preferred.kind != kind || boundary_is_closed(left, right) {
                    continue;
                }
                opportunities.push(JustificationOpportunity {
                    target_cluster_index: left,
                    kind: preferred_glue_kind(kind),
                    priority: 2,
                    capacity: preferred.capacity(),
                    reason: None,
                });
            }
            remaining = allocate(
                remaining,
                opportunities,
                JustificationStage::InlineObjectPreferredStretch(kind),
                &mut allocations,
            );
            if remaining <= 0.0 {
                return self.finalize(
                    request.line_cluster_range,
                    deficit_before,
                    remaining,
                    allocations,
                    None,
                );
            }
        }

        let terminal = self
            .boundary_opportunities(
                &request,
                GlueKind::EmergencyGraphemeTracking,
                3,
                remaining,
                None,
                |left, _| {
                    request
                        .preferred_emergency_tracking_boundary_after_clusters
                        .contains_key(&left)
                },
            )
            .into_iter()
            .map(|mut opportunity| {
                if let Some(reason) = request
                    .preferred_emergency_tracking_boundary_after_clusters
                    .get(&opportunity.target_cluster_index)
                {
                    opportunity.reason = Some(
                        JustificationReason::TerminalTechnicalEmergencyTracking(*reason),
                    );
                }
                opportunity
            })
            .collect();
        remaining = allocate(
            remaining,
            terminal,
            JustificationStage::TerminalTechnicalEmergencyTracking,
            &mut allocations,
        );
        if remaining <= 0.0 {
            return self.finalize(
                request.line_cluster_range,
                deficit_before,
                remaining,
                allocations,
                None,
            );
        }

        let has_cjk_body = request.line_cluster_range.into_iter().any(|index| {
            spacing_edges_at(request.east_asian_spacing_edges, index as usize).contains_wide
        });
        let has_object_boundary =
            (request.line_cluster_range.first()..request.line_cluster_range.last()).any(|left| {
                request
                    .uniform_inline_object_boundary_after_clusters
                    .contains(&left)
                    && !boundary_is_closed(left, left + 1)
            });
        let has_emergency_boundary =
            (request.line_cluster_range.first()..request.line_cluster_range.last()).any(|left| {
                request
                    .emergency_tracking_boundary_after_clusters
                    .contains_key(&left)
            });
        if !has_cjk_body && !has_object_boundary && !has_emergency_boundary {
            return self.finalize(
                request.line_cluster_range,
                deficit_before,
                remaining,
                allocations,
                Some("WesternDominantLineNaturalSpacing".to_owned()),
            );
        }

        let uniform_text = self.boundary_opportunities(
            &request,
            GlueKind::CjkInterChar,
            3,
            remaining,
            None,
            |left, right| {
                let left_role = role_at(request.cluster_roles, left as usize);
                let right_role = role_at(request.cluster_roles, right as usize);
                let both_cjk = is_cjk_like(left_role) && is_cjk_like(right_role);
                let punctuation_western = (left_role == FontRole::CjkPunctuation
                    && spacing_edges_at(request.east_asian_spacing_edges, right as usize).leading
                        == EastAsianSpacingValue::Narrow)
                    || (spacing_edges_at(request.east_asian_spacing_edges, left as usize).trailing
                        == EastAsianSpacingValue::Narrow
                        && right_role == FontRole::CjkPunctuation);
                let virtual_sino = request.allow_sino_western_gap_stretch
                    && is_wide_narrow_boundary(left, right, request.east_asian_spacing_edges);
                (both_cjk || punctuation_western || virtual_sino)
                    && !request
                        .western_bracket_cjk_inter_char_boundary_after_clusters
                        .contains(&left)
                    && !request
                        .attached_inline_physical_boundary_after_clusters
                        .contains(&left)
                    && !request
                        .attached_inline_virtual_boundary_after_clusters
                        .contains_key(&left)
                    && !request
                        .uniform_inline_object_boundary_after_clusters
                        .contains(&left)
                    && !boundary_is_closed(left, right)
            },
        );
        let bracket = self.boundary_opportunities(
            &request,
            GlueKind::CjkInterChar,
            3,
            remaining,
            Some(JustificationReason::WesternBracketCjkInterChar),
            |left, right| {
                request
                    .western_bracket_cjk_inter_char_boundary_after_clusters
                    .contains(&left)
                    && !request
                        .attached_inline_physical_boundary_after_clusters
                        .contains(&left)
                    && !request
                        .uniform_inline_object_boundary_after_clusters
                        .contains(&left)
                    && !boundary_is_closed(left, right)
            },
        );
        let attached = self.boundary_opportunities(
            &request,
            GlueKind::CjkInterChar,
            3,
            remaining,
            Some(JustificationReason::AttachedInlineVirtualInterChar),
            |left, right| {
                let Some(previous) = request
                    .attached_inline_virtual_boundary_after_clusters
                    .get(&left)
                else {
                    return false;
                };
                (request.allow_sino_western_gap_stretch
                    || !request
                        .attached_inline_virtual_sino_western_boundary_after_clusters
                        .contains(&left))
                    && !request
                        .uniform_inline_object_boundary_after_clusters
                        .contains(&left)
                    && !request.no_stretch_boundary_clusters.contains(previous)
                    && !request.no_stretch_boundary_clusters.contains(&right)
                    && !request
                        .no_stretch_boundary_after_clusters
                        .contains(previous)
            },
        );
        let object = self.boundary_opportunities(
            &request,
            GlueKind::InlineObjectBoundary,
            3,
            remaining,
            None,
            |left, right| {
                request
                    .uniform_inline_object_boundary_after_clusters
                    .contains(&left)
                    && !boundary_is_closed(left, right)
            },
        );
        let mut spaces = Vec::new();
        for index in request.line_cluster_range {
            let word = is_word_space_between_narrow(
                request.adjusted_clusters,
                index,
                request.east_asian_spacing_edges,
            );
            let typed = request.allow_sino_western_gap_stretch
                && is_wide_narrow_typed_space(
                    request.adjusted_clusters,
                    index,
                    request.east_asian_spacing_edges,
                );
            if (word || typed)
                && request.adjusted_clusters[index as usize].advance > 0.0
                && !space_gap_is_closed(index)
            {
                spaces.push(JustificationOpportunity {
                    target_cluster_index: index,
                    kind: GlueKind::CjkInterChar,
                    priority: 3,
                    capacity: remaining,
                    reason: None,
                });
            }
        }
        remaining = allocate(
            remaining,
            [uniform_text, bracket, attached, object, spaces].concat(),
            JustificationStage::CjkInterChar,
            &mut allocations,
        );
        if remaining <= 0.0 {
            return self.finalize(
                request.line_cluster_range,
                deficit_before,
                remaining,
                allocations,
                None,
            );
        }

        let emergency = self
            .boundary_opportunities(
                &request,
                GlueKind::EmergencyGraphemeTracking,
                4,
                remaining,
                None,
                |left, _| {
                    request
                        .emergency_tracking_boundary_after_clusters
                        .contains_key(&left)
                        && !request
                            .preferred_emergency_tracking_boundary_after_clusters
                            .contains_key(&left)
                },
            )
            .into_iter()
            .map(|mut opportunity| {
                if let Some(reason) = request
                    .emergency_tracking_boundary_after_clusters
                    .get(&opportunity.target_cluster_index)
                {
                    opportunity.reason =
                        Some(JustificationReason::EmergencyGraphemeTracking(*reason));
                }
                opportunity
            })
            .collect();
        remaining = allocate(
            remaining,
            emergency,
            JustificationStage::EmergencyGraphemeTracking,
            &mut allocations,
        );
        self.finalize(
            request.line_cluster_range,
            deficit_before,
            remaining,
            allocations,
            if remaining > 0.0 && has_emergency_boundary {
                Some("EmergencyTrackingNoOpenBoundary".to_owned())
            } else {
                None
            },
        )
    }

    pub fn compress(
        &self,
        surplus: f32,
        shrink_opportunities: &[ShrinkOpportunity],
    ) -> CompressionPlan {
        if surplus.is_nan() {
            return CompressionPlan {
                allocations: Vec::new(),
                surplus_before: surplus,
                unfilled_surplus: surplus,
            };
        }
        if surplus <= 0.0 {
            return CompressionPlan {
                allocations: Vec::new(),
                surplus_before: 0.0,
                unfilled_surplus: 0.0,
            };
        }
        let mut remaining = surplus;
        let mut tiers: Vec<i32> = shrink_opportunities
            .iter()
            .filter(|opportunity| opportunity.capacity > 0.0)
            .map(|opportunity| opportunity.tier)
            .collect();
        tiers.sort();
        tiers.dedup();
        let mut allocations = Vec::new();
        for tier in tiers {
            if remaining <= 0.0 {
                break;
            }
            let opportunities: Vec<_> = shrink_opportunities
                .iter()
                .filter(|opportunity| opportunity.tier == tier && opportunity.capacity > 0.0)
                .collect();
            let total: f32 = opportunities
                .iter()
                .map(|opportunity| opportunity.capacity)
                .sum();
            if total <= 0.0 {
                continue;
            }
            let factor = (remaining / total).min(1.0);
            for opportunity in opportunities {
                let shrink = opportunity.capacity * factor;
                if shrink > 0.0 {
                    allocations.push(PushInAllocation {
                        cluster_index: opportunity.cluster_index,
                        shrink,
                        available_capacity: opportunity.capacity,
                        channel: opportunity.channel,
                    });
                }
            }
            remaining -= total * factor;
        }
        CompressionPlan {
            allocations,
            surplus_before: surplus,
            unfilled_surplus: remaining.max(0.0),
        }
    }

    fn boundary_opportunities(
        &self,
        request: &JustificationRequest<'_>,
        kind: GlueKind,
        priority: i32,
        capacity: f32,
        reason: Option<JustificationReason>,
        predicate: impl Fn(i32, i32) -> bool,
    ) -> Vec<JustificationOpportunity> {
        if capacity <= 0.0 || request.line_cluster_range.is_empty() {
            return Vec::new();
        }
        (request.line_cluster_range.first()..request.line_cluster_range.last())
            .filter(|left| predicate(*left, *left + 1))
            .map(|left| JustificationOpportunity {
                target_cluster_index: left,
                kind,
                priority,
                capacity,
                reason,
            })
            .collect()
    }

    fn allocate(
        &self,
        deficit: f32,
        opportunities: Vec<JustificationOpportunity>,
        stage: JustificationStage,
        allocations: &mut Vec<JustificationAllocation>,
    ) -> f32 {
        if deficit <= 0.0 || opportunities.is_empty() {
            return deficit;
        }
        let total: f32 = opportunities
            .iter()
            .map(|opportunity| opportunity.capacity)
            .sum();
        if total <= 0.0 {
            return deficit;
        }
        if total >= deficit {
            let factor = deficit / total;
            for opportunity in opportunities {
                let delta = opportunity.capacity * factor;
                if delta > 0.0 {
                    allocations.push(JustificationAllocation {
                        target_cluster_index: opportunity.target_cluster_index,
                        kind: opportunity.kind,
                        priority: opportunity.priority,
                        delta,
                        reason: opportunity
                            .reason
                            .map(JustificationAllocationReason::Opportunity)
                            .unwrap_or(JustificationAllocationReason::Stage(stage)),
                    });
                }
            }
            0.0
        } else {
            for opportunity in opportunities {
                if opportunity.capacity > 0.0 {
                    allocations.push(JustificationAllocation {
                        target_cluster_index: opportunity.target_cluster_index,
                        kind: opportunity.kind,
                        priority: opportunity.priority,
                        delta: opportunity.capacity,
                        reason: opportunity
                            .reason
                            .map(JustificationAllocationReason::Opportunity)
                            .unwrap_or(JustificationAllocationReason::Stage(stage)),
                    });
                }
            }
            deficit - total
        }
    }

    fn finalize(
        &self,
        line_cluster_range: IntRange,
        deficit_before: f32,
        unfilled: f32,
        allocations: Vec<JustificationAllocation>,
        fallback_reason: Option<String>,
    ) -> JustificationPlan {
        JustificationPlan {
            line_cluster_range,
            allocations,
            deficit_before,
            unfilled_deficit: unfilled.max(0.0),
            fallback_reason,
        }
    }
}

fn is_word_space_between_narrow(
    clusters: &[Cluster],
    index: i32,
    edges: &[EastAsianSpacingEdges],
) -> bool {
    let Some(cluster) = clusters.get(index as usize) else {
        return false;
    };
    !cluster.text.is_empty()
        && cluster.text.chars().all(|character| character == ' ')
        && index > 0
        && index < clusters.len() as i32 - 1
        && spacing_edges_at(edges, index as usize - 1).trailing == EastAsianSpacingValue::Narrow
        && !clusters[index as usize - 1]
            .text
            .chars()
            .all(|character| character == ' ')
        && spacing_edges_at(edges, index as usize + 1).leading == EastAsianSpacingValue::Narrow
        && !clusters[index as usize + 1]
            .text
            .chars()
            .all(|character| character == ' ')
}

fn is_wide_narrow_typed_space(
    clusters: &[Cluster],
    index: i32,
    edges: &[EastAsianSpacingEdges],
) -> bool {
    let Some(cluster) = clusters.get(index as usize) else {
        return false;
    };
    !cluster.text.is_empty()
        && cluster.text.chars().all(|character| character == ' ')
        && index > 0
        && index < clusters.len() as i32 - 1
        && is_wide_narrow_pair(
            spacing_edges_at(edges, index as usize - 1).trailing,
            spacing_edges_at(edges, index as usize + 1).leading,
        )
}

fn is_wide_narrow_boundary(left: i32, right: i32, edges: &[EastAsianSpacingEdges]) -> bool {
    is_wide_narrow_pair(
        spacing_edges_at(edges, left as usize).trailing,
        spacing_edges_at(edges, right as usize).leading,
    )
}
fn spacing_edges_at(edges: &[EastAsianSpacingEdges], index: usize) -> EastAsianSpacingEdges {
    edges.get(index).copied().unwrap_or_else(|| {
        log::warn!("missing East Asian spacing edge; using neutral edge");
        MISSING_EAST_ASIAN_SPACING_EDGES
    })
}
fn role_at(roles: &[FontRole], index: usize) -> FontRole {
    roles.get(index).copied().unwrap_or_else(|| {
        log::warn!("missing cluster role; using unknown role");
        FontRole::Unknown
    })
}
fn is_wide_narrow_pair(left: EastAsianSpacingValue, right: EastAsianSpacingValue) -> bool {
    (left == EastAsianSpacingValue::Wide && right == EastAsianSpacingValue::Narrow)
        || (left == EastAsianSpacingValue::Narrow && right == EastAsianSpacingValue::Wide)
}
fn is_cjk_like(role: FontRole) -> bool {
    role == FontRole::CjkText || role == FontRole::CjkPunctuation
}
fn preferred_glue_kind(kind: InlineObjectPreferredStretchKind) -> GlueKind {
    match kind {
        InlineObjectPreferredStretchKind::PunctuationTrailing => {
            GlueKind::InlineObjectPunctuationTrailing
        }
        InlineObjectPreferredStretchKind::Relation => GlueKind::InlineObjectRelation,
        InlineObjectPreferredStretchKind::BinaryOperator => GlueKind::InlineObjectBinaryOperator,
    }
}
fn preferred_reason(kind: InlineObjectPreferredStretchKind) -> &'static str {
    match kind {
        InlineObjectPreferredStretchKind::PunctuationTrailing => "InlineObjectPunctuationTrailing",
        InlineObjectPreferredStretchKind::Relation => "InlineObjectRelation",
        InlineObjectPreferredStretchKind::BinaryOperator => "InlineObjectBinaryOperator",
    }
}
