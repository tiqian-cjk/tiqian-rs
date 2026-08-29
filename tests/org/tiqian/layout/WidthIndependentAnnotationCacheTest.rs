use tiqian::common::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use tiqian::org::tiqian::core::Geometry::{LayoutConstraints, TextRange};
use tiqian::org::tiqian::core::Text::Text;
use tiqian::org::tiqian::core::TextModel::{
    DecorationKind, DecorationSpan, InlineBoxSpan, LayoutInput, ParagraphStyle, RubySpan,
    TextStyle, TiqianTextContent,
};
use tiqian::org::tiqian::core::Units::Ic;
use tiqian::org::tiqian::layout::ParagraphLayoutEngine::{
    ExplainableStubParagraphLayoutEngine, ParagraphLayoutEngine,
};
use tiqian::org::tiqian::layout::WidthIndependentAnnotationCache::{
    LruWidthIndependentAnnotationCache, WidthIndependentAnnotationCache,
    WidthIndependentAnnotationKey, WidthIndependentParagraphAnnotation,
    to_width_independent_annotation_key,
};
use tiqian::org::tiqian::shaping::TextShaper::{
    ExplainableStubTextShaper, ShapingInput, ShapingResult, TextShaper,
};

struct CountingTextShaper {
    count: Arc<AtomicUsize>,
}

impl TextShaper for CountingTextShaper {
    fn shape(&self, input: &ShapingInput) -> ShapingResult {
        self.count.fetch_add(1, Ordering::SeqCst);
        ExplainableStubTextShaper.shape(input)
    }
}

struct SharedLruCache {
    entries: Arc<Mutex<LruWidthIndependentAnnotationCache>>,
}

impl WidthIndependentAnnotationCache for SharedLruCache {
    fn get(
        &mut self,
        key: &WidthIndependentAnnotationKey,
    ) -> Option<Arc<WidthIndependentParagraphAnnotation>> {
        self.entries.lock().unwrap().get(key)
    }

    fn put(
        &mut self,
        key: WidthIndependentAnnotationKey,
        annotation: Arc<WidthIndependentParagraphAnnotation>,
    ) {
        self.entries.lock().unwrap().put(key, annotation);
    }

    fn clear(&mut self) {
        self.entries.lock().unwrap().clear();
    }

    fn size(&self) -> usize {
        self.entries.lock().unwrap().size()
    }
}

struct DisabledCache;

impl WidthIndependentAnnotationCache for DisabledCache {
    fn get(
        &mut self,
        _: &WidthIndependentAnnotationKey,
    ) -> Option<Arc<WidthIndependentParagraphAnnotation>> {
        None
    }

    fn put(
        &mut self,
        _: WidthIndependentAnnotationKey,
        _: Arc<WidthIndependentParagraphAnnotation>,
    ) {
    }

    fn clear(&mut self) {}

    fn size(&self) -> usize {
        0
    }
}

fn input(text: &str, width: f32) -> LayoutInput {
    LayoutInput::builder(
        TiqianTextContent::new(Text::from(text)),
        LayoutConstraints::with_defaults(width),
    )
    .paragraph_style(
        ParagraphStyle::builder()
            .first_line_indent(Some(Ic::ZERO))
            .build(),
    )
    .build()
}

#[test]
fn relayout_at_three_widths_hits_annotation_cache_without_reshaping() {
    let calls = Arc::new(AtomicUsize::new(0));
    let entries = Arc::new(Mutex::new(LruWidthIndependentAnnotationCache::new(64)));
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.text_shaper = Box::new(CountingTextShaper {
        count: calls.clone(),
    });
    engine.annotation_cache = Box::new(SharedLruCache {
        entries: entries.clone(),
    });
    let normal = input("提椠是一个面向中文正文的 CJK 段落布局引擎。", 300.0);

    let normal_result = engine.layout(normal.clone());
    let initial_calls = calls.load(Ordering::SeqCst);
    assert!(initial_calls > 0);
    assert_eq!(1, entries.lock().unwrap().size());

    let narrow_result = engine.layout(input(&normal.content.text, 180.0));
    let wide_result = engine.layout(input(&normal.content.text, 500.0));
    assert_eq!(initial_calls, calls.load(Ordering::SeqCst));
    assert!(narrow_result.lines.len() >= normal_result.lines.len());
    assert!(normal_result.lines.len() >= wide_result.lines.len());
}

#[test]
fn cache_key_distinguishes_text_style_decoration_ruby_and_inline_box() {
    let entries = Arc::new(Mutex::new(LruWidthIndependentAnnotationCache::new(64)));
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.annotation_cache = Box::new(SharedLruCache {
        entries: entries.clone(),
    });
    let base = input("中西混合排版与测试文本。", 300.0);

    engine.layout(base.clone());
    engine.layout(input("中西混合排版与变动文本。", 300.0));
    let mut font_changed = base.clone();
    font_changed.text_style = TextStyle::builder().font_size(24.0).build();
    engine.layout(font_changed);
    let mut emphasis_changed = base.clone();
    emphasis_changed.decorations = vec![DecorationSpan {
        range: TextRange::new(0, 4),
        kind: DecorationKind::Emphasis,
    }];
    engine.layout(emphasis_changed);
    let mut ruby_changed = base.clone();
    ruby_changed.ruby_spans = vec![RubySpan::new(TextRange::new(0, 2), Text::from("zhōngxī"))];
    engine.layout(ruby_changed);
    let mut inline_box_changed = base;
    inline_box_changed.inline_boxes =
        vec![InlineBoxSpan::with_edges(TextRange::new(2, 4), 4.0, 4.0)];
    engine.layout(inline_box_changed);

    assert_eq!(6, entries.lock().unwrap().size());
}

#[test]
fn lru_refreshes_accessed_entry_before_evicting_least_recently_used_entry() {
    let entries = Arc::new(Mutex::new(LruWidthIndependentAnnotationCache::new(2)));
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.annotation_cache = Box::new(SharedLruCache {
        entries: entries.clone(),
    });
    let first = input("段落一文本内容", 300.0);
    let second = input("段落二文本内容", 300.0);
    let third = input("段落三文本内容", 300.0);
    let first_key = to_width_independent_annotation_key(&first, HashMap::new());
    let second_key = to_width_independent_annotation_key(&second, HashMap::new());
    let third_key = to_width_independent_annotation_key(&third, HashMap::new());

    engine.layout(first);
    engine.layout(second);
    {
        let mut cache = entries.lock().unwrap();
        assert!(cache.get(&first_key).is_some());
    }
    engine.layout(third);
    let mut cache = entries.lock().unwrap();
    assert_eq!(2, cache.size());
    assert!(cache.get(&third_key).is_some());
    assert!(cache.get(&first_key).is_some());
    assert!(cache.get(&second_key).is_none());
}

#[test]
fn cached_and_uncached_layouts_match_at_narrow_normal_and_wide_widths() {
    let text =
        "提椠是一个面向中文正文的段落排版引擎，遵循中文排版需求规范，支持两端对齐与标点挤压。";
    let mut cached = ExplainableStubParagraphLayoutEngine::default();
    let mut uncached = ExplainableStubParagraphLayoutEngine::default();
    uncached.annotation_cache = Box::new(DisabledCache);

    for width in [80.0, 300.0, 650.0] {
        let expected = uncached.layout(input(text, width));
        let actual = cached.layout(input(text, width));
        assert_eq!(expected.lines.len(), actual.lines.len(), "width {width}");
        for (expected_line, actual_line) in expected.lines.iter().zip(&actual.lines) {
            assert_eq!(expected_line.range, actual_line.range, "width {width}");
            assert!(
                (expected_line.visual_width - actual_line.visual_width).abs() < 0.001,
                "width {width}"
            );
            assert!(
                (expected_line.adjusted_width - actual_line.adjusted_width).abs() < 0.001,
                "width {width}"
            );
        }
    }
}
