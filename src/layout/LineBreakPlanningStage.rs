// 对应 Kotlin 源文件：engine/src/commonMain/kotlin/org/tiqian/layout/LineBreakPlanningStage.kt

use crate::common::{HashMap, HashSet};
use std::sync::Arc;

use super::super::clreq::ClreqProfile::{
    AdjustmentStylePolicy, ClreqProfile, ClreqPunctuationGlyphSubstitutor, PunctuationClass,
    ResolvedKinsoku,
};
use super::super::clreq::NumberSymbolCohesion::number_symbol_cohesion;
use super::super::core::EastAsianSpacing::EastAsianSpacingEdges;
use super::super::core::Geometry::TextRange;
use super::super::core::IntRange::IntRange;
use super::super::core::LayoutModel::{
    AutoSpaceDecisionInfo, BreakOpportunityDecisionInfo, Cluster,
    EmergencyTrackingEligibilityDecisionInfo, Glyph, MandatoryBreakDecisionInfo, RoleOverrideInfo,
    ShapingDecisionInfo, ZeroWidthBreakDecisionInfo,
};
use super::super::core::Text::Text;
use super::super::core::TextModel::{
    DecorationKind, InlineAttachment, InlineObjectPreferredStretch, InlineObjectSpan, LayoutInput,
    LineBreakPolicy, RubySpan, TextStyle,
};
use super::super::core::Units::Ic;
use super::super::font::FontMetrics::{
    FontMetricsNormalizationInput, FontMetricsNormalizer, FontMetricsRequest, FontMetricsResolver,
};
use super::super::font::FontPolicy::{FontDecision, FontRole};
use super::AnnotationGeometryStage::RubyFontGeometry;
use super::KinsokuRule::ClreqKinsokuRule;
use super::KinsokuRule::KinsokuRule;
use super::LineBreaker::{LineBreaker, LineBreakerConfig};
use super::LineGeometryStage::{ClusterMetricDecision, ResolvedLineMetrics, line_metrics};
use super::ProgressiveBreakDecisions::{
    ProgressiveBreakOpportunity, ProgressiveBreakTier, ShrinkOpportunity,
};
use super::PunctuationGeometryLedger::{
    AttachedInlinePunctuationBoundaryResult, PunctuationGeometryLedger,
};
use super::PunctuationGeometryStage::{
    ContextualKinsoku, InlineBoxApplicationResult, InlineObjectAttachedMark,
    attached_ascii_point_mark_kinsoku, inline_object_attached_kinsoku,
    is_east_asian_spacing_boundary_at,
};
use super::PunctuationModel::{PunctuationAtom, PunctuationSpacingCompressionResult};
use super::QuotePairAnalyzer::QuotePair;
use super::UnicodePunctuationBoundaryResolver::{
    UnicodePunctuationBoundaries, resolve_attached_inline_inter_char_boundaries,
    resolve_attached_inline_virtual_boundaries, resolve_unicode_punctuation_boundaries,
    resolve_western_bracket_cjk_inter_char_boundaries,
};
use super::WidthIndependentAnnotationCache::containing_items;

/// cluster preparation 产生、line planning 与 finish stage 原样消费的 paragraph 级共享状态。
pub struct ParagraphLayoutPrep {
    pub input: LayoutInput,
    pub rejected_technical_tiers_by_span: HashMap<TextRange, HashSet<ProgressiveBreakTier>>,
    pub text: Text,
    pub font_size: f32,
    pub style_at: Arc<dyn Fn(i32) -> TextStyle + Send + Sync>,
    pub font_size_at: Arc<dyn Fn(i32) -> f32 + Send + Sync>,
    pub bopomofo_font_weight_at: Arc<dyn Fn(i32) -> i32 + Send + Sync>,
    pub ruby_font_size: f32,
    pub ruby_stack_gap: f32,
    pub ruby_font_weight: i32,
    pub pinyin_spans: Vec<RubySpan>,
    pub clreq_profile: ClreqProfile,
    pub punctuation_glyph_substitutor: ClreqPunctuationGlyphSubstitutor,
    pub measure: f32,
    pub measure_em: f32,
    pub grid_body_offset: f32,
    pub line_length_grid_decision: super::super::core::LayoutModel::LineLengthGridDecisionInfo,
    pub quote_pairs: Vec<QuotePair>,
    pub role_override_infos: Vec<RoleOverrideInfo>,
    pub font_decisions: Vec<FontDecision>,
    pub hyphen_offsets: HashSet<i32>,
    pub hyphen_advance: f32,
    pub hyphen_glyphs: Vec<Glyph>,
    pub substitution_rollbacks: HashMap<TextRange, String>,
    pub break_opportunity_decisions: Vec<BreakOpportunityDecisionInfo>,
    pub emergency_tracking_eligibility_decisions: Vec<EmergencyTrackingEligibilityDecisionInfo>,
    pub progressive_break_offsets: HashMap<i32, ProgressiveBreakOpportunity>,
    pub shaped_glyphs_by_cluster_range: HashMap<TextRange, Vec<Glyph>>,
    pub open_type_features_by_cluster_range: HashMap<TextRange, Vec<String>>,
    pub shaping_decisions: Vec<ShapingDecisionInfo>,
    pub east_asian_spacing_edges: Vec<EastAsianSpacingEdges>,
    pub auto_space_decisions: Vec<AutoSpaceDecisionInfo>,
    pub inline_box_result: InlineBoxApplicationResult,
    pub natural_clusters: Vec<Cluster>,
    pub inline_object_by_cluster_index: HashMap<i32, InlineObjectSpan>,
    pub uniform_inline_object_boundary_after_clusters: HashSet<i32>,
    pub preferred_inline_object_boundary_after_clusters: HashMap<i32, InlineObjectPreferredStretch>,
    pub inline_object_boundary_unbreakable_ranges: Vec<super::super::core::IntRange::IntRange>,
    pub cluster_roles: Vec<FontRole>,
    pub resolved_kinsoku: ResolvedKinsoku,
    pub kinsoku_rule: ClreqKinsokuRule,
    pub inline_object_attached_marks: Vec<InlineObjectAttachedMark>,
    pub inline_object_separator_space_trims: HashMap<i32, f32>,
    pub inline_object_attachment_no_stretch_boundaries: HashSet<i32>,
    pub inline_object_punctuation_attachment_decisions:
        Vec<super::super::core::LayoutModel::InlineObjectPunctuationAttachmentDecisionInfo>,
    pub mandatory_break_clusters: HashSet<i32>,
    pub zero_width_break_clusters: HashSet<i32>,
    pub mandatory_break_decisions: Vec<MandatoryBreakDecisionInfo>,
    pub zero_width_break_decisions: Vec<ZeroWidthBreakDecisionInfo>,
    pub punctuation_atoms: Vec<PunctuationAtom>,
    pub spacing_plan: PunctuationSpacingCompressionResult,
    pub ruby_font_geometry_by_span: HashMap<RubySpan, RubyFontGeometry>,
    pub ruby_and_bopomofo_spread: HashMap<i32, f32>,
    pub natural_inline_attachments: Vec<InlineAttachment>,
    pub attached_punctuation_boundary: AttachedInlinePunctuationBoundaryResult,
    pub base_geometry: PunctuationGeometryLedger,
    pub attached_punctuation_trailing_glue_by_cluster: HashMap<i32, f32>,
    pub clusters: Vec<Cluster>,
    pub adjustment_style: AdjustmentStylePolicy,
    pub atom_class_by_range: HashMap<TextRange, PunctuationClass>,
    pub shrink_opportunities: Vec<ShrinkOpportunity>,
}

#[derive(Clone, Debug)]
pub struct LineBreakPlanningStageResult {
    pub metric_decisions: Vec<ClusterMetricDecision>,
    pub metric_decision_by_range: HashMap<TextRange, ClusterMetricDecision>,
    pub base_ascent: f32,
    pub base_descent: f32,
    pub base_box_descent: f32,
    pub base_face_height: f32,
    pub existing_interline_space: f32,
    pub ruby_extent: f32,
    pub base_line_metrics: ResolvedLineMetrics,
    pub line_spacing_decision: Option<super::super::core::LayoutModel::LineSpacingDecisionInfo>,
    pub block_indent: f32,
    pub first_line_indent: f32,
    pub first_line_indent_decision: super::super::core::LayoutModel::FirstLineIndentDecisionInfo,
    pub kinsoku_decision: super::super::core::LayoutModel::KinsokuDecisionInfo,
    pub ascii_point_mark_kinsoku: ContextualKinsoku,
    pub inline_object_kinsoku: ContextualKinsoku,
    pub unicode_punctuation_boundaries: UnicodePunctuationBoundaries,
    pub western_bracket_cjk_inter_char_boundary_after_clusters: HashSet<i32>,
    pub attached_inline_physical_boundary_after_clusters: HashSet<i32>,
    pub attached_inline_virtual_boundary_after_clusters: HashMap<i32, i32>,
    pub attached_inline_virtual_sino_western_boundary_after_clusters: HashSet<i32>,
    pub no_stretch_boundary_clusters: HashSet<i32>,
    pub no_stretch_boundary_after_clusters: HashSet<i32>,
    pub technical_boundary_after_clusters: HashMap<i32, ProgressiveBreakTier>,
    pub emergency_tracking_boundary_after_clusters: HashMap<i32, String>,
    pub progressive_break_opportunities: HashMap<i32, ProgressiveBreakOpportunity>,
    pub line_solution: super::LineOptimization::LineSolution,
}

pub struct LineBreakPlanningRequest<'a> {
    pub prep: &'a ParagraphLayoutPrep,
    pub font_metrics_resolver: &'a dyn FontMetricsResolver,
    pub font_metrics_normalizer: &'a dyn FontMetricsNormalizer,
    pub justifier: &'a super::Justifier::Justifier,
    pub line_breaker: &'a dyn LineBreaker,
}

impl<'a> LineBreakPlanningRequest<'a> {
    pub fn new(
        prep: &'a ParagraphLayoutPrep,
        font_metrics_resolver: &'a dyn FontMetricsResolver,
        font_metrics_normalizer: &'a dyn FontMetricsNormalizer,
        justifier: &'a super::Justifier::Justifier,
        line_breaker: &'a dyn LineBreaker,
    ) -> Self {
        Self {
            prep,
            font_metrics_resolver,
            font_metrics_normalizer,
            justifier,
            line_breaker,
        }
    }
}

pub fn plan_paragraph_lines(request: LineBreakPlanningRequest<'_>) -> LineBreakPlanningStageResult {
    let prep = request.prep;
    let mut metric_cluster_index = 0usize;
    let metric_decisions: Vec<_> = prep
        .font_decisions
        .iter()
        .map(|decision| {
            while metric_cluster_index < prep.natural_clusters.len()
                && prep.natural_clusters[metric_cluster_index].range.end() <= decision.range.start()
            {
                metric_cluster_index += 1;
            }
            let mut displayed_face_selection_text = String::new();
            while metric_cluster_index < prep.natural_clusters.len()
                && prep.natural_clusters[metric_cluster_index].range.start() < decision.range.end()
            {
                let cluster = &prep.natural_clusters[metric_cluster_index];
                assert!(
                    cluster.range.start() >= decision.range.start()
                        && cluster.range.end() <= decision.range.end(),
                    "Shaped cluster {:?} crosses font decision {:?}",
                    cluster.range,
                    decision.range
                );
                displayed_face_selection_text.push_str(&cluster.display_text);
                metric_cluster_index += 1;
            }
            if displayed_face_selection_text.is_empty() {
                displayed_face_selection_text = prep.text.slice(decision.range).to_owned();
            }
            let style = (prep.style_at)(decision.range.start());
            let metric_request = FontMetricsRequest::builder(
                decision.candidate.key.clone(),
                (prep.font_size_at)(decision.range.start()),
                decision.role,
                prep.input.text_style.locale.clone(),
            )
            .font_weight(style.font_weight)
            .italic(style.italic)
            .face_selection_text(Text::from(displayed_face_selection_text))
            .font_families(style.font_families)
            .build();
            let raw_metrics = request.font_metrics_resolver.resolve(&metric_request);
            let layout_metrics =
                request
                    .font_metrics_normalizer
                    .normalize(&FontMetricsNormalizationInput {
                        request: metric_request.clone(),
                        raw_metrics,
                    });
            ClusterMetricDecision {
                range: decision.range,
                source_text: prep.text.slice_text(decision.range),
                request: metric_request,
                raw_metrics,
                layout_metrics,
            }
        })
        .collect();
    let base_metrics: Vec<_> = metric_decisions
        .iter()
        .filter(|decision| {
            decision.layout_metrics.metric_box
                == super::super::font::FontMetrics::MetricBox::IdeographicEmBox
        })
        .collect();
    let base_metric_source: Vec<_> = if base_metrics.is_empty() {
        metric_decisions.iter().collect()
    } else {
        base_metrics
    };
    let base_ascent = base_metric_source
        .iter()
        .map(|decision| decision.layout_metrics.ascent)
        .fold(None, |maximum: Option<f32>, value| {
            Some(maximum.map_or(value, |current| current.max(value)))
        })
        .unwrap_or(prep.font_size * CJK_FACE_ASCENT_FALLBACK_EM);
    let base_descent = base_metric_source
        .iter()
        .map(|decision| decision.layout_metrics.descent)
        .fold(None, |maximum: Option<f32>, value| {
            Some(maximum.map_or(value, |current| current.max(value)))
        })
        .unwrap_or(prep.font_size * CJK_FACE_DESCENT_FALLBACK_EM);
    let base_box_descent = metric_decisions
        .iter()
        .find(|decision| {
            decision.layout_metrics.metric_box
                == super::super::font::FontMetrics::MetricBox::IdeographicEmBox
                && decision.request.font_size == prep.font_size
        })
        .map(|decision| decision.layout_metrics.descent)
        .unwrap_or(base_descent);
    let ruby_extent = prep
        .ruby_font_geometry_by_span
        .values()
        .map(|geometry| geometry.required_extent)
        .fold(0.0, f32::max);
    let interlinear_spacing_floor = if prep.input.decorations.is_empty() {
        0.0
    } else {
        0.5 * prep.font_size
    };
    let default_body_line_height = prep.font_size * DEFAULT_BODY_LINE_HEIGHT_EM;
    let base_line_metrics = line_metrics(
        &metric_decisions,
        prep.input.paragraph_style.line_height,
        default_body_line_height,
        interlinear_spacing_floor,
    );
    let metric_decision_by_range = prep
        .natural_clusters
        .iter()
        .zip(containing_items(
            &prep.natural_clusters,
            &metric_decisions,
            |decision| decision.range,
        ))
        .filter_map(|(cluster, decision)| {
            decision.map(|index| (cluster.range, metric_decisions[index].clone()))
        })
        .collect();
    let base_face_height = base_ascent + base_descent;
    let existing_interline_space = (base_line_metrics.height - base_face_height).max(0.0);
    let line_spacing_decision = (base_line_metrics.height > 0.0).then(|| {
        let natural = base_line_metrics.height - base_line_metrics.extra_leading;
        let requested = prep.input.paragraph_style.line_height;
        let mark_floor_binds = interlinear_spacing_floor > 0.0
            && natural + interlinear_spacing_floor
                > requested.unwrap_or(default_body_line_height) + 0.001;
        super::super::core::LayoutModel::LineSpacingDecisionInfo {
            natural_height: natural,
            requested_line_height: requested,
            resolved_height: base_line_metrics.height,
            spacing_floor: interlinear_spacing_floor,
            floor_applied: mark_floor_binds,
            reason: if requested.is_some() && !mark_floor_binds {
                "ExplicitLineHeight".to_owned()
            } else if mark_floor_binds {
                "InterlinearMarkLineSpacingFloor".to_owned()
            } else {
                "CjkBodyLineHeightDefault".to_owned()
            },
        }
    });

    let explicit_indent_em = prep
        .input
        .paragraph_style
        .first_line_indent
        .map(|indent: Ic| indent.count);
    let indent_policy = prep.input.paragraph_style.first_line_indent_policy;
    let block_indent = prep
        .input
        .paragraph_style
        .block_indent
        .to_px(prep.font_size);
    let resolved_indent_em =
        explicit_indent_em.unwrap_or_else(|| indent_policy.resolve_em(prep.measure_em));
    let first_line_indent = (block_indent + resolved_indent_em * prep.font_size).max(0.0);
    let first_line_indent_decision = super::super::core::LayoutModel::FirstLineIndentDecisionInfo {
        source: if explicit_indent_em.is_some() {
            "Explicit".to_owned()
        } else {
            "MeasureAdaptiveFirstLineIndent".to_owned()
        },
        measure_em: prep.measure_em,
        threshold_em: indent_policy.short_below_em,
        resolved_em: resolved_indent_em,
    };
    let kinsoku_decision = super::super::core::LayoutModel::KinsokuDecisionInfo {
        measure_em: prep.measure_em,
        level: format!("{:?}", prep.resolved_kinsoku.level),
        hanging: format!("{:?}", prep.resolved_kinsoku.hanging),
        reason: prep.resolved_kinsoku.reason.clone(),
    };
    let hangable_clusters: HashSet<_> = match prep.resolved_kinsoku.hanging {
        super::super::clreq::ClreqProfile::HangingPunctuationStyle::Disabled => HashSet::new(),
        super::super::clreq::ClreqProfile::HangingPunctuationStyle::PauseStops => prep
            .natural_clusters
            .iter()
            .enumerate()
            .filter_map(|(index, cluster)| {
                (cluster.display_text.chars().count() == 1
                    && cluster
                        .display_text
                        .chars()
                        .next()
                        .is_some_and(|character| HANGABLE_PUNCTUATION.contains(&character)))
                .then_some(index as i32)
            })
            .collect(),
    };
    let ascii_point_mark_kinsoku = attached_ascii_point_mark_kinsoku(
        &prep.natural_clusters,
        &prep.cluster_roles,
        &prep.clusters,
        prep.resolved_kinsoku.level,
        prep.measure - block_indent,
        prep.measure - first_line_indent,
    );
    let inline_object_kinsoku = inline_object_attached_kinsoku(
        &prep.natural_clusters,
        &prep.inline_object_attached_marks,
        &prep.clusters,
        prep.resolved_kinsoku.level,
        prep.measure - block_indent,
        prep.measure - first_line_indent,
    );
    let mut resolved_hangable_clusters = hangable_clusters;
    resolved_hangable_clusters
        .extend(&ascii_point_mark_kinsoku.impossible_measure_hang_eligible_clusters);
    resolved_hangable_clusters
        .extend(&inline_object_kinsoku.impossible_measure_hang_eligible_clusters);
    let unicode_punctuation_boundaries = resolve_unicode_punctuation_boundaries(
        &prep.text,
        &prep.natural_clusters,
        &prep.cluster_roles,
        &prep.quote_pairs,
    );
    let western_bracket_boundaries = resolve_western_bracket_cjk_inter_char_boundaries(
        &prep.text,
        &prep.natural_clusters,
        &prep.cluster_roles,
    );
    let attached_inline = resolve_attached_inline_inter_char_boundaries(
        &prep.text,
        &prep.natural_clusters,
        &prep.cluster_roles,
        &prep.east_asian_spacing_edges,
        &western_bracket_boundaries,
        &prep.natural_inline_attachments,
    );
    let attached_inline_forbidden_line_start_clusters: HashSet<_> = prep
        .natural_inline_attachments
        .iter()
        .enumerate()
        .filter_map(|(index, attachment)| {
            (*attachment == InlineAttachment::Previous).then_some(index as i32)
        })
        .collect();
    let forbidden_line_start_clusters: HashSet<_> = prep
        .natural_clusters
        .iter()
        .enumerate()
        .filter_map(|(index, cluster)| {
            let index = index as i32;
            (attached_inline_forbidden_line_start_clusters.contains(&index)
                || prep.zero_width_break_clusters.contains(&index)
                || (prep.cluster_roles.get(index as usize) == Some(&FontRole::CjkPunctuation)
                    && prep.kinsoku_rule.forbidden_at_line_start(cluster))
                || unicode_punctuation_boundaries
                    .forbidden_line_start_clusters
                    .contains(&index)
                || ascii_point_mark_kinsoku
                    .forbidden_line_start_clusters
                    .contains(&index)
                || inline_object_kinsoku
                    .forbidden_line_start_clusters
                    .contains(&index))
            .then_some(index)
        })
        .collect();
    let forbidden_line_end_clusters: HashSet<_> = prep
        .natural_clusters
        .iter()
        .enumerate()
        .filter_map(|(index, cluster)| {
            let index = index as i32;
            ((prep.cluster_roles.get(index as usize) == Some(&FontRole::CjkPunctuation)
                && prep.kinsoku_rule.forbidden_at_line_end(cluster))
                || unicode_punctuation_boundaries
                    .forbidden_line_end_clusters
                    .contains(&index))
            .then_some(index)
        })
        .collect();
    let hyphen_break_clusters: HashSet<_> = if prep.hyphen_offsets.is_empty() {
        HashSet::new()
    } else {
        prep.natural_clusters
            .iter()
            .enumerate()
            .filter_map(|(index, cluster)| {
                prep.hyphen_offsets
                    .contains(&cluster.range.start())
                    .then_some(index as i32)
            })
            .collect()
    };
    let cluster_index_by_source_start: HashMap<_, _> = prep
        .natural_clusters
        .iter()
        .enumerate()
        .map(|(index, cluster)| (cluster.range.start(), index as i32))
        .collect();
    let technical_whitespace_capacity = request
        .justifier
        .progressive_technical_whitespace_stretch_capacity(prep.font_size);
    let progressive_break_opportunities: HashMap<_, _> = prep
        .progressive_break_offsets
        .iter()
        .filter_map(|(source_offset, opportunity)| {
            cluster_index_by_source_start
                .get(source_offset)
                .map(|index| {
                    (
                        *index,
                        if opportunity.tier == ProgressiveBreakTier::Whitespace {
                            ProgressiveBreakOpportunity::with_preceding_whitespace_stretch_capacity(
                                opportunity.tier,
                                opportunity.span_range,
                                technical_whitespace_capacity,
                            )
                        } else {
                            *opportunity
                        },
                    )
                })
        })
        .collect();
    let progressive_technical_ranges: Vec<_> = prep
        .input
        .content
        .line_break_spans
        .iter()
        .filter(|span| span.policy == LineBreakPolicy::ProgressiveTechnical)
        .map(|span| span.range)
        .collect();
    let number_symbol_cluster_ranges: Vec<_> =
        number_symbol_cohesion::unbreakable_ranges(&prep.text)
            .into_iter()
            .filter(|source_range| {
                !progressive_technical_ranges.iter().any(|technical_range| {
                    source_range.first() < technical_range.end()
                        && source_range.last() + 1 > technical_range.start()
                })
            })
            .filter_map(|source_range| {
                cluster_index_range_for_source_range(
                    &prep.natural_clusters,
                    TextRange::new(source_range.first(), source_range.last() + 1),
                )
            })
            .collect();
    let number_symbol_unbreakable_ranges: Vec<_> = number_symbol_cluster_ranges
        .iter()
        .copied()
        .filter(|range| {
            range
                .into_iter()
                .map(|index| prep.natural_clusters[index as usize].advance)
                .sum::<f32>()
                <= prep.measure
        })
        .collect();
    let no_stretch_boundary_clusters: HashSet<_> = prep
        .natural_clusters
        .iter()
        .enumerate()
        .filter_map(|(index, cluster)| {
            matches!(
                prep.atom_class_by_range.get(&cluster.range),
                Some(
                    PunctuationClass::Connector
                        | PunctuationClass::Solidus
                        | PunctuationClass::Dash
                        | PunctuationClass::Ellipsis
                )
            )
            .then_some(index as i32)
        })
        .collect();
    let mut no_stretch_boundary_after_clusters: HashSet<i32> = number_symbol_cluster_ranges
        .iter()
        .flat_map(|range| range.first()..range.last())
        .collect();
    no_stretch_boundary_after_clusters.extend(&prep.inline_object_attachment_no_stretch_boundaries);
    let technical_boundary_after_clusters: HashMap<_, _> = progressive_break_opportunities
        .iter()
        .filter_map(|(right, opportunity)| {
            (opportunity.tier == ProgressiveBreakTier::Whitespace)
                .then_some((right - 1, opportunity.tier))
        })
        .collect();
    let emergency_tracking_boundary_after_clusters: HashMap<_, _> =
        (0..prep.natural_clusters.len().saturating_sub(1))
            .filter_map(|left| {
                let right = left + 1;
                let left_cluster = &prep.natural_clusters[left];
                let right_cluster = &prep.natural_clusters[right];
                if left_cluster.range.end() != right_cluster.range.start()
                    || prep
                        .inline_object_by_cluster_index
                        .contains_key(&(left as i32))
                    || prep
                        .inline_object_by_cluster_index
                        .contains_key(&(right as i32))
                    || prep.zero_width_break_clusters.contains(&(left as i32))
                    || prep.zero_width_break_clusters.contains(&(right as i32))
                    || prep.mandatory_break_clusters.contains(&(left as i32))
                    || prep.mandatory_break_clusters.contains(&(right as i32))
                    || left_cluster.text.is_empty()
                    || right_cluster.text.is_empty()
                    || left_cluster.text.chars().all(char::is_whitespace)
                    || right_cluster.text.chars().all(char::is_whitespace)
                {
                    return None;
                }
                prep.emergency_tracking_eligibility_decisions
                    .iter()
                    .find(|decision| {
                        left_cluster.range.start() >= decision.range.start()
                            && right_cluster.range.end() <= decision.range.end()
                    })
                    .map(|decision| (left as i32, decision.reason.clone()))
            })
            .collect();
    let adjustable_inline_boundary_right_clusters: HashSet<_> = prep
        .uniform_inline_object_boundary_after_clusters
        .iter()
        .filter_map(|left| {
            let right = left + 1;
            (!no_stretch_boundary_after_clusters.contains(left)
                && !no_stretch_boundary_clusters.contains(left)
                && !no_stretch_boundary_clusters.contains(&right))
            .then_some(right)
        })
        .collect();
    let mut cjk_inter_char_boundaries: HashSet<_> = (1..prep.natural_clusters.len())
        .filter_map(|right| {
            (!attached_inline
                .suppressed_physical_boundary_after_clusters
                .contains(&(right as i32 - 1))
                && !no_stretch_boundary_after_clusters.contains(&(right as i32 - 1))
                && prep.cluster_roles.get(right - 1) == Some(&FontRole::CjkText)
                && prep.cluster_roles.get(right) == Some(&FontRole::CjkText))
            .then_some(right as i32)
        })
        .collect();
    cjk_inter_char_boundaries.extend(adjustable_inline_boundary_right_clusters);
    cjk_inter_char_boundaries.extend(
        attached_inline
            .ordinary_western_boundary_after_clusters
            .iter()
            .map(|left| left + 1),
    );
    cjk_inter_char_boundaries.extend(
        attached_inline
            .virtual_boundary_after_clusters
            .keys()
            .map(|left| left + 1),
    );
    let mut sino_western_boundaries: HashSet<_> = (1..prep.natural_clusters.len())
        .filter_map(|right| {
            (!attached_inline
                .suppressed_physical_boundary_after_clusters
                .contains(&(right as i32 - 1))
                && !no_stretch_boundary_after_clusters.contains(&(right as i32 - 1))
                && is_east_asian_spacing_boundary_at(
                    right,
                    &prep.natural_clusters,
                    &prep.east_asian_spacing_edges,
                ))
            .then_some(right as i32)
        })
        .collect();
    sino_western_boundaries.extend(
        attached_inline
            .virtual_sino_western_boundary_after_clusters
            .iter()
            .map(|left| left + 1),
    );
    let attached_inline_unbreakable_ranges: Vec<_> =
        resolve_attached_inline_virtual_boundaries(&prep.natural_inline_attachments)
            .into_iter()
            .map(|boundary| {
                IntRange::new(
                    boundary.previous_cluster_index,
                    boundary.attached_cluster_range.1,
                )
            })
            .collect();
    let mut unbreakable_ranges: Vec<_> = prep
        .input
        .decorations
        .iter()
        .filter(|decoration| decoration.kind == DecorationKind::Mourning)
        .filter_map(|decoration| {
            cluster_index_range_for_source_range(&prep.natural_clusters, decoration.range)
        })
        .collect();
    unbreakable_ranges.extend(prep.pinyin_spans.iter().filter_map(|ruby| {
        cluster_index_range_for_source_range(&prep.natural_clusters, ruby.base_range)
    }));
    unbreakable_ranges.extend(attached_inline_unbreakable_ranges);
    unbreakable_ranges.extend(number_symbol_unbreakable_ranges);
    unbreakable_ranges.extend(
        unicode_punctuation_boundaries
            .unbreakable_ranges
            .iter()
            .map(|&(first, last)| IntRange::new(first, last)),
    );
    unbreakable_ranges.extend(
        ascii_point_mark_kinsoku
            .unbreakable_ranges
            .iter()
            .map(|&(first, last)| IntRange::new(first, last)),
    );
    unbreakable_ranges.extend(
        inline_object_kinsoku
            .unbreakable_ranges
            .iter()
            .map(|&(first, last)| IntRange::new(first, last)),
    );
    unbreakable_ranges.extend(
        prep.inline_object_boundary_unbreakable_ranges
            .iter()
            .copied(),
    );
    let line_solution = if prep.text.is_empty() {
        super::LineOptimization::LineSolution::new(Vec::new())
    } else {
        let mut config = LineBreakerConfig::default();
        config.shrink_opportunities = prep.shrink_opportunities.clone();
        config.unbreakable_ranges = unbreakable_ranges;
        config.first_line_indent = first_line_indent - block_indent;
        config.hangable_clusters = resolved_hangable_clusters;
        config.extendable_hang_ranges = ascii_point_mark_kinsoku
            .extendable_hang_ranges
            .iter()
            .chain(&inline_object_kinsoku.extendable_hang_ranges)
            .map(|&(first, last)| IntRange::new(first, last))
            .collect();
        config.forbidden_line_start_clusters = Some(forbidden_line_start_clusters);
        config.forbidden_line_end_clusters = forbidden_line_end_clusters;
        config.hyphen_break_clusters = hyphen_break_clusters;
        config.cjk_inter_char_boundaries = cjk_inter_char_boundaries;
        config.max_cjk_stretch_per_gap = HYPHEN_LAST_RESORT_CJK_STRETCH_EM * prep.font_size;
        config.sino_western_boundaries = sino_western_boundaries;
        config.sino_western_stretch_cap = HYPHEN_SINO_WESTERN_STRETCH_CAP_EM * prep.font_size;
        config.line_adjustment_push_in = prep.adjustment_style.line_adjustment
            != super::super::clreq::ClreqProfile::LineAdjustmentStrategy::PushOutOnly;
        config.line_adjustment_compress_bias = match prep.adjustment_style.line_adjustment {
            super::super::clreq::ClreqProfile::LineAdjustmentStrategy::PushInFirst => 1_000_000.0,
            super::super::clreq::ClreqProfile::LineAdjustmentStrategy::PushOutFirst => 0.5,
            super::super::clreq::ClreqProfile::LineAdjustmentStrategy::PushOutOnly => 0.0,
        };
        config.hard_break_after_clusters = prep.mandatory_break_clusters.clone();
        config.non_rendering_control_clusters = prep.zero_width_break_clusters.clone();
        config.progressive_break_opportunities = progressive_break_opportunities.clone();
        request.line_breaker.break_lines(
            &prep.natural_clusters,
            &prep.clusters,
            prep.measure - block_indent,
            &config,
        )
    };
    LineBreakPlanningStageResult {
        metric_decisions,
        metric_decision_by_range,
        base_ascent,
        base_descent,
        base_box_descent,
        base_face_height,
        existing_interline_space,
        ruby_extent,
        base_line_metrics,
        line_spacing_decision,
        block_indent,
        first_line_indent,
        first_line_indent_decision,
        kinsoku_decision,
        ascii_point_mark_kinsoku,
        inline_object_kinsoku,
        unicode_punctuation_boundaries,
        western_bracket_cjk_inter_char_boundary_after_clusters: attached_inline
            .ordinary_western_boundary_after_clusters,
        attached_inline_physical_boundary_after_clusters: attached_inline
            .suppressed_physical_boundary_after_clusters,
        attached_inline_virtual_boundary_after_clusters: attached_inline
            .virtual_boundary_after_clusters,
        attached_inline_virtual_sino_western_boundary_after_clusters: attached_inline
            .virtual_sino_western_boundary_after_clusters,
        no_stretch_boundary_clusters,
        no_stretch_boundary_after_clusters,
        technical_boundary_after_clusters,
        emergency_tracking_boundary_after_clusters,
        progressive_break_opportunities,
        line_solution,
    }
}

fn cluster_index_range_for_source_range(
    clusters: &[Cluster],
    source_range: TextRange,
) -> Option<IntRange> {
    let mut first = None;
    let mut last = 0;
    for (index, cluster) in clusters.iter().enumerate() {
        if cluster.range.start() >= source_range.start()
            && cluster.range.end() <= source_range.end()
        {
            first.get_or_insert(index as i32);
            last = index as i32;
        }
    }
    first.map(|first| IntRange::new(first, last))
}

const CJK_FACE_ASCENT_FALLBACK_EM: f32 = 0.88;
pub const CJK_FACE_DESCENT_FALLBACK_EM: f32 = 0.12;
const DEFAULT_BODY_LINE_HEIGHT_EM: f32 = 1.5;
const HYPHEN_LAST_RESORT_CJK_STRETCH_EM: f32 = 0.5;
const HYPHEN_SINO_WESTERN_STRETCH_CAP_EM: f32 = 0.25;
pub const HANGABLE_PUNCTUATION: [char; 3] = ['、', '，', '。'];
