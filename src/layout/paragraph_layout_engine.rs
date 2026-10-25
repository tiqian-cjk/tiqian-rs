// 对应 Kotlin 源文件：engine/src/commonMain/kotlin/org/tiqian/layout/ParagraphLayoutEngine.kt

use crate::common::{HashMap, HashSet};
use std::sync::Arc;

use super::super::clreq::clreq_profile::{BuiltInClreqProfileResolver, ClreqProfileResolver};
use super::super::core::geometry::TextRange;
use super::super::core::layout_model::LayoutResult;
use super::super::core::text_model::LayoutInput;
use super::super::font::font_metrics::{FontMetricsNormalizer, ScriptAwareFontMetricsNormalizer};
use super::super::font::font_policy::{CjkFontRoleClassifier, FontRoleClassifier};
use super::super::linebreak::hyphenation::Hyphenator;
use super::super::shaping::font_backend::FontBackend;
use super::default_hyphenator::default_hyphenator;
use super::justifier::Justifier;
use super::line_adjustment_stage::{
    LineAdjustmentRequest, LineAdjustmentStageOutcome, finish_paragraph_layout,
};
use super::line_break_planning_stage::{LineBreakPlanningRequest, plan_paragraph_lines};
use super::line_breaker::{GreedyLineBreaker, LineBreaker};
use super::progressive_break_decisions::ProgressiveBreakTier;
use super::punctuation_model::{PunctuationAtomBuilder, PunctuationSpacingCompressor};
use super::quote_pair_analyzer::QuotePairAnalyzer;
use super::width_independent_annotation_cache::{
    LruWidthIndependentAnnotationCache, WidthIndependentAnnotationCache,
    build_paragraph_layout_prep, prepare_width_independent_annotation,
    to_width_independent_annotation_key,
};

pub struct ParagraphLayoutEngine {
    font_role_classifier: Box<dyn FontRoleClassifier>,
    font_backend: Box<dyn FontBackend>,
    clreq_profile_resolver: Box<dyn ClreqProfileResolver>,
    font_metrics_normalizer: Box<dyn FontMetricsNormalizer>,
    punctuation_atom_builder: PunctuationAtomBuilder,
    punctuation_spacing_compressor: PunctuationSpacingCompressor,
    quote_pair_analyzer: QuotePairAnalyzer,
    line_breaker: Box<dyn LineBreaker>,
    justifier: Justifier,
    hyphenator: &'static dyn Hyphenator,
    annotation_cache: Box<dyn WidthIndependentAnnotationCache>,
}

pub struct ParagraphLayoutEngineBuilder {
    font_backend: Box<dyn FontBackend>,
    clreq_profile_resolver: Box<dyn ClreqProfileResolver>,
    line_breaker: Box<dyn LineBreaker>,
    hyphenator: &'static dyn Hyphenator,
    annotation_cache: Box<dyn WidthIndependentAnnotationCache>,
}

impl ParagraphLayoutEngineBuilder {
    pub fn new(font_backend: Box<dyn FontBackend>) -> Self {
        Self {
            font_backend,
            clreq_profile_resolver: Box::new(BuiltInClreqProfileResolver),
            line_breaker: Box::new(GreedyLineBreaker::default()),
            hyphenator: default_hyphenator(),
            annotation_cache: Box::new(LruWidthIndependentAnnotationCache::default()),
        }
    }

    pub fn clreq_profile_resolver(
        mut self,
        clreq_profile_resolver: Box<dyn ClreqProfileResolver>,
    ) -> Self {
        self.clreq_profile_resolver = clreq_profile_resolver;
        self
    }

    pub fn line_breaker(mut self, line_breaker: Box<dyn LineBreaker>) -> Self {
        self.line_breaker = line_breaker;
        self
    }

    pub fn hyphenator(mut self, hyphenator: &'static dyn Hyphenator) -> Self {
        self.hyphenator = hyphenator;
        self
    }

    pub fn annotation_cache(
        mut self,
        annotation_cache: Box<dyn WidthIndependentAnnotationCache>,
    ) -> Self {
        self.annotation_cache = annotation_cache;
        self
    }

    pub fn build(self) -> ParagraphLayoutEngine {
        ParagraphLayoutEngine {
            font_role_classifier: Box::new(CjkFontRoleClassifier),
            font_backend: self.font_backend,
            clreq_profile_resolver: self.clreq_profile_resolver,
            font_metrics_normalizer: Box::new(ScriptAwareFontMetricsNormalizer),
            punctuation_atom_builder: PunctuationAtomBuilder::default(),
            punctuation_spacing_compressor: PunctuationSpacingCompressor,
            quote_pair_analyzer: QuotePairAnalyzer,
            line_breaker: self.line_breaker,
            justifier: Justifier::default(),
            hyphenator: self.hyphenator,
            annotation_cache: self.annotation_cache,
        }
    }
}

impl ParagraphLayoutEngine {
    pub fn layout(&mut self, input: LayoutInput) -> LayoutResult {
        self.layout_with_rejected_technical_tiers(input, HashMap::new())
    }

    // 用于递归重试。
    fn layout_with_rejected_technical_tiers(
        &mut self,
        input: LayoutInput,
        rejected_technical_tiers_by_span: HashMap<TextRange, HashSet<ProgressiveBreakTier>>,
    ) -> LayoutResult {
        let cache_key =
            to_width_independent_annotation_key(&input, rejected_technical_tiers_by_span.clone());
        let annotation = self.annotation_cache.get(&cache_key).unwrap_or_else(|| {
            let annotation = Arc::new(prepare_width_independent_annotation(
                &input,
                &rejected_technical_tiers_by_span,
                self.clreq_profile_resolver.as_ref(),
                self.font_role_classifier.as_ref(),
                self.font_backend.as_ref(),
                &self.quote_pair_analyzer,
                self.hyphenator,
            ));
            self.annotation_cache.put(cache_key, annotation.clone());
            annotation
        });
        let prep = build_paragraph_layout_prep(
            &input,
            annotation.as_ref(),
            &rejected_technical_tiers_by_span,
            self.font_backend.as_ref(),
            self.hyphenator,
            &self.punctuation_atom_builder,
            &self.punctuation_spacing_compressor,
        );
        let plan = plan_paragraph_lines(LineBreakPlanningRequest::new(
            &prep,
            self.font_backend.as_ref(),
            self.font_metrics_normalizer.as_ref(),
            &self.justifier,
            self.line_breaker.as_ref(),
        ));
        match finish_paragraph_layout(LineAdjustmentRequest {
            prep,
            plan: &plan,
            justifier: &self.justifier,
            line_breaker_strategy_name: self.line_breaker.strategy_name(),
            font_backend: self.font_backend.as_ref(),
        }) {
            LineAdjustmentStageOutcome::Finished(result) => *result,
            LineAdjustmentStageOutcome::Retry {
                rejected_technical_tiers_by_span,
            } => self.layout_with_rejected_technical_tiers(input, rejected_technical_tiers_by_span),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ParagraphLayoutEngine;

    #[test]
    fn paragraph_layout_engine_is_send_and_sync() {
        fn assert_send_and_sync<T: Send + Sync>() {}

        assert_send_and_sync::<ParagraphLayoutEngine>();
    }
}
