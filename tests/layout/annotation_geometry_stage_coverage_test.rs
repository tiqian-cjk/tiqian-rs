use std::cell::Cell;

use tiqian::common::HashMap;
use tiqian::core::geometry::{LayoutConstraints, Rect, TextRange};
use tiqian::core::int_range::IntRange;
use tiqian::core::layout_model::{
    AutoSpaceDecisionInfo, Cluster, ClusterGeometryDecisionInfo, Glyph, GlyphRun, LineBox,
    LineEndReason,
};
use tiqian::core::text::Text;
use tiqian::core::text_model::{
    DecorationKind, DecorationSpan, InlineObjectBoundaryAdjustment, InlineObjectPreferredStretch,
    InlineObjectPreferredStretchKind, InlineObjectSpan, LayoutInput, RubyKind, RubySpan,
    TiqianTextContent,
};
use tiqian::layout::paragraph_layout_engine::{
    ExplainableStubParagraphLayoutEngine, ParagraphLayoutEngine,
};
use tiqian::layout::annotation_geometry_stage::{
    resolve_annotation_geometry, AnnotationGeometryRequest, RubyFontGeometry,
};
use tiqian::layout::line_geometry_stage::ClusterMetricDecision;
use tiqian::layout::line_optimization::{LineCandidate, LineSolution};
use tiqian::font::font_metrics::FontMetricsRequest;
use tiqian::font::font_policy::{
    BaselinePolicy, FontMetricsPolicy, FontRole, LayoutFontMetrics, RawFontMetrics,
};
use tiqian::shaping::text_shaper::{
    ExplainableStubTextShaper, ShapingInput, ShapingResult, TextShaper,
};

struct InkBoundsTextShaper;

impl TextShaper for InkBoundsTextShaper {
    fn shape(&self, input: &ShapingInput) -> ShapingResult {
        let result = ExplainableStubTextShaper.shape(input);
        ShapingResult::with_decisions(
            result.clusters,
            result
                .glyph_runs
                .into_iter()
                .map(|run| {
                    GlyphRun::new(
                        run.range,
                        run.font_key,
                        run.glyphs
                            .into_iter()
                            .map(|glyph| {
                                Glyph::builder(glyph.id, glyph.cluster_range, glyph.advance)
                                    .x(glyph.x)
                                    .y(glyph.y)
                                    .render_font_key(glyph.render_font_key)
                                    .bounds(Some(Rect { left: 1.0, top: 2.0, right: 9.0, bottom: 10.0 }))
                                    .halt_advance(glyph.halt_advance)
                                    .halt_placement_x(glyph.halt_placement_x)
                                    .build()
                            })
                            .collect(),
                        run.advance,
                    )
                })
                .collect(),
            result.decisions,
        )
    }
}

struct MultiGlyphTextShaper;

impl TextShaper for MultiGlyphTextShaper {
    fn shape(&self, input: &ShapingInput) -> ShapingResult {
        let result = ExplainableStubTextShaper.shape(input);
        ShapingResult::with_decisions(
            result.clusters,
            result
                .glyph_runs
                .into_iter()
                .map(|run| {
                    GlyphRun::new(
                        run.range,
                        run.font_key,
                        vec![
                            Glyph::builder(1, input.range, 4.0)
                                .x(0.0)
                                .bounds(Some(Rect { left: 5.0, top: 5.0, right: 5.0, bottom: 5.0 }))
                                .build(),
                            Glyph::builder(2, input.range, 4.0)
                                .x(4.0)
                                .bounds(Some(Rect { left: 0.0, top: 0.0, right: 10.0, bottom: 10.0 }))
                                .build(),
                            Glyph::builder(3, input.range, 4.0)
                                .x(8.0)
                                .bounds(Some(Rect { left: 10.0, top: 10.0, right: 0.0, bottom: 0.0 }))
                                .build(),
                        ],
                        run.advance,
                    )
                })
                .collect(),
            result.decisions,
        )
    }
}

struct AlternatingGlyphTextShaper {
    call_count: Cell<i32>,
}

impl TextShaper for AlternatingGlyphTextShaper {
    fn shape(&self, input: &ShapingInput) -> ShapingResult {
        let result = ExplainableStubTextShaper.shape(input);
        let call_count = self.call_count.get() + 1;
        self.call_count.set(call_count);
        ShapingResult::with_decisions(
            result.clusters,
            result
                .glyph_runs
                .into_iter()
                .map(|run| {
                    let glyphs = if call_count % 2 == 0 {
                        vec![
                            Glyph::builder(1, input.range, 5.0)
                                .bounds(Some(Rect { left: 10.0, top: 10.0, right: 20.0, bottom: 20.0 }))
                                .build(),
                            Glyph::builder(2, input.range, 5.0)
                                .x(5.0)
                                .bounds(Some(Rect { left: 5.0, top: 5.0, right: 25.0, bottom: 25.0 }))
                                .build(),
                            Glyph::builder(3, input.range, 6.0)
                                .x(10.0)
                                .bounds(Some(Rect { left: 15.0, top: 15.0, right: 15.0, bottom: 15.0 }))
                                .build(),
                        ]
                    } else {
                        Vec::new()
                    };
                    GlyphRun::new(run.range, run.font_key, glyphs, run.advance)
                })
                .collect(),
            result.decisions,
        )
    }
}

#[test]
fn inline_object_decisions_with_preferred_stretch_and_fixed() {
    let text = Text::from("前置文本【嵌入对象】后置文本");
    let preferred_leading = InlineObjectBoundaryAdjustment::builder()
        .participates_in_uniform_stretch(true)
        .preferred_stretch(InlineObjectPreferredStretch::new(
            InlineObjectPreferredStretchKind::PunctuationTrailing, 10.0, 15.0,
        ))
        .prevents_line_break(true)
        .build();
    let preferred_trailing = InlineObjectBoundaryAdjustment::builder()
        .participates_in_uniform_stretch(true)
        .preferred_stretch(InlineObjectPreferredStretch::new(
            InlineObjectPreferredStretchKind::Relation, 10.0, 20.0,
        ))
        .shrink_capacity(3.0)
        .line_end_discardable_advance(2.0)
        .build();
    let result = ExplainableStubParagraphLayoutEngine::default().layout(
        LayoutInput::builder(TiqianTextContent::new(text), LayoutConstraints::with_defaults(300.0))
            .inline_objects(vec![
                InlineObjectSpan::new(TextRange::new(4, 5), 30.0, 12.0, 4.0, preferred_leading, preferred_trailing),
                InlineObjectSpan::with_fixed_boundaries(TextRange::new(6, 7), 20.0, 10.0, 2.0),
            ])
            .build(),
    );
    assert!(!result.lines.is_empty());
    assert_eq!(2, result.debug.inline_object_decisions.len());
    assert_eq!("PunctuationTrailing", result.debug.inline_object_decisions[0].leading_preferred_stretch_kind.as_deref().unwrap());
    assert_eq!("MeasurableOpaqueInlineObject", result.debug.inline_object_decisions[1].reason);
}

#[test]
fn decoration_decisions_emphasis_on_han_punctuation_and_western() {
    let text = Text::from("汉字，。English");
    let result = ExplainableStubParagraphLayoutEngine::default().layout(
        LayoutInput::builder(TiqianTextContent::new(text.clone()), LayoutConstraints::with_defaults(300.0))
            .decorations(vec![DecorationSpan { range: TextRange::new(0, text.utf16_len()), kind: DecorationKind::Emphasis }])
            .build(),
    );
    assert!(result.debug.decoration_decisions.iter().any(|decision| decision.applied));
    assert!(result.debug.decoration_decisions.iter().any(|decision| decision.reason == "clreq-no-dot-on-punctuation"));
    assert!(result.debug.decoration_decisions.iter().any(|decision| decision.reason == "no-dot-on-non-han"));
}

#[test]
fn decoration_segments_mourning_proper_noun_book_title_and_shortening() {
    let result = ExplainableStubParagraphLayoutEngine::default().layout(
        LayoutInput::builder(TiqianTextContent::new(Text::from("张三李四王五赵六钱七孙八周吴郑王")), LayoutConstraints::with_defaults(120.0))
            .decorations(vec![
                DecorationSpan { range: TextRange::new(0, 2), kind: DecorationKind::ProperNoun },
                DecorationSpan { range: TextRange::new(2, 4), kind: DecorationKind::ProperNoun },
                DecorationSpan { range: TextRange::new(4, 8), kind: DecorationKind::BookTitle },
                DecorationSpan { range: TextRange::new(8, 12), kind: DecorationKind::Mourning },
                DecorationSpan { range: TextRange::new(0, 16), kind: DecorationKind::Mourning },
            ])
            .build(),
    );
    assert!(result.debug.decoration_segments.iter().any(|segment| segment.kind == "ProperNoun"));
    assert!(result.debug.decoration_segments.iter().any(|segment| segment.kind == "BookTitle"));
    assert!(result.debug.decoration_segments.iter().any(|segment| segment.kind == "Mourning"));
    assert!(result.debug.decoration_segments.iter().any(|segment| segment.reason.contains("AdjacentInterlinearLineShortening")));
}

#[test]
fn decoration_segments_leading_and_trailing_blanks() {
    let text = Text::from("「开头」中文 English 混排【结束】");
    let result = ExplainableStubParagraphLayoutEngine::default().layout(
        LayoutInput::builder(TiqianTextContent::new(text.clone()), LayoutConstraints::with_defaults(150.0))
            .decorations(vec![DecorationSpan { range: TextRange::new(0, text.utf16_len()), kind: DecorationKind::ProperNoun }])
            .build(),
    );
    assert!(!result.debug.decoration_segments.is_empty());
}

#[test]
fn ruby_decisions_pinyin_single_and_split_lines() {
    let result = ExplainableStubParagraphLayoutEngine::default().layout(
        LayoutInput::builder(TiqianTextContent::new(Text::from("这是一个很长很长的段落用于测试拼音行间注跨行")), LayoutConstraints::with_defaults(100.0))
            .ruby_spans(vec![
                RubySpan::builder(TextRange::new(0, 2), Text::from("zhèshì")).locale(Some("zh-Latn".to_owned())).build(),
                RubySpan::new(TextRange::new(2, 6), Text::from("yīgehěncháng")),
                RubySpan::new(TextRange::new(6, 12), Text::from("chángdeduànluò")),
            ])
            .build(),
    );
    assert!(result.lines.len() > 1);
    assert_eq!(3, result.debug.ruby_decisions.len());
    assert_eq!("zh-Latn", result.debug.ruby_decisions[0].locale);
}

#[test]
fn bopomofo_decisions_all_tones_and_symbol_counts() {
    let ruby_spans = ["˙ㄅ", "˙ㄅㄆ", "˙ㄅㄆㄇ", "ㄅˊ", "ㄅㄆˊ", "ㄅㄆㄇˊ", "ㄅˇ", "ㄅㄆˇ", "ㄅㄆㄇˇ", "ㄅˋ", "ㄅㄆˋ", "ㄅㄆㄇˋ", "ㄅ", "ㄅㄆ", "ㄅㄆㄇ"]
        .into_iter()
        .enumerate()
        .map(|(index, reading)| {
            RubySpan::builder(TextRange::new(index as i32, index as i32 + 1), Text::from(reading))
                .kind(RubyKind::Bopomofo)
                .locale(if index == 0 { Some("zh-Bopo".to_owned()) } else { None })
                .build()
        })
        .collect();
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.text_shaper = Box::new(InkBoundsTextShaper);
    let result = engine.layout(
        LayoutInput::builder(TiqianTextContent::new(Text::from("一二三四五六七八九十甲乙丙丁戊己庚辛")), LayoutConstraints::with_defaults(300.0))
            .ruby_spans(ruby_spans)
            .build(),
    );
    assert_eq!(15, result.debug.bopomofo_decisions.len());
    assert!(result.debug.bopomofo_decisions.iter().any(|decision| decision.placements.len() == 4));
}

#[test]
fn direct_resolve_annotation_geometry_fallback_branches() {
    let engine = ExplainableStubParagraphLayoutEngine::default();
    let input = LayoutInput::builder(
        TiqianTextContent::new(Text::from("汉字，测试English")),
        LayoutConstraints::with_defaults(300.0),
    )
    .decorations(vec![
        DecorationSpan { range: TextRange::new(0, 2), kind: DecorationKind::Emphasis },
        DecorationSpan { range: TextRange::new(2, 3), kind: DecorationKind::Emphasis },
        DecorationSpan { range: TextRange::new(5, 12), kind: DecorationKind::Emphasis },
        DecorationSpan { range: TextRange::new(0, 4), kind: DecorationKind::ProperNoun },
    ])
    .build();
    let clusters = vec![
        Cluster::with_display_text(TextRange::new(0, 2), Text::from("汉字"), Text::from("汉字"), "k".to_owned(), 32.0),
        Cluster::with_display_text(TextRange::new(2, 3), Text::from("，"), Text::from("，"), "k".to_owned(), 16.0),
        Cluster::with_display_text(TextRange::new(3, 5), Text::from("测试"), Text::from("测试"), "k".to_owned(), 32.0),
        Cluster::with_display_text(TextRange::new(5, 12), Text::from("English"), Text::from("English"), "k".to_owned(), 56.0),
    ];
    let line_solution = LineSolution::new(vec![
        LineCandidate::new(IntRange::new(0, 2), TextRange::new(0, 5), 80.0, 80.0),
        LineCandidate::new(IntRange::new(3, 3), TextRange::new(5, 12), 56.0, 56.0),
    ]);
    let lines = vec![
        LineBox::builder(TextRange::new(0, 5), IntRange::new(0, 2), 16.0, 0.0, 20.0, 80.0, 80.0, 80.0)
            .end_reason(LineEndReason::AutoWrap)
            .build(),
        LineBox::builder(TextRange::new(5, 12), IntRange::new(3, 3), 36.0, 20.0, 40.0, 56.0, 56.0, 56.0)
            .end_reason(LineEndReason::MandatoryBreak)
            .build(),
    ];
    let hanzi = RubySpan::builder(TextRange::new(0, 2), Text::from("hànzì")).locale(Some("zh-Latn".to_owned())).build();
    let ceshi = RubySpan::new(TextRange::new(3, 5), Text::from("cèshì"));
    let geometry = ClusterGeometryDecisionInfo::builder(
        TextRange::new(0, 2), Text::from("汉字"), Text::from("汉字"), 32.0, 32.0,
        4.0, 2.0, 4.0, 2.0, 0.0, 32.0, "test".to_owned(), "test".to_owned(),
    ).build();
    let leading = InlineObjectBoundaryAdjustment::builder()
        .preferred_stretch(InlineObjectPreferredStretch::new(InlineObjectPreferredStretchKind::PunctuationTrailing, 10.0, 15.0))
        .build();
    let trailing = InlineObjectBoundaryAdjustment::builder()
        .preferred_stretch(InlineObjectPreferredStretch::new(InlineObjectPreferredStretchKind::Relation, 10.0, 20.0))
        .build();
    let inline_objects = HashMap::from([
        (0, InlineObjectSpan::with_leading_boundary(TextRange::new(0, 2), 32.0, 12.0, 4.0, leading)),
        (3, InlineObjectSpan::with_trailing_boundary(TextRange::new(5, 12), 56.0, 12.0, 4.0, trailing)),
        (99, InlineObjectSpan::with_fixed_boundaries(TextRange::new(99, 100), 10.0, 8.0, 2.0)),
    ]);
    let ruby_geometry = HashMap::from([
        (hanzi.clone(), RubyFontGeometry { width: 20.0, ascent: 8.0, descent: 2.0, required_extent: 10.0, glyphs: Vec::new() }),
        (ceshi.clone(), RubyFontGeometry { width: 20.0, ascent: 8.0, descent: 2.0, required_extent: 10.0, glyphs: Vec::new() }),
    ]);
    let auto_space = vec![
        AutoSpaceDecisionInfo { cluster_range: TextRange::new(0, 2), side: "leading".to_owned(), boundary_role: "Wide".to_owned(), mode: "Normal".to_owned(), characters_affected: 1, reduction_per_char: 0.0, total_reduction: 0.0, reason: "test".to_owned() },
        AutoSpaceDecisionInfo { cluster_range: TextRange::new(2, 3), side: "leading".to_owned(), boundary_role: "Wide".to_owned(), mode: "Normal".to_owned(), characters_affected: 1, reduction_per_char: 0.0, total_reduction: 0.0, reason: "test".to_owned() },
        AutoSpaceDecisionInfo { cluster_range: TextRange::new(3, 5), side: "trailing".to_owned(), boundary_role: "Wide".to_owned(), mode: "Normal".to_owned(), characters_affected: 1, reduction_per_char: 0.0, total_reduction: 0.0, reason: "test".to_owned() },
        AutoSpaceDecisionInfo { cluster_range: TextRange::new(5, 12), side: "trailing".to_owned(), boundary_role: "Wide".to_owned(), mode: "Normal".to_owned(), characters_affected: 1, reduction_per_char: 0.0, total_reduction: 0.0, reason: "test".to_owned() },
    ];
    let result = resolve_annotation_geometry(AnnotationGeometryRequest {
        input: &input,
        font_size: 16.0,
        inline_object_by_cluster_index: &inline_objects,
        line_solution: &line_solution,
        clreq_profile: &engine.clreq_profile_resolver.resolve(&input.profile_id),
        geometry_decisions: &[geometry],
        auto_space_decisions: &auto_space,
        visible_line_ranges: &[IntRange::new(0, 2), IntRange::new(3, 3)],
        lines: &lines,
        final_clusters: &clusters,
        cluster_roles: &[FontRole::CjkText, FontRole::CjkPunctuation, FontRole::CjkText, FontRole::LatinText],
        justify_delta_by_cluster: &HashMap::from([(0, 2.0)]),
        ruby_and_bopomofo_spread: &HashMap::from([(0, 4.0)]),
        metric_decisions: &[],
        pinyin_spans: &[hanzi.clone(), ceshi.clone()],
        natural_clusters: &clusters,
        ruby_font_geometry_by_span: &ruby_geometry,
        ruby_stack_gap: 0.0,
        base_ascent: 16.0,
        ruby_font_size: 8.0,
        ruby_font_weight: 400,
        base_descent: 4.0,
        bopomofo_font_weight_at: &|_| 400,
        fallback_resolver: engine.fallback_resolver.as_ref(),
        text_shaper: engine.text_shaper.as_ref(),
    });
    assert_eq!(3, result.inline_object_decisions.len());
    assert_eq!(-1, result.inline_object_decisions.last().unwrap().line_index);
    let metric = ClusterMetricDecision {
        range: TextRange::new(0, 2),
        source_text: Text::from("汉字"),
        request: FontMetricsRequest::new("k".to_owned(), 16.0, FontRole::CjkText, "zh-Hans".to_owned()),
        raw_metrics: RawFontMetrics::new(14.0, 4.0),
        layout_metrics: LayoutFontMetrics::new(14.0, 4.0, 0.0, FontMetricsPolicy::Raw, BaselinePolicy::Alphabetic),
    };
    let second = resolve_annotation_geometry(AnnotationGeometryRequest {
        input: &input,
        font_size: 16.0,
        inline_object_by_cluster_index: &HashMap::new(),
        line_solution: &line_solution,
        clreq_profile: &engine.clreq_profile_resolver.resolve(&input.profile_id),
        geometry_decisions: &[],
        auto_space_decisions: &[],
        visible_line_ranges: &[IntRange::new(0, 2), IntRange::new(3, 3)],
        lines: &lines,
        final_clusters: &clusters,
        cluster_roles: &[FontRole::CjkText, FontRole::CjkPunctuation, FontRole::CjkText, FontRole::LatinText],
        justify_delta_by_cluster: &HashMap::new(),
        ruby_and_bopomofo_spread: &HashMap::new(),
        metric_decisions: &[metric],
        pinyin_spans: &[RubySpan::new(TextRange::new(0, 2), Text::from("hànzì"))],
        natural_clusters: &clusters,
        ruby_font_geometry_by_span: &HashMap::from([(
            RubySpan::new(TextRange::new(0, 2), Text::from("hànzì")),
            RubyFontGeometry { width: 20.0, ascent: 8.0, descent: 2.0, required_extent: 10.0, glyphs: Vec::new() },
        )]),
        ruby_stack_gap: 0.0,
        base_ascent: 16.0,
        ruby_font_size: 8.0,
        ruby_font_weight: 400,
        base_descent: 4.0,
        bopomofo_font_weight_at: &|_| 400,
        fallback_resolver: engine.fallback_resolver.as_ref(),
        text_shaper: engine.text_shaper.as_ref(),
    });
    assert!(!second.ruby_decisions.is_empty());
}

#[test]
fn direct_resolve_annotation_geometry_empty_line_ranges_and_gap_at_line_edges() {
    let engine = ExplainableStubParagraphLayoutEngine::default();
    let input = LayoutInput::builder(
        TiqianTextContent::new(Text::from("汉字，测试English")),
        LayoutConstraints::with_defaults(300.0),
    )
    .decorations(vec![
        DecorationSpan { range: TextRange::new(0, 2), kind: DecorationKind::Emphasis },
        DecorationSpan { range: TextRange::new(0, 5), kind: DecorationKind::ProperNoun },
    ])
    .build();
    let clusters = vec![
        Cluster::with_display_text(TextRange::new(0, 2), Text::from("汉字"), Text::from("汉字"), "k".to_owned(), 32.0),
        Cluster::with_display_text(TextRange::new(2, 3), Text::from("，"), Text::from("，"), "k".to_owned(), 16.0),
        Cluster::with_display_text(TextRange::new(3, 5), Text::from("测试"), Text::from("测试"), "k".to_owned(), 32.0),
        Cluster::with_display_text(TextRange::new(5, 12), Text::from("English"), Text::from("English"), "k".to_owned(), 56.0),
    ];
    let line_solution = LineSolution::new(vec![
        LineCandidate::new(IntRange::EMPTY, TextRange::new(0, 0), 0.0, 0.0),
        LineCandidate::new(IntRange::new(0, 2), TextRange::new(0, 5), 80.0, 80.0),
    ]);
    let lines = vec![
        LineBox::builder(TextRange::new(0, 0), IntRange::EMPTY, 0.0, 0.0, 20.0, 0.0, 0.0, 0.0)
            .end_reason(LineEndReason::AutoWrap)
            .build(),
        LineBox::builder(TextRange::new(0, 5), IntRange::new(0, 2), 16.0, 0.0, 20.0, 80.0, 80.0, 80.0)
            .end_reason(LineEndReason::AutoWrap)
            .build(),
    ];
    let geometry = ClusterGeometryDecisionInfo::builder(
        TextRange::new(0, 2), Text::from("汉字"), Text::from("汉字"), 32.0, 32.0,
        4.0, 2.0, 4.0, 2.0, 0.0, 32.0, "test".to_owned(), "test".to_owned(),
    )
    .build();
    let metric_cjk = ClusterMetricDecision {
        range: TextRange::new(0, 2),
        source_text: Text::from("汉字"),
        request: FontMetricsRequest::new("k".to_owned(), 24.0, FontRole::CjkText, "zh-Hans".to_owned()),
        raw_metrics: RawFontMetrics::new(18.0, 6.0),
        layout_metrics: LayoutFontMetrics::new(18.0, 6.0, 0.0, FontMetricsPolicy::Raw, BaselinePolicy::Alphabetic),
    };
    let metric_punctuation = ClusterMetricDecision {
        range: TextRange::new(2, 3),
        source_text: Text::from("，"),
        request: FontMetricsRequest::new("k".to_owned(), 24.0, FontRole::CjkPunctuation, "zh-Hans".to_owned()),
        raw_metrics: RawFontMetrics::new(18.0, 6.0),
        layout_metrics: LayoutFontMetrics::new(18.0, 6.0, 0.0, FontMetricsPolicy::Raw, BaselinePolicy::Alphabetic),
    };
    let leading = InlineObjectBoundaryAdjustment::builder()
        .preferred_stretch(InlineObjectPreferredStretch::new(InlineObjectPreferredStretchKind::Relation, 5.0, 10.0))
        .build();
    let trailing = InlineObjectBoundaryAdjustment::builder()
        .preferred_stretch(InlineObjectPreferredStretch::new(InlineObjectPreferredStretchKind::BinaryOperator, 5.0, 10.0))
        .build();
    let inline_objects = HashMap::from([
        (0, InlineObjectSpan::with_leading_boundary(TextRange::new(0, 2), 32.0, 16.0, 4.0, leading)),
        (99, InlineObjectSpan::with_trailing_boundary(TextRange::new(15, 17), 32.0, 16.0, 4.0, trailing)),
    ]);
    let hanzi = RubySpan::new(TextRange::new(0, 2), Text::from("hànzì"));
    let chu = RubySpan::new(TextRange::new(2, 3), Text::from("chù"));
    let ceshi = RubySpan::new(TextRange::new(3, 5), Text::from("cèshì"));
    let ruby_geometry = HashMap::from([
        (hanzi.clone(), RubyFontGeometry { width: 20.0, ascent: 8.0, descent: 2.0, required_extent: 10.0, glyphs: Vec::new() }),
        (chu.clone(), RubyFontGeometry { width: 10.0, ascent: 8.0, descent: 2.0, required_extent: 10.0, glyphs: Vec::new() }),
        (ceshi.clone(), RubyFontGeometry { width: 20.0, ascent: 8.0, descent: 2.0, required_extent: 10.0, glyphs: Vec::new() }),
    ]);
    let auto_space = vec![
        AutoSpaceDecisionInfo { cluster_range: TextRange::new(0, 2), side: "leading".to_owned(), boundary_role: "Wide".to_owned(), mode: "Normal".to_owned(), characters_affected: 1, reduction_per_char: 0.0, total_reduction: 0.0, reason: "test".to_owned() },
        AutoSpaceDecisionInfo { cluster_range: TextRange::new(2, 3), side: "leading".to_owned(), boundary_role: "Wide".to_owned(), mode: "Normal".to_owned(), characters_affected: 1, reduction_per_char: 0.0, total_reduction: 0.0, reason: "test".to_owned() },
        AutoSpaceDecisionInfo { cluster_range: TextRange::new(2, 3), side: "trailing".to_owned(), boundary_role: "Wide".to_owned(), mode: "Normal".to_owned(), characters_affected: 1, reduction_per_char: 0.0, total_reduction: 0.0, reason: "test".to_owned() },
        AutoSpaceDecisionInfo { cluster_range: TextRange::new(3, 5), side: "trailing".to_owned(), boundary_role: "Wide".to_owned(), mode: "Normal".to_owned(), characters_affected: 1, reduction_per_char: 0.0, total_reduction: 0.0, reason: "test".to_owned() },
    ];
    let result = resolve_annotation_geometry(AnnotationGeometryRequest {
        input: &input,
        font_size: 16.0,
        inline_object_by_cluster_index: &inline_objects,
        line_solution: &line_solution,
        clreq_profile: &engine.clreq_profile_resolver.resolve(&input.profile_id),
        geometry_decisions: &[geometry],
        auto_space_decisions: &auto_space,
        visible_line_ranges: &[IntRange::EMPTY, IntRange::new(0, 2)],
        lines: &lines,
        final_clusters: &clusters,
        cluster_roles: &[FontRole::CjkText, FontRole::CjkPunctuation, FontRole::CjkText, FontRole::LatinText],
        justify_delta_by_cluster: &HashMap::new(),
        ruby_and_bopomofo_spread: &HashMap::new(),
        metric_decisions: &[metric_cjk, metric_punctuation],
        pinyin_spans: &[hanzi, chu, ceshi],
        natural_clusters: &clusters,
        ruby_font_geometry_by_span: &ruby_geometry,
        ruby_stack_gap: 0.0,
        base_ascent: 16.0,
        ruby_font_size: 8.0,
        ruby_font_weight: 400,
        base_descent: 4.0,
        bopomofo_font_weight_at: &|_| 400,
        fallback_resolver: engine.fallback_resolver.as_ref(),
        text_shaper: engine.text_shaper.as_ref(),
    });
    assert!(!result.decoration_decisions.is_empty());
}

#[test]
fn bopomofo_decisions_multi_glyph_min_max_and_empty_placements() {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.text_shaper = Box::new(MultiGlyphTextShaper);
    let result = engine.layout(
        LayoutInput::builder(TiqianTextContent::new(Text::from("一二三四五六七八")), LayoutConstraints::with_defaults(300.0))
            .ruby_spans(vec![
                RubySpan::builder(TextRange::new(0, 2), Text::from("ㄅㄆˊ")).kind(RubyKind::Bopomofo).locale(Some("zh-Bopo".to_owned())).build(),
                RubySpan::with_kind(TextRange::new(2, 3), Text::from(" "), RubyKind::Bopomofo),
                RubySpan::with_kind(TextRange::new(3, 4), Text::from("ㄅ"), RubyKind::Bopomofo),
                RubySpan::with_kind(TextRange::new(4, 5), Text::from("˙ㄅ"), RubyKind::Bopomofo),
                RubySpan::with_kind(TextRange::new(5, 6), Text::from("ㄅˇ"), RubyKind::Bopomofo),
                RubySpan::with_kind(TextRange::new(6, 7), Text::from("ㄅˋ"), RubyKind::Bopomofo),
            ])
            .build(),
    );
    assert!(!result.lines.is_empty());
}

#[test]
fn bopomofo_and_decoration_leading_blank_exhaustive_branches() {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.text_shaper = Box::new(AlternatingGlyphTextShaper { call_count: Cell::new(0) });
    let input = LayoutInput::builder(TiqianTextContent::new(Text::from("中文English")), LayoutConstraints::with_defaults(500.0))
        .decorations(vec![
            DecorationSpan { range: TextRange::new(0, 7), kind: DecorationKind::ProperNoun },
            DecorationSpan { range: TextRange::new(2, 7), kind: DecorationKind::ProperNoun },
        ])
        .ruby_spans(vec![
            RubySpan::builder(TextRange::new(0, 1), Text::from("ㄅ")).kind(RubyKind::Bopomofo).locale(None).build(),
            RubySpan::builder(TextRange::new(1, 2), Text::from("ㄆ")).kind(RubyKind::Bopomofo).locale(Some("zh-TW".to_owned())).build(),
            RubySpan::with_kind(TextRange::new(0, 1), Text::from(""), RubyKind::Bopomofo),
        ])
        .build();
    let wide = engine.layout(input.clone());
    assert_eq!(2, wide.debug.bopomofo_decisions.len());
    let narrow = engine.layout(
        LayoutInput::builder(input.content, LayoutConstraints::with_defaults(30.0))
            .decorations(input.decorations)
            .ruby_spans(input.ruby_spans)
            .build(),
    );
    assert!(narrow.lines.len() > 1);
}

#[test]
fn bopomofo_over_latin_clusters_covers_cross_metric_lookup() {
    let result = ExplainableStubParagraphLayoutEngine::default().layout(
        LayoutInput::builder(TiqianTextContent::new(Text::from("中文English")), LayoutConstraints::with_defaults(500.0))
            .ruby_spans(vec![
                RubySpan::builder(TextRange::new(2, 3), Text::from("ㄅ")).kind(RubyKind::Bopomofo).locale(None).build(),
                RubySpan::builder(TextRange::new(3, 4), Text::from("ㄆ")).kind(RubyKind::Bopomofo).locale(Some("zh-TW".to_owned())).build(),
            ])
            .build(),
    );
    assert_eq!(2, result.debug.bopomofo_decisions.len());
}
