// 对应 Kotlin 源文件：engine/src/commonMain/kotlin/org/tiqian/layout/ParagraphLayoutEngine.kt

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use super::super::clreq::ClreqProfile::{BuiltInClreqProfileResolver, ClreqProfileResolver};
use super::super::core::Geometry::TextRange;
use super::super::core::LayoutModel::LayoutResult;
use super::super::core::TextModel::LayoutInput;
use super::super::font::FontMetrics::{
    FontMetricsNormalizer, FontMetricsResolver, ScriptAwareFontMetricsNormalizer,
    StubFontMetricsResolver,
};
use super::super::font::FontPolicy::{
    CjkFontRoleClassifier, FallbackResolver, FontRoleClassifier,
    PreferCjkForAmbiguousPunctuationResolver,
};
use super::super::linebreak::Hyphenation::Hyphenator;
use super::super::shaping::TextShaper::{ExplainableStubTextShaper, TextShaper};
use super::DefaultHyphenator::default_hyphenator;
use super::Justifier::Justifier;
use super::LineAdjustmentStage::{
    LineAdjustmentRequest, LineAdjustmentStageOutcome, finish_paragraph_layout,
};
use super::LineBreakPlanningStage::{LineBreakPlanningRequest, plan_paragraph_lines};
use super::LineBreaker::{GreedyLineBreaker, LineBreaker};
use super::ProgressiveBreakDecisions::ProgressiveBreakTier;
use super::PunctuationModel::{PunctuationAtomBuilder, PunctuationSpacingCompressor};
use super::QuotePairAnalyzer::QuotePairAnalyzer;
use super::WidthIndependentAnnotationCache::{
    LruWidthIndependentAnnotationCache, WidthIndependentAnnotationCache, build_paragraph_layout_prep,
    prepare_width_independent_annotation, to_width_independent_annotation_key,
};

pub const MANDATORY_BREAK_FONT_KEY: &str = "mandatory-break";

pub trait ParagraphLayoutEngine {
    fn layout(&mut self, input: LayoutInput) -> LayoutResult;
}

pub struct ExplainableStubParagraphLayoutEngine {
    pub font_role_classifier: Box<dyn FontRoleClassifier>,
    pub fallback_resolver: Box<dyn FallbackResolver>,
    pub clreq_profile_resolver: Box<dyn ClreqProfileResolver>,
    pub font_metrics_resolver: Box<dyn FontMetricsResolver>,
    pub font_metrics_normalizer: Box<dyn FontMetricsNormalizer>,
    pub punctuation_atom_builder: PunctuationAtomBuilder,
    pub punctuation_spacing_compressor: PunctuationSpacingCompressor,
    pub quote_pair_analyzer: QuotePairAnalyzer,
    pub line_breaker: Box<dyn LineBreaker>,
    pub justifier: Justifier,
    pub text_shaper: Box<dyn TextShaper>,
    pub hyphenator: &'static dyn Hyphenator,
    pub annotation_cache: Box<dyn WidthIndependentAnnotationCache>,
}

impl Default for ExplainableStubParagraphLayoutEngine {
    fn default() -> Self {
        Self {
            font_role_classifier: Box::new(CjkFontRoleClassifier),
            fallback_resolver: Box::new(PreferCjkForAmbiguousPunctuationResolver::default()),
            clreq_profile_resolver: Box::new(BuiltInClreqProfileResolver),
            font_metrics_resolver: Box::new(StubFontMetricsResolver),
            font_metrics_normalizer: Box::new(ScriptAwareFontMetricsNormalizer),
            punctuation_atom_builder: PunctuationAtomBuilder::default(),
            punctuation_spacing_compressor: PunctuationSpacingCompressor,
            quote_pair_analyzer: QuotePairAnalyzer,
            line_breaker: Box::new(GreedyLineBreaker::default()),
            justifier: Justifier::default(),
            text_shaper: Box::new(ExplainableStubTextShaper),
            hyphenator: default_hyphenator(),
            annotation_cache: Box::new(LruWidthIndependentAnnotationCache::default()),
        }
    }
}

impl ExplainableStubParagraphLayoutEngine {
    pub fn layout_with_rejected_technical_tiers(
        &mut self,
        input: LayoutInput,
        rejected_technical_tiers_by_span: HashMap<TextRange, HashSet<ProgressiveBreakTier>>,
    ) -> LayoutResult {
        validate_layout_input(&input);
        let cache_key =
            to_width_independent_annotation_key(&input, rejected_technical_tiers_by_span.clone());
        let annotation = self.annotation_cache.get(&cache_key).unwrap_or_else(|| {
            let annotation = Arc::new(prepare_width_independent_annotation(
                &input,
                &rejected_technical_tiers_by_span,
                self.clreq_profile_resolver.as_ref(),
                self.font_role_classifier.as_ref(),
                self.fallback_resolver.as_ref(),
                self.font_metrics_resolver.as_ref(),
                &self.quote_pair_analyzer,
                self.text_shaper.as_ref(),
                self.hyphenator,
            ));
            self.annotation_cache.put(cache_key, annotation.clone());
            annotation
        });
        let prep = build_paragraph_layout_prep(
            &input,
            annotation.as_ref(),
            &rejected_technical_tiers_by_span,
            self.text_shaper.as_ref(),
            self.hyphenator,
            &self.punctuation_atom_builder,
            &self.punctuation_spacing_compressor,
        );
        let plan = plan_paragraph_lines(LineBreakPlanningRequest::new(
            &prep,
            self.font_metrics_resolver.as_ref(),
            self.font_metrics_normalizer.as_ref(),
            &self.justifier,
            self.line_breaker.as_ref(),
        ));
        match finish_paragraph_layout(LineAdjustmentRequest {
            prep: &prep,
            plan: &plan,
            justifier: &self.justifier,
            line_breaker_strategy_name: self.line_breaker.strategy_name(),
            fallback_resolver: self.fallback_resolver.as_ref(),
            text_shaper: self.text_shaper.as_ref(),
        }) {
            LineAdjustmentStageOutcome::Finished(result) => result,
            LineAdjustmentStageOutcome::Retry {
                rejected_technical_tiers_by_span,
            } => self.layout_with_rejected_technical_tiers(input, rejected_technical_tiers_by_span),
        }
    }
}

impl ParagraphLayoutEngine for ExplainableStubParagraphLayoutEngine {
    fn layout(&mut self, input: LayoutInput) -> LayoutResult {
        self.layout_with_rejected_technical_tiers(input, HashMap::new())
    }
}

fn validate_layout_input(input: &LayoutInput) {
    let text_length = input.content.text.encode_utf16().count() as i32;
    assert!(
        input.paragraph_style.emphasis_dot_gap_em.is_finite()
            && input.paragraph_style.emphasis_dot_gap_em >= 0.0,
        "ParagraphStyle.emphasisDotGapEm must be finite and non-negative"
    );
    assert!(
        input
            .paragraph_style
            .inline_object_minimum_clearance_em
            .is_finite()
            && input.paragraph_style.inline_object_minimum_clearance_em >= 0.0,
        "ParagraphStyle.inlineObjectMinimumClearanceEm must be finite and non-negative"
    );
    for inline_box in &input.inline_boxes {
        assert!(
            is_non_empty_source_range(inline_box.range, text_length),
            "InlineBoxSpan {:?} must be a non-empty source range",
            inline_box.range
        );
        assert!(
            inline_box.inline_start.is_finite() && inline_box.inline_end.is_finite(),
            "InlineBoxSpan {:?} must have finite inline edges",
            inline_box.range
        );
    }
    for span in &input.content.line_break_spans {
        assert!(
            is_non_empty_source_range(span.range, text_length),
            "LineBreakSpan {:?} must be a non-empty source range",
            span.range
        );
    }
    for range in &input.content.auto_space_suppressed_ranges {
        assert!(
            is_non_empty_source_range(*range, text_length),
            "Auto-space suppressed range {:?} must be a non-empty source range",
            range
        );
    }
    let mut objects = input.inline_objects.clone();
    objects.sort_by_key(|object| object.range.start());
    assert!(
        objects
            .windows(2)
            .all(|pair| pair[0].range != pair[1].range),
        "InlineObjectSpan ranges must be unique"
    );
    assert!(
        objects
            .windows(2)
            .all(|pair| pair[0].range.end() <= pair[1].range.start()),
        "InlineObjectSpan ranges must not overlap"
    );
    for object in &objects {
        assert!(
            is_non_empty_source_range(object.range, text_length),
            "InlineObjectSpan {:?} must cover a non-empty source range",
            object.range
        );
        assert!(
            object.advance.is_finite()
                && object.advance > 0.0
                && object.ascent.is_finite()
                && object.ascent >= 0.0
                && object.descent.is_finite()
                && object.descent >= 0.0,
            "InlineObjectSpan {:?} must have finite positive geometry",
            object.range
        );
        assert!(
            object.leading_boundary.shrink_capacity == 0.0,
            "InlineObjectSpan {:?} cannot shrink its leading boundary",
            object.range
        );
        assert!(
            object.leading_boundary.line_end_discardable_advance == 0.0,
            "InlineObjectSpan {:?} cannot discard advance at its leading boundary",
            object.range
        );
        assert!(
            object.trailing_boundary.shrink_capacity <= object.advance,
            "InlineObjectSpan {:?} trailing shrink capacity must not exceed its advance",
            object.range
        );
        assert!(
            object.trailing_boundary.line_end_discardable_advance <= object.advance,
            "InlineObjectSpan {:?} trailing line-end discard must not exceed its advance",
            object.range
        );
    }
}

fn is_non_empty_source_range(range: TextRange, text_length: i32) -> bool {
    range.start() >= 0 && range.start() < range.end() && range.end() <= text_length
}
