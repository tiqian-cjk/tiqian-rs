// 对应 Kotlin 源文件：engine/src/commonMain/kotlin/org/tiqian/layout/WidthIndependentAnnotationCache.kt

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

use super::super::clreq::ClreqProfile::ClreqProfileResolver;
use super::super::clreq::ClreqProfile::{
    ClreqProfile, ClreqPunctuationGlyphSubstitutor, PunctuationClass,
};
use super::super::core::EastAsianSpacing::{
    EastAsianSpacingEdges, EastAsianSpacingValue, unicode_east_asian_spacing,
};
use super::super::core::Geometry::TextRange;
use super::super::core::IntRange::IntRange;
use super::super::core::LayoutModel::{
    AutoSpaceDecisionInfo, Cluster, InlineObjectPunctuationAttachmentDecisionInfo,
    LineLengthGridDecisionInfo, MandatoryBreakDecisionInfo, RoleOverrideInfo,
    ZeroWidthBreakDecisionInfo,
};
use super::super::core::Text::Text;
use super::super::core::TextModel::{
    DecorationKind, DecorationSpan, InlineAttachment, InlineBoxOuterSpacing, InlineBoxSpan,
    InlineObjectSpan, LastLineAlignment, LayoutInput, LayoutProfileId, LineBreakPolicy,
    LineBreakSpan, RubyKind, RubySpan, TextSpan, TextStyle,
};
use super::super::font::FontMetrics::{FontMetricsRequest, FontMetricsResolver};
use super::super::font::FontPolicy::{
    FallbackResolver, FontDecision, FontRequest, FontRole, FontRoleClassifier, FontRoleContext,
};
use super::super::linebreak::Hyphenation::Hyphenator;
use super::super::shaping::TextShaper::ShapingResult;
use super::super::shaping::TextShaper::{ShapingInput, TextShaper};
use super::AnnotationGeometryStage::RubyFontGeometry;
use super::ClusterRoleResolution::{
    ClusterRoleRangeOptions, ResolvedClusterRange, cluster_role_ranges_with_options,
    require_covered_by,
};
use super::KinsokuRule::ClreqKinsokuRule;
use super::LineBreakPlanningStage::ParagraphLayoutPrep;
use super::ParagraphShapingStage::{
    ParagraphShapingStageResult, is_mandatory_break_cluster, is_zero_width_soft_break_cluster,
    shape_paragraph,
};
use super::ProgressiveBreakDecisions::{ProgressiveBreakTier, ShrinkChannel, ShrinkOpportunity};
use super::PunctuationGeometryLedger::{PunctuationGeometryLedger, cluster_index_range_for};
use super::PunctuationGeometryStage::{
    apply_auto_space_policy, apply_inline_box_spans, inline_object_attached_marks,
    is_attached_ascii_point_mark_at, punctuation_atoms,
};
use super::PunctuationModel::{
    PunctuationAtomBuilder, PunctuationSpacingCompressionResult, PunctuationSpacingCompressor,
};
use super::QuotePairAnalyzer::{QuotePair, QuotePairAnalyzer, QuotePairAwareFontRoleClassifier};

/// 宽度无关 annotation cache 的完整输入身份；被拒绝的 technical tier 也属于 key，防止重用错误的 emergency candidate 集。
#[derive(Clone, Debug, PartialEq)]
pub struct WidthIndependentAnnotationKey {
    pub text: Text,
    pub spans: Vec<TextSpan>,
    pub line_break_spans: Vec<LineBreakSpan>,
    pub source_boundaries: HashSet<i32>,
    pub text_style: TextStyle,
    pub decorations: Vec<DecorationSpan>,
    pub ruby_spans: Vec<RubySpan>,
    pub inline_boxes: Vec<InlineBoxSpan>,
    pub inline_objects: Vec<InlineObjectSpan>,
    pub profile_id: LayoutProfileId,
    pub emphasis_dot_gap_em: f32,
    pub rejected_technical_tiers_by_span: HashMap<TextRange, HashSet<ProgressiveBreakTier>>,
}

pub fn to_width_independent_annotation_key(
    input: &LayoutInput,
    rejected: HashMap<TextRange, HashSet<ProgressiveBreakTier>>,
) -> WidthIndependentAnnotationKey {
    WidthIndependentAnnotationKey {
        text: input.content.text.clone(),
        spans: input.content.spans.clone(),
        line_break_spans: input.content.line_break_spans.clone(),
        source_boundaries: input.content.source_boundaries.clone(),
        text_style: input.text_style.clone(),
        decorations: input.decorations.clone(),
        ruby_spans: input.ruby_spans.clone(),
        inline_boxes: input.inline_boxes.clone(),
        inline_objects: input.inline_objects.clone(),
        profile_id: input.profile_id.clone(),
        emphasis_dot_gap_em: input.paragraph_style.emphasis_dot_gap_em,
        rejected_technical_tiers_by_span: rejected,
    }
}

pub struct WidthIndependentParagraphAnnotation {
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
    pub quote_pairs: Vec<QuotePair>,
    pub role_override_infos: Vec<RoleOverrideInfo>,
    pub font_decisions: Vec<FontDecision>,
    pub cluster_ranges: Vec<ResolvedClusterRange>,
    pub font_decision_by_range: HashMap<TextRange, FontDecision>,
    pub inline_object_by_range: HashMap<TextRange, InlineObjectSpan>,
    pub segment_shaping_cache: HashMap<TextRange, ShapingResult>,
    pub substitution_rollbacks: HashMap<TextRange, String>,
    pub ruby_font_geometry_by_span: HashMap<RubySpan, RubyFontGeometry>,
    pub base_shaping_stage: ParagraphShapingStageResult,
}
pub trait WidthIndependentAnnotationCache {
    fn get(
        &mut self,
        key: &WidthIndependentAnnotationKey,
    ) -> Option<Arc<WidthIndependentParagraphAnnotation>>;
    fn put(
        &mut self,
        key: WidthIndependentAnnotationKey,
        annotation: Arc<WidthIndependentParagraphAnnotation>,
    );
    fn clear(&mut self);
    fn size(&self) -> usize;
}
pub struct LruWidthIndependentAnnotationCache {
    max_entries: usize,
    entries: Vec<(
        WidthIndependentAnnotationKey,
        Arc<WidthIndependentParagraphAnnotation>,
    )>,
}
impl Default for LruWidthIndependentAnnotationCache {
    fn default() -> Self {
        Self::new(512)
    }
}
impl LruWidthIndependentAnnotationCache {
    pub fn new(max_entries: usize) -> Self {
        Self {
            max_entries,
            entries: Vec::new(),
        }
    }
}
impl WidthIndependentAnnotationCache for LruWidthIndependentAnnotationCache {
    fn get(
        &mut self,
        key: &WidthIndependentAnnotationKey,
    ) -> Option<Arc<WidthIndependentParagraphAnnotation>> {
        let index = self
            .entries
            .iter()
            .position(|(candidate, _)| candidate == key)?;
        let entry = self.entries.remove(index);
        let value = entry.1.clone();
        self.entries.push(entry);
        Some(value)
    }
    fn put(
        &mut self,
        key: WidthIndependentAnnotationKey,
        annotation: Arc<WidthIndependentParagraphAnnotation>,
    ) {
        if let Some(index) = self
            .entries
            .iter()
            .position(|(candidate, _)| candidate == &key)
        {
            self.entries.remove(index);
        } else if self.entries.len() >= self.max_entries {
            self.entries.remove(0);
        }
        self.entries.push((key, annotation));
    }
    fn clear(&mut self) {
        self.entries.clear()
    }
    fn size(&self) -> usize {
        self.entries.len()
    }
}

/// source-ordered cluster 与 source-ordered item 的单调包含 join。
pub fn containing_items<T>(
    clusters: &[Cluster],
    items: &[T],
    range_of: impl Fn(&T) -> TextRange,
) -> Vec<Option<usize>> {
    let mut item = 0;
    clusters
        .iter()
        .map(|cluster| {
            while item < items.len() && range_of(&items[item]).end() <= cluster.range.start() {
                item += 1;
            }
            (item < items.len() && {
                let range = range_of(&items[item]);
                cluster.range.start() >= range.start() && cluster.range.end() <= range.end()
            })
            .then_some(item)
        })
        .collect()
}
/// cluster 可包含多个 source-ordered item 时的反向单调包含 join。
pub fn first_contained_item<T>(
    clusters: &[Cluster],
    items: &[T],
    range_of: impl Fn(&T) -> TextRange,
) -> Vec<Option<usize>> {
    let mut item = 0;
    clusters
        .iter()
        .map(|cluster| {
            while item < items.len() && range_of(&items[item]).end() <= cluster.range.start() {
                item += 1;
            }
            (item < items.len() && {
                let range = range_of(&items[item]);
                range.start() >= cluster.range.start() && range.end() <= cluster.range.end()
            })
            .then_some(item)
        })
        .collect()
}

/// 解析任何行宽均共享的 paragraph annotation。T6 engine 从自己的字段传入同一组 dependency，避免出现第二份布局策略。
#[allow(clippy::too_many_arguments)]
pub fn prepare_width_independent_annotation(
    input: &LayoutInput,
    rejected: &HashMap<TextRange, HashSet<ProgressiveBreakTier>>,
    clreq_profile_resolver: &dyn ClreqProfileResolver,
    font_role_classifier: &dyn FontRoleClassifier,
    fallback_resolver: &dyn FallbackResolver,
    font_metrics_resolver: &dyn FontMetricsResolver,
    quote_pair_analyzer: &QuotePairAnalyzer,
    text_shaper: &dyn TextShaper,
    hyphenator: &dyn Hyphenator,
) -> WidthIndependentParagraphAnnotation {
    let text = input.content.text.clone();
    let font_size = input.text_style.font_size;
    let inline_object_by_range: HashMap<_, _> = input
        .inline_objects
        .iter()
        .cloned()
        .map(|object| (object.range, object))
        .collect();
    let spans: Vec<_> = input
        .content
        .spans
        .iter()
        .filter(|span| span.range.start() < span.range.end())
        .cloned()
        .collect();
    let style_spans = spans.clone();
    let input_style = input.text_style.clone();
    let style_at: Arc<dyn Fn(i32) -> TextStyle + Send + Sync> = Arc::new(move |offset| {
        style_spans
            .iter()
            .rev()
            .find(|span| offset >= span.range.start() && offset < span.range.end())
            .map(|span| span.style.clone())
            .unwrap_or_else(|| input_style.clone())
    });
    let font_size_at = {
        let style_at = style_at.clone();
        Arc::new(move |offset| style_at(offset).font_size)
    };
    let bopomofo_font_weight_at = {
        let style_at = style_at.clone();
        Arc::new(move |offset| {
            (style_at(offset).font_weight + BOPOMOFO_FONT_WEIGHT_BOOST).clamp(1, 900)
        })
    };
    let emphasis_ranges: Vec<_> = input
        .decorations
        .iter()
        .filter(|decoration| decoration.kind == DecorationKind::Emphasis)
        .map(|decoration| decoration.range)
        .collect();
    let emphasis_italic_at = |offset: i32| {
        emphasis_ranges
            .iter()
            .any(|range| offset >= range.start() && offset < range.end())
    };
    let ruby_font_size = font_size * RUBY_FONT_EM;
    let ruby_stack_gap = font_size * RUBY_STACK_GAP_EM;
    let ruby_font_weight = (input.text_style.font_weight + RUBY_FONT_WEIGHT_BOOST).clamp(1, 900);
    let pinyin_spans: Vec<_> = input
        .ruby_spans
        .iter()
        .filter(|ruby| ruby.kind == RubyKind::Pinyin)
        .cloned()
        .collect();
    let mut boundaries = HashSet::new();
    let length = text.utf16_len();
    let mut add_range = |range: TextRange| {
        for offset in [range.start(), range.end()] {
            if offset > 0 && offset < length {
                boundaries.insert(offset);
            }
        }
    };
    for span in &spans {
        add_range(span.range)
    }
    for decoration in &input.decorations {
        add_range(decoration.range)
    }
    for ruby in &input.ruby_spans {
        add_range(ruby.base_range)
    }
    for inline_box in &input.inline_boxes {
        add_range(inline_box.range)
    }
    for inline_object in &input.inline_objects {
        add_range(inline_object.range)
    }
    for span in &input.content.line_break_spans {
        add_range(span.range)
    }
    for offset in &input.content.source_boundaries {
        if *offset > 0 && *offset < length {
            boundaries.insert(*offset);
        }
    }
    let mut emoji_shaping_boundaries = HashSet::new();
    let mut add_emoji_shaping_range = |range: TextRange| {
        for offset in [range.start(), range.end()] {
            if offset > 0 && offset < length {
                emoji_shaping_boundaries.insert(offset);
            }
        }
    };
    // `EmojiGraphemeShapingAtomicity`: TextSpan carries one ShapingInput style
    // and must split. An inline box is hard only when it changes occupied
    // geometry; decorations, ruby, technical-break annotations, and source
    // interaction boundaries leave complex emoji shaping intact.
    for span in &spans {
        add_emoji_shaping_range(span.range)
    }
    for inline_box in &input.inline_boxes {
        if inline_box.inline_start != 0.
            || inline_box.inline_end != 0.
            || inline_box.outer_spacing == InlineBoxOuterSpacing::Narrow
        {
            add_emoji_shaping_range(inline_box.range)
        }
    }
    for inline_object in &input.inline_objects {
        add_emoji_shaping_range(inline_object.range)
    }
    let profile = clreq_profile_resolver.resolve(&input.profile_id);
    let context = FontRoleContext::new(
        input.text_style.locale.clone(),
        Some(format!("{:?}", profile.region)),
    );
    let substitutor = ClreqPunctuationGlyphSubstitutor::new(profile.punctuation_glyph_policy);
    let pairs = quote_pair_analyzer.analyze(&text);
    let quote_decisions = quote_pair_analyzer.classify_quote_roles(&text, &pairs, &context);
    let overrides: HashMap<_, _> = quote_decisions
        .iter()
        .map(|decision| (decision.index, decision.role))
        .collect();
    let quote_role_override_infos: Vec<_> = quote_decisions
        .iter()
        .map(|decision| {
            let range = TextRange::new(decision.index, decision.index + 1);
            RoleOverrideInfo {
                range,
                source_text: text.slice_text(range),
                original_role: format!(
                    "{:?}",
                    font_role_classifier.classify(&text, range, &context)
                ),
                overridden_role: format!("{:?}", decision.role),
                source: decision.source.clone(),
                reason: decision.reason.clone(),
            }
        })
        .collect();
    let inline_by_start: HashMap<_, _> = input
        .inline_objects
        .iter()
        .cloned()
        .map(|object| (object.range.start(), object))
        .collect();
    let options = ClusterRoleRangeOptions::builder()
        .span_boundaries(boundaries)
        .emoji_shaping_boundaries(emoji_shaping_boundaries)
        .inline_objects_by_start(inline_by_start)
        .build();
    let cluster_ranges = if overrides.is_empty() {
        cluster_role_ranges_with_options(&text, font_role_classifier, &context, &profile, &options)
    } else {
        let aware = QuotePairAwareFontRoleClassifier::new(font_role_classifier, &overrides);
        cluster_role_ranges_with_options(&text, &aware, &context, &profile, &options)
    };
    let mut role_override_infos = quote_role_override_infos;
    role_override_infos.extend(
        cluster_ranges
            .iter()
            .filter_map(|range| range.role_override.clone()),
    );
    role_override_infos.sort_by_key(|info| info.range.start());
    let shapeable: Vec<_> = cluster_ranges
        .iter()
        .filter(|range| {
            !range.mandatory_break
                && !range.zero_width_soft_break
                && !inline_object_by_range.contains_key(&range.range)
        })
        .cloned()
        .collect();
    let font_decisions: Vec<_> = shapeable
        .iter()
        .map(|range| {
            fallback_resolver.resolve(
                &text,
                range.range,
                &FontRequest {
                    preferred_families: input.text_style.font_families.clone(),
                    locale: input.text_style.locale.clone(),
                    role: range.role,
                },
            )
        })
        .collect();
    let font_decision_by_range: HashMap<_, _> = shapeable
        .into_iter()
        .zip(font_decisions.iter().cloned())
        .map(|(range, decision)| (range.range, decision))
        .collect();
    let base = shape_paragraph(
        text_shaper,
        hyphenator,
        input,
        &text,
        font_size,
        f32::INFINITY,
        &cluster_ranges,
        &font_decision_by_range,
        &inline_object_by_range,
        &substitutor,
        &*style_at,
        &emphasis_italic_at,
        rejected,
        &HashMap::new(),
        &HashMap::new(),
    );
    let mut ruby_geometry = HashMap::new();
    for ruby in &pinyin_spans {
        let empty_metric = Text::from("x");
        let metric = if ruby.text.is_empty() {
            &empty_metric
        } else {
            &ruby.text
        };
        let locale = ruby
            .locale
            .clone()
            .unwrap_or_else(|| input.text_style.locale.clone());
        let range = TextRange::new(0, metric.utf16_len());
        let decision = fallback_resolver.resolve(
            metric,
            range,
            &FontRequest {
                preferred_families: ruby.font_families.clone(),
                locale: locale.clone(),
                role: FontRole::LatinText,
            },
        );
        let raw = font_metrics_resolver.resolve(
            &FontMetricsRequest::builder(
                decision.candidate.key.clone(),
                ruby_font_size,
                FontRole::LatinText,
                locale.clone(),
            )
            .font_families(ruby.font_families.clone())
            .font_weight(ruby_font_weight)
            .italic(input.text_style.italic)
            .face_selection_text(metric.clone())
            .build(),
        );
        let shaped = (!ruby.text.is_empty()).then(|| {
            let mut style = input.text_style.clone();
            style.font_size = ruby_font_size;
            style.font_families = ruby.font_families.clone();
            style.font_weight = ruby_font_weight;
            style.locale = locale;
            text_shaper.shape(
                &ShapingInput::builder(
                    ruby.text.clone(),
                    TextRange::new(0, ruby.text.utf16_len()),
                    style,
                    decision,
                )
                .display_text(ruby.text.clone())
                .build(),
            )
        });
        let ascent = raw.typo_ascent.unwrap_or(raw.ascent);
        let descent = raw.typo_descent.unwrap_or(raw.descent);
        ruby_geometry.insert(
            ruby.clone(),
            RubyFontGeometry {
                width: shaped.as_ref().map_or(0., |result| {
                    result.clusters.iter().map(|cluster| cluster.advance).sum()
                }),
                ascent: if ruby.text.is_empty() { 0. } else { ascent },
                descent: if ruby.text.is_empty() { 0. } else { descent },
                required_extent: if ruby.text.is_empty() {
                    0.
                } else {
                    ascent + descent + ruby_stack_gap
                },
                glyphs: shaped
                    .into_iter()
                    .flat_map(|result| result.glyph_runs)
                    .flat_map(|run| run.glyphs)
                    .collect(),
            },
        );
    }
    WidthIndependentParagraphAnnotation {
        text,
        font_size,
        style_at,
        font_size_at,
        bopomofo_font_weight_at,
        ruby_font_size,
        ruby_stack_gap,
        ruby_font_weight,
        pinyin_spans,
        clreq_profile: profile,
        punctuation_glyph_substitutor: substitutor,
        quote_pairs: pairs,
        role_override_infos,
        font_decisions,
        cluster_ranges,
        font_decision_by_range,
        inline_object_by_range,
        segment_shaping_cache: base.segment_shaping_cache.clone(),
        substitution_rollbacks: base.substitution_rollbacks.clone(),
        ruby_font_geometry_by_span: ruby_geometry,
        base_shaping_stage: base,
    }
}

/// 将可缓存的宽度无关 annotation 与本次 measure 绑定，生成 line-planning 所需的完整 preparation。
///
/// Kotlin 的所属文件是 `WidthIndependentAnnotationCache.kt`，因此即使返回类型定义于
/// `LineBreakPlanningStage.rs`，宽度量化、重 shaping、标点和 shrink 资源的装配仍保留在此处。
#[allow(clippy::too_many_arguments)]
pub fn build_paragraph_layout_prep(
    input: &LayoutInput,
    annotation: &WidthIndependentParagraphAnnotation,
    rejected: &HashMap<TextRange, HashSet<ProgressiveBreakTier>>,
    text_shaper: &dyn TextShaper,
    hyphenator: &dyn Hyphenator,
    punctuation_atom_builder: &PunctuationAtomBuilder,
    punctuation_spacing_compressor: &PunctuationSpacingCompressor,
) -> ParagraphLayoutPrep {
    let text = &annotation.text;
    let font_size = input.text_style.font_size;
    let grid = input.paragraph_style.line_length_grid;
    let container_width = input.constraints.max_width();
    let grid_cells = (container_width / font_size).floor() as i32;
    let grid_cells = grid_cells.max(1);
    let measure = if grid.enabled {
        (grid_cells as f32 * font_size).min(container_width)
    } else {
        container_width
    };
    let grid_slack = container_width - measure;
    let alignment = grid
        .body_alignment
        .unwrap_or(input.paragraph_style.last_line_alignment);
    let grid_body_offset = if !grid.enabled {
        0.
    } else {
        match alignment {
            LastLineAlignment::Start => 0.,
            LastLineAlignment::Center => grid_slack / 2.,
            LastLineAlignment::End => grid_slack,
        }
    };
    let line_length_grid_decision = LineLengthGridDecisionInfo {
        enabled: grid.enabled,
        container_width,
        font_size,
        cells: if grid.enabled {
            grid_cells
        } else {
            (measure / font_size) as i32
        },
        measure,
        slack: grid_slack,
        body_alignment: format!("{:?}", alignment),
        body_offset: grid_body_offset,
        reason: if grid.enabled {
            "LineLengthGridQuantization".to_owned()
        } else {
            "GridBypassed".to_owned()
        },
    };
    let measure_em = measure / font_size;
    let resolved_kinsoku = annotation.clreq_profile.kinsoku_mode.resolve(measure_em);
    let kinsoku_rule = ClreqKinsokuRule::new(resolved_kinsoku.level);
    let needs_dynamic = !rejected.is_empty()
        || input
            .content
            .line_break_spans
            .iter()
            .any(|span| span.policy == LineBreakPolicy::ProgressiveTechnical)
        || annotation
            .base_shaping_stage
            .shaping_results
            .iter()
            .any(|result| {
                result
                    .clusters
                    .iter()
                    .map(|cluster| cluster.advance)
                    .sum::<f32>()
                    > measure
            });
    let emphasis = |offset: i32| {
        input.decorations.iter().any(|decoration| {
            decoration.kind == DecorationKind::Emphasis
                && offset >= decoration.range.start()
                && offset < decoration.range.end()
        })
    };
    let shaping_stage = if needs_dynamic {
        shape_paragraph(
            text_shaper,
            hyphenator,
            input,
            text,
            font_size,
            measure,
            &annotation.cluster_ranges,
            &annotation.font_decision_by_range,
            &annotation.inline_object_by_range,
            &annotation.punctuation_glyph_substitutor,
            &*annotation.style_at,
            &emphasis,
            rejected,
            &annotation.segment_shaping_cache,
            &annotation.substitution_rollbacks,
        )
    } else {
        annotation.base_shaping_stage.clone()
    };
    let shaping_results = &shaping_stage.shaping_results;
    let raw_natural: Vec<Cluster> = shaping_results
        .iter()
        .flat_map(|result| result.clusters.clone())
        .collect();
    let mut shaped_glyphs: HashMap<TextRange, Vec<super::super::core::LayoutModel::Glyph>> =
        HashMap::new();
    let mut features = HashMap::new();
    for result in shaping_results {
        for run in &result.glyph_runs {
            for glyph in &run.glyphs {
                shaped_glyphs
                    .entry(glyph.cluster_range)
                    .or_default()
                    .push(glyph.clone());
                let previous = features.insert(glyph.cluster_range, run.open_type_features.clone());
                assert!(
                    previous
                        .as_ref()
                        .is_none_or(|old| old == &run.open_type_features),
                    "Conflicting OpenType features for shaped cluster {:?}",
                    glyph.cluster_range
                );
            }
        }
    }
    require_covered_by(&raw_natural, &annotation.font_decisions);
    let inline_ranges: Vec<TextRange> = input
        .inline_objects
        .iter()
        .map(|object| object.range)
        .collect();
    let narrow_ranges: HashSet<TextRange> = input
        .inline_boxes
        .iter()
        .filter(|span| span.outer_spacing == InlineBoxOuterSpacing::Narrow)
        .map(|span| span.range)
        .collect();
    let narrow_leading: HashSet<i32> = raw_natural
        .iter()
        .enumerate()
        .filter(|(_, cluster)| {
            narrow_ranges
                .iter()
                .any(|range| range.start() == cluster.range.start())
        })
        .map(|(index, _)| index as i32)
        .collect();
    let narrow_trailing: HashSet<i32> = raw_natural
        .iter()
        .enumerate()
        .filter(|(_, cluster)| {
            narrow_ranges
                .iter()
                .any(|range| range.end() == cluster.range.end())
        })
        .map(|(index, _)| index as i32)
        .collect();
    let resolved_edges: Vec<EastAsianSpacingEdges> = raw_natural
        .iter()
        .enumerate()
        .map(|(index, cluster)| {
            if inline_ranges.iter().any(|range| {
                cluster.range.start() >= range.start() && cluster.range.end() <= range.end()
            }) || (is_attached_ascii_point_mark_at(&raw_natural, index)
                && !narrow_leading.contains(&(index as i32)))
            {
                EastAsianSpacingEdges {
                    leading: EastAsianSpacingValue::Other,
                    trailing: EastAsianSpacingValue::Other,
                    contains_wide: false,
                }
            } else {
                let mut edge = unicode_east_asian_spacing::resolved_edges(
                    &cluster.text,
                    &input.text_style.locale,
                );
                if narrow_leading.contains(&(index as i32)) {
                    edge.leading = EastAsianSpacingValue::Narrow
                }
                if narrow_trailing.contains(&(index as i32)) {
                    edge.trailing = EastAsianSpacingValue::Narrow
                }
                edge
            }
        })
        .collect();
    let suppressed = |offset: i32| {
        input
            .content
            .auto_space_suppressed_ranges
            .iter()
            .any(|range| range.start() < offset && offset < range.end())
    };
    let mut verbatim_decisions = Vec::new();
    let east_asian_spacing_edges: Vec<_> = resolved_edges
        .iter()
        .enumerate()
        .map(|(index, edge)| {
            let cluster = &raw_natural[index];
            let lead =
                !narrow_leading.contains(&(index as i32)) && suppressed(cluster.range.start());
            let trail =
                !narrow_trailing.contains(&(index as i32)) && suppressed(cluster.range.end());
            if index > 0
                && suppressed(cluster.range.start())
                && matches!(
                    (resolved_edges[index - 1].trailing, edge.leading),
                    (EastAsianSpacingValue::Wide, EastAsianSpacingValue::Narrow)
                        | (EastAsianSpacingValue::Narrow, EastAsianSpacingValue::Wide)
                )
            {
                verbatim_decisions.push(AutoSpaceDecisionInfo {
                    cluster_range: cluster.range,
                    side: "leading".to_owned(),
                    boundary_role: "EastAsianSpacing.Wide".to_owned(),
                    mode: "Disabled".to_owned(),
                    characters_affected: 0,
                    reduction_per_char: 0.,
                    total_reduction: 0.,
                    reason: "VerbatimRangeAutoSpace:east-asian-spacing-W-N-suppressed".to_owned(),
                });
            }
            let mut result = *edge;
            if lead {
                result.leading = EastAsianSpacingValue::Other
            }
            if trail {
                result.trailing = EastAsianSpacingValue::Other
            }
            result
        })
        .collect();
    let attachments: Vec<InlineAttachment> = raw_natural
        .iter()
        .map(|cluster| (annotation.style_at)(cluster.range.start()).inline_attachment)
        .collect();
    let auto_space = apply_auto_space_policy(
        &raw_natural,
        &east_asian_spacing_edges,
        &attachments,
        annotation.clreq_profile.auto_space,
        font_size,
        &narrow_leading,
        &narrow_trailing,
    );
    let inline_box_result = apply_inline_box_spans(&auto_space.clusters, &input.inline_boxes);
    let natural = inline_box_result.clusters.clone();
    let inline_object_by_cluster_index: HashMap<i32, InlineObjectSpan> = natural
        .iter()
        .enumerate()
        .filter_map(|(index, cluster)| {
            annotation
                .inline_object_by_range
                .get(&cluster.range)
                .cloned()
                .map(|object| (index as i32, object))
        })
        .collect();
    let mut boundary: BTreeMap<i32, super::super::core::TextModel::InlineObjectBoundaryAdjustment> =
        BTreeMap::new();
    for (index, _) in natural.iter().enumerate() {
        let index = index as i32;
        let Some(object) = inline_object_by_cluster_index.get(&index) else {
            continue;
        };
        for (left, adjustment) in [
            ((index > 0).then_some(index - 1), &object.leading_boundary),
            (
                (index < (natural.len() as i32 - 1)).then_some(index),
                &object.trailing_boundary,
            ),
        ] {
            let Some(left) = left else { continue };
            if *adjustment == super::super::core::TextModel::InlineObjectBoundaryAdjustment::FIXED {
                continue;
            }
            if let Some(previous) = boundary.get(&left).cloned() {
                let kinds: HashSet<_> = previous
                    .preferred_stretch
                    .iter()
                    .chain(adjustment.preferred_stretch.iter())
                    .map(|value| value.kind)
                    .collect();
                assert!(
                    kinds.len() <= 1,
                    "Conflicting inline-object stretch classes at cluster boundary {left}"
                );
                let preferred = match (previous.preferred_stretch, adjustment.preferred_stretch) {
                    (Some(a), Some(b)) => Some(if a.capacity() >= b.capacity() { a } else { b }),
                    (Some(a), None) | (None, Some(a)) => Some(a),
                    (None, None) => None,
                };
                boundary.insert(
                    left,
                    super::super::core::TextModel::InlineObjectBoundaryAdjustment::builder()
                        .participates_in_uniform_stretch(
                            previous.participates_in_uniform_stretch
                                || adjustment.participates_in_uniform_stretch,
                        )
                        .shrink_capacity(previous.shrink_capacity.max(adjustment.shrink_capacity))
                        .line_end_discardable_advance(
                            previous
                                .line_end_discardable_advance
                                .max(adjustment.line_end_discardable_advance),
                        )
                        .prevents_line_break(
                            previous.prevents_line_break || adjustment.prevents_line_break,
                        )
                        .build(),
                );
                if let Some(value) = preferred {
                    let existing = boundary.get(&left).cloned().unwrap();
                    boundary.insert(
                        left,
                        super::super::core::TextModel::InlineObjectBoundaryAdjustment::builder()
                            .participates_in_uniform_stretch(
                                existing.participates_in_uniform_stretch,
                            )
                            .preferred_stretch(value)
                            .shrink_capacity(existing.shrink_capacity)
                            .line_end_discardable_advance(existing.line_end_discardable_advance)
                            .prevents_line_break(existing.prevents_line_break)
                            .build(),
                    );
                }
            } else {
                boundary.insert(left, adjustment.clone());
            }
        }
    }
    let uniform: HashSet<i32> = boundary
        .iter()
        .filter(|(_, value)| value.participates_in_uniform_stretch)
        .map(|(index, _)| *index)
        .collect();
    let preferred: HashMap<i32, _> = boundary
        .iter()
        .filter_map(|(index, value)| value.preferred_stretch.map(|stretch| (*index, stretch)))
        .collect();
    let unbreakable: Vec<IntRange> = boundary
        .iter()
        .filter(|(_, value)| value.prevents_line_break)
        .map(|(index, _)| IntRange::new(*index, *index + 1))
        .collect();
    let auto_space_decisions = [auto_space.decisions.clone(), verbatim_decisions].concat();
    let cluster_roles: Vec<FontRole> =
        containing_items(&natural, &annotation.font_decisions, |decision| {
            decision.range
        })
        .into_iter()
        .map(|decision| {
            decision.map_or(FontRole::Unknown, |index| {
                annotation.font_decisions[index].role
            })
        })
        .collect();
    let attached_marks = inline_object_attached_marks(
        &natural,
        &cluster_roles,
        resolved_kinsoku.level,
        &kinsoku_rule,
    );
    let separator_trims: HashMap<i32, f32> = attached_marks
        .iter()
        .flat_map(|attachment| {
            attachment
                .separator_cluster_indices
                .iter()
                .map(|index| (*index, natural[*index as usize].advance))
        })
        .collect();
    let attachment_no_stretch: HashSet<i32> = attached_marks
        .iter()
        .flat_map(|attachment| attachment.object_cluster_index..attachment.mark_cluster_index)
        .collect();
    let attachment_decisions: Vec<_> = attached_marks
        .iter()
        .filter(|attachment| !attachment.separator_cluster_indices.is_empty())
        .map(|attachment| {
            let first = &natural[attachment.separator_cluster_indices[0] as usize];
            let last = &natural[*attachment.separator_cluster_indices.last().unwrap() as usize];
            let mark = &natural[attachment.mark_cluster_index as usize];
            InlineObjectPunctuationAttachmentDecisionInfo::new(
                natural[attachment.object_cluster_index as usize].range,
                TextRange::new(first.range.start(), last.range.end()),
                mark.range,
                mark.text.clone(),
                TextRange::new(
                    natural[attachment.object_cluster_index as usize]
                        .range
                        .start(),
                    mark.range.end(),
                ),
                attachment
                    .separator_cluster_indices
                    .iter()
                    .map(|index| natural[*index as usize].advance)
                    .sum(),
            )
        })
        .collect();
    let mandatory: HashSet<i32> = natural
        .iter()
        .enumerate()
        .filter(|(_, cluster)| is_mandatory_break_cluster(cluster))
        .map(|(index, _)| index as i32)
        .collect();
    let zero_width: HashSet<i32> = natural
        .iter()
        .enumerate()
        .filter(|(_, cluster)| is_zero_width_soft_break_cluster(cluster))
        .map(|(index, _)| index as i32)
        .collect();
    let mandatory_decisions: Vec<_> = natural
        .iter()
        .enumerate()
        .filter(|(_, cluster)| is_mandatory_break_cluster(cluster))
        .map(|(index, cluster)| MandatoryBreakDecisionInfo {
            range: cluster.range,
            source_text: cluster.text.clone(),
            break_after_cluster_index: index as i32,
            reason: "MandatoryBreakNoShape".to_owned(),
        })
        .collect();
    let mut zero_indexes: Vec<_> = zero_width.iter().copied().collect();
    zero_indexes.sort();
    let zero_decisions: Vec<_> = zero_indexes
        .iter()
        .map(|index| {
            let cluster = &natural[*index as usize];
            ZeroWidthBreakDecisionInfo::new(cluster.range, cluster.text.clone(), *index)
        })
        .collect();
    let atoms: Vec<_> = natural
        .iter()
        .enumerate()
        .filter(|(index, _)| cluster_roles[*index] != FontRole::LatinText)
        .flat_map(|(_, cluster)| {
            punctuation_atoms(
                cluster,
                font_size,
                punctuation_atom_builder,
                shaped_glyphs
                    .get(&cluster.range)
                    .map(Vec::as_slice)
                    .unwrap_or(&[]),
                annotation.clreq_profile.glue_placement,
                annotation.clreq_profile.punctuation_width,
            )
        })
        .collect();
    let mut adjustments = punctuation_spacing_compressor
        .compress(&atoms, font_size)
        .adjustments;
    adjustments.extend(
        punctuation_spacing_compressor
            .compress_cjk_closing_before_ascii_point_mark(&atoms, text, font_size)
            .adjustments,
    );
    let _spacing_plan = PunctuationSpacingCompressionResult::new(adjustments);
    let mut ruby_spread = HashMap::new();
    if !annotation.pinyin_spans.is_empty() {
        let word_space = annotation.ruby_font_size * RUBY_MIN_GAP_EM_OF_RUBY;
        let mut left = Vec::with_capacity(natural.len());
        let mut position = 0.;
        for cluster in &natural {
            left.push(position);
            position += cluster.advance
        }
        let mut measures: Vec<_> = annotation
            .pinyin_spans
            .iter()
            .filter_map(|ruby| {
                cluster_index_range_for(&natural, ruby.base_range).and_then(|(first, last)| {
                    annotation
                        .ruby_font_geometry_by_span
                        .get(ruby)
                        .map(|geometry| {
                            (
                                first,
                                (left[first as usize]
                                    + left[last as usize]
                                    + natural[last as usize].advance)
                                    / 2.,
                                geometry.width,
                            )
                        })
                })
            })
            .collect();
        measures.sort_by_key(|entry| entry.0);
        let mut shift = 0.;
        let mut previous_right = f32::NEG_INFINITY;
        for (first, center_natural, width) in measures {
            let mut center = center_natural + shift;
            let needed = previous_right + word_space - (center - width / 2.);
            if needed > 0. && first > 0 {
                *ruby_spread.entry(first - 1).or_insert(0.) += needed;
                shift += needed;
                center += needed
            }
            previous_right = center + width / 2.;
        }
    }
    let atoms: Vec<_> = natural
        .iter()
        .enumerate()
        .filter(|(index, _)| cluster_roles[*index] != FontRole::LatinText)
        .flat_map(|(_, cluster)| {
            punctuation_atoms(
                cluster,
                font_size,
                punctuation_atom_builder,
                shaped_glyphs
                    .get(&cluster.range)
                    .map(Vec::as_slice)
                    .unwrap_or(&[]),
                annotation.clreq_profile.glue_placement,
                annotation.clreq_profile.punctuation_width,
            )
        })
        .collect();
    let mut adjustments = punctuation_spacing_compressor
        .compress(&atoms, font_size)
        .adjustments;
    adjustments.extend(
        punctuation_spacing_compressor
            .compress_cjk_closing_before_ascii_point_mark(&atoms, text, font_size)
            .adjustments,
    );
    let spacing_plan = PunctuationSpacingCompressionResult::new(adjustments);
    let mut ruby_spread = HashMap::new();
    if !annotation.pinyin_spans.is_empty() {
        let word_space = annotation.ruby_font_size * RUBY_MIN_GAP_EM_OF_RUBY;
        let mut left = Vec::with_capacity(natural.len());
        let mut position = 0.;
        for cluster in &natural {
            left.push(position);
            position += cluster.advance
        }
        let mut measures: Vec<_> = annotation
            .pinyin_spans
            .iter()
            .filter_map(|ruby| {
                cluster_index_range_for(&natural, ruby.base_range).map(|(first, last)| {
                    let geometry = annotation
                        .ruby_font_geometry_by_span
                        .get(ruby)
                        .expect("pinyin ruby span must have measured geometry");
                    (
                        first,
                        (left[first as usize]
                            + left[last as usize]
                            + natural[last as usize].advance)
                            / 2.,
                        geometry.width,
                    )
                })
            })
            .collect();
        measures.sort_by_key(|entry| entry.0);
        let mut shift = 0.;
        let mut previous_right = f32::NEG_INFINITY;
        for (first, center_natural, width) in measures {
            let mut center = center_natural + shift;
            let needed = previous_right + word_space - (center - width / 2.);
            if needed > 0. && first > 0 {
                *ruby_spread.entry(first - 1).or_insert(0.) += needed;
                shift += needed;
                center += needed
            }
            previous_right = center + width / 2.;
        }
    }
    for ruby in input
        .ruby_spans
        .iter()
        .filter(|ruby| ruby.kind == RubyKind::Bopomofo)
    {
        if let Some((_, last)) = cluster_index_range_for(&natural, ruby.base_range) {
            *ruby_spread.entry(last).or_insert(0.) += 0.5 * font_size
        }
    }
    let natural_attachments: Vec<_> = natural
        .iter()
        .map(|cluster| (annotation.style_at)(cluster.range.start()).inline_attachment)
        .collect();
    let punctuation_base = PunctuationGeometryLedger::from(natural.clone(), &atoms, &spacing_plan)
        .with_inline_box_advances(&inline_box_result.advance_by_cluster)
        .with_ruby_spread(&ruby_spread)
        .with_raw_edge_trims(&separator_trims);
    let attached_punctuation_boundary = punctuation_base
        .resolve_attached_inline_punctuation_boundaries(&natural_attachments, &atoms, font_size);
    let base_geometry = attached_punctuation_boundary.geometry.clone();
    let trailing_glue = attached_punctuation_boundary
        .trailing_glue_by_cluster
        .clone();
    let clusters = base_geometry.resolve_clusters();
    let glue_caps = base_geometry.glue_capacities();
    let gap_ranges: HashSet<TextRange> = auto_space_decisions
        .iter()
        .filter(|decision| decision.side == "gap")
        .map(|decision| decision.cluster_range)
        .collect();
    let atom_class: HashMap<TextRange, PunctuationClass> =
        first_contained_item(&natural, &atoms, |atom| atom.range)
            .into_iter()
            .enumerate()
            .filter_map(|(index, atom)| {
                atom.map(|atom| (natural[index].range, atoms[atom].punctuation_class))
            })
            .collect();
    let adjustment = annotation.clreq_profile.adjustment;
    let mut shrink = Vec::new();
    for (index, cluster) in natural.iter().enumerate() {
        let index = index as i32;
        if let Some(capacity) = glue_caps.get(&index) {
            let class = atom_class.get(&cluster.range).copied();
            let tier = match class {
                Some(PunctuationClass::Interpunct | PunctuationClass::MiddleDot) => 3,
                Some(
                    PunctuationClass::Opening | PunctuationClass::Closing | PunctuationClass::Quote,
                ) => 4,
                Some(PunctuationClass::PauseOrStop)
                    if cluster
                        .display_text
                        .chars()
                        .next()
                        .is_some_and(|character| INLINE_STOPS.contains(&character)) =>
                {
                    7
                }
                _ => 5,
            };
            let end_only = tier == 7 && !adjustment.allow_inline_stop_compression;
            if capacity.paired {
                let amount = 2. * capacity.leading.min(capacity.trailing);
                if amount > 0. {
                    shrink.push(ShrinkOpportunity::with_line_end_only(
                        index,
                        tier,
                        amount,
                        ShrinkChannel::LeadingAndTrailingGlue,
                        end_only,
                    ))
                }
            } else {
                if capacity.leading > 0. {
                    shrink.push(ShrinkOpportunity::with_line_end_only(
                        index,
                        tier,
                        capacity.leading,
                        ShrinkChannel::LeadingGlue,
                        end_only,
                    ))
                }
                if capacity.trailing > 0. {
                    shrink.push(ShrinkOpportunity::with_line_end_only(
                        index,
                        tier,
                        capacity.trailing,
                        ShrinkChannel::TrailingGlue,
                        end_only,
                    ))
                }
            }
        } else if !cluster.text.is_empty()
            && cluster.text.chars().all(|character| character == ' ')
            && !separator_trims.contains_key(&index)
        {
            if gap_ranges.contains(&cluster.range) {
                let capacity = cluster.advance - SINO_WESTERN_GAP_MIN_EM * font_size;
                if adjustment.allow_sino_western_gap_adjustment && capacity > 0. {
                    shrink.push(ShrinkOpportunity::new(
                        index,
                        6,
                        capacity,
                        ShrinkChannel::RawAdvance,
                    ))
                }
            } else {
                let capacity = cluster.advance - WORD_SPACE_MIN_EM * font_size;
                if capacity > 0. {
                    shrink.push(ShrinkOpportunity::new(
                        index,
                        2,
                        capacity,
                        ShrinkChannel::RawAdvance,
                    ))
                }
            }
        }
        for (index, _) in natural.iter().enumerate() {
            let index = index as i32;
            let Some(object) = inline_object_by_cluster_index.get(&index) else {
                continue;
            };
            if object.trailing_boundary.shrink_capacity > 0. {
                shrink.push(ShrinkOpportunity::new(
                    index,
                    8,
                    object.trailing_boundary.shrink_capacity,
                    ShrinkChannel::RawAdvance,
                ))
            }
        }
    }
    ParagraphLayoutPrep {
        input: input.clone(),
        rejected_technical_tiers_by_span: rejected.clone(),
        text: text.clone(),
        font_size,
        style_at: annotation.style_at.clone(),
        font_size_at: annotation.font_size_at.clone(),
        bopomofo_font_weight_at: annotation.bopomofo_font_weight_at.clone(),
        ruby_font_size: annotation.ruby_font_size,
        ruby_stack_gap: annotation.ruby_stack_gap,
        ruby_font_weight: annotation.ruby_font_weight,
        pinyin_spans: annotation.pinyin_spans.clone(),
        clreq_profile: annotation.clreq_profile.clone(),
        punctuation_glyph_substitutor: annotation.punctuation_glyph_substitutor,
        measure,
        measure_em,
        grid_body_offset,
        line_length_grid_decision,
        quote_pairs: annotation.quote_pairs.clone(),
        role_override_infos: annotation.role_override_infos.clone(),
        font_decisions: annotation.font_decisions.clone(),
        hyphen_offsets: shaping_stage.hyphen_offsets,
        hyphen_advance: shaping_stage.hyphen_advance,
        hyphen_glyphs: shaping_stage.hyphen_glyphs,
        substitution_rollbacks: shaping_stage.substitution_rollbacks,
        break_opportunity_decisions: shaping_stage.break_opportunity_decisions,
        emergency_tracking_eligibility_decisions: shaping_stage
            .emergency_tracking_eligibility_decisions,
        progressive_break_offsets: shaping_stage.progressive_break_offsets,
        shaped_glyphs_by_cluster_range: shaped_glyphs,
        open_type_features_by_cluster_range: features,
        shaping_decisions: shaping_results
            .iter()
            .flat_map(|result| result.decisions.clone())
            .collect(),
        east_asian_spacing_edges,
        auto_space_decisions,
        inline_box_result,
        natural_clusters: natural,
        inline_object_by_cluster_index,
        uniform_inline_object_boundary_after_clusters: uniform,
        preferred_inline_object_boundary_after_clusters: preferred,
        inline_object_boundary_unbreakable_ranges: unbreakable,
        cluster_roles,
        resolved_kinsoku,
        kinsoku_rule,
        inline_object_attached_marks: attached_marks,
        inline_object_separator_space_trims: separator_trims,
        inline_object_attachment_no_stretch_boundaries: attachment_no_stretch,
        inline_object_punctuation_attachment_decisions: attachment_decisions,
        mandatory_break_clusters: mandatory,
        zero_width_break_clusters: zero_width,
        mandatory_break_decisions: mandatory_decisions,
        zero_width_break_decisions: zero_decisions,
        punctuation_atoms: atoms,
        spacing_plan,
        ruby_font_geometry_by_span: annotation.ruby_font_geometry_by_span.clone(),
        ruby_and_bopomofo_spread: ruby_spread,
        natural_inline_attachments: natural_attachments,
        attached_punctuation_boundary,
        base_geometry,
        attached_punctuation_trailing_glue_by_cluster: trailing_glue,
        clusters,
        adjustment_style: adjustment,
        atom_class_by_range: atom_class,
        shrink_opportunities: shrink,
    }
}
const RUBY_FONT_EM: f32 = 0.5;
const RUBY_FONT_WEIGHT_BOOST: i32 = 100;
const BOPOMOFO_FONT_WEIGHT_BOOST: i32 = 300;
const RUBY_STACK_GAP_EM: f32 = 0.;
const RUBY_MIN_GAP_EM_OF_RUBY: f32 = 0.25;
const WORD_SPACE_MIN_EM: f32 = 0.25;
const SINO_WESTERN_GAP_MIN_EM: f32 = 0.125;
const INLINE_STOPS: [char; 4] = ['。', '！', '？', '．'];
