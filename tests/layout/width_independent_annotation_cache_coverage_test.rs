use std::sync::Arc;

use tiqian::common::{HashMap, HashSet};
use tiqian::clreq::clreq_profile::{ClreqProfile, ClreqProfileResolver};
use tiqian::core::geometry::{scalar_offset, text_range, LayoutConstraints};
use tiqian::core::layout_model::{Cluster, Glyph, GlyphRun};
use tiqian::core::text::Text;
use tiqian::core::text_model::{
    DecorationKind, DecorationSpan, LastLineAlignment, LayoutInput, LineBreakPolicy,
    InlineBoxOuterSpacing, InlineBoxSpan, LineBreakSpan, LineLengthGrid, ParagraphStyle,
    InlineAttachment, InlineObjectBoundaryAdjustment, InlineObjectPreferredStretch,
    InlineObjectPreferredStretchKind, InlineObjectSpan, RubySpan, TextSpan, TextStyle,
    RubyKind, TiqianTextContent,
};
use tiqian::core::units::Ic;
use tiqian::layout::paragraph_layout_engine::ParagraphLayoutEngine;
use tiqian::layout::paragraph_layout_engine::ExplainableStubParagraphLayoutEngine;
use tiqian::layout::progressive_break_decisions::ProgressiveBreakTier;
use tiqian::layout::width_independent_annotation_cache::{
    LruWidthIndependentAnnotationCache, WidthIndependentAnnotationCache, containing_items,
    first_contained_item, build_paragraph_layout_prep, prepare_width_independent_annotation,
    to_width_independent_annotation_key,
};
use tiqian::shaping::text_shaper::{ShapingInput, ShapingResult, TextShaper};

#[test]
fn lru_cache_update_existing_key_and_clear() {
    let engine = ExplainableStubParagraphLayoutEngine::default();
    let input = LayoutInput::builder(
        TiqianTextContent::new(Text::from("测试缓存")),
        LayoutConstraints::with_defaults(300.0),
    )
    .build();
    let annotation = Arc::new(prepare_width_independent_annotation(
        &input,
        &HashMap::new(),
        engine.clreq_profile_resolver.as_ref(),
        engine.font_role_classifier.as_ref(),
        engine.fallback_resolver.as_ref(),
        engine.font_metrics_resolver.as_ref(),
        &engine.quote_pair_analyzer,
        engine.text_shaper.as_ref(),
        engine.hyphenator,
    ));
    let key = to_width_independent_annotation_key(&input, HashMap::new());
    let mut cache = LruWidthIndependentAnnotationCache::new(2);

    cache.put(key.clone(), annotation.clone());
    assert_eq!(1, cache.size());
    assert_eq!(annotation.text, cache.get(&key).unwrap().text);

    cache.put(key.clone(), annotation.clone());
    assert_eq!(1, cache.size());
    assert_eq!(annotation.text, cache.get(&key).unwrap().text);

    let mut input2 = input.clone();
    input2.text_style = TextStyle::builder().font_size(20.0).build();
    let key2 = to_width_independent_annotation_key(&input2, HashMap::new());
    cache.put(key2.clone(), annotation.clone());
    assert_eq!(2, cache.size());

    let mut input3 = input;
    input3.text_style = TextStyle::builder().font_size(30.0).build();
    let key3 = to_width_independent_annotation_key(&input3, HashMap::new());
    cache.put(key3.clone(), annotation);
    assert_eq!(2, cache.size());
    assert!(cache.get(&key).is_none());
    assert!(cache.get(&key2).is_some());
    assert!(cache.get(&key3).is_some());

    cache.clear();
    assert_eq!(0, cache.size());
    assert!(cache.get(&key2).is_none());
    assert!(cache.get(&key3).is_none());
}

#[test]
fn containing_items_and_first_contained_item_branches() {
    let clusters = vec![
        Cluster::with_display_text(
            text_range(0, 2), Text::from("aa"), Text::from("aa"), "k".to_owned(), 10.0,
        ),
        Cluster::with_display_text(
            text_range(2, 5), Text::from("bbb"), Text::from("bbb"), "k".to_owned(), 15.0,
        ),
        Cluster::with_display_text(
            text_range(5, 7), Text::from("cc"), Text::from("cc"), "k".to_owned(), 10.0,
        ),
        Cluster::with_display_text(
            text_range(7, 9), Text::from("dd"), Text::from("dd"), "k".to_owned(), 10.0,
        ),
    ];
    let items = vec![
        text_range(0, 2),
        text_range(1, 4),
        text_range(5, 8),
        text_range(10, 12),
    ];

    assert_eq!(
        vec![Some(0), None, Some(2), None],
        containing_items(&clusters, &items, |item| *item),
    );
    assert_eq!(
        vec![Some(0), None, None, None],
        first_contained_item(&clusters, &items, |item| *item),
    );
}

#[test]
fn line_length_grid_body_alignment_branches() {
    for (alignment, expected_offset) in [
        (LastLineAlignment::Start, 0.0),
        (LastLineAlignment::Center, 2.0),
        (LastLineAlignment::End, 4.0),
    ] {
        let engine = ExplainableStubParagraphLayoutEngine::default();
        let input = LayoutInput::builder(
            TiqianTextContent::new(Text::from("一二三四五六七八九十")),
            LayoutConstraints::with_defaults(100.0),
        )
        .text_style(TextStyle::builder().font_size(16.0).build())
        .paragraph_style(
            ParagraphStyle::builder()
                .line_length_grid(LineLengthGrid::new(true, Some(alignment)))
                .build(),
        )
        .build();
        let annotation = prepare_width_independent_annotation(
            &input,
            &HashMap::new(),
            engine.clreq_profile_resolver.as_ref(),
            engine.font_role_classifier.as_ref(),
            engine.fallback_resolver.as_ref(),
            engine.font_metrics_resolver.as_ref(),
            &engine.quote_pair_analyzer,
            engine.text_shaper.as_ref(),
            engine.hyphenator,
        );
        let prep = build_paragraph_layout_prep(
            &input,
            &annotation,
            &HashMap::new(),
            engine.text_shaper.as_ref(),
            engine.hyphenator,
            &engine.punctuation_atom_builder,
            &engine.punctuation_spacing_compressor,
        );
        assert!((prep.grid_body_offset - expected_offset).abs() < 0.001);
    }
}

#[test]
fn dynamic_shaping_triggers_and_emphasis_italic() {
    let engine = ExplainableStubParagraphLayoutEngine::default();
    for (input, rejected) in [
        (
            LayoutInput::builder(
                TiqianTextContent::new(Text::from("中文正文排版")),
                LayoutConstraints::with_defaults(500.0),
            )
            .build(),
            HashMap::new(),
        ),
        (
            LayoutInput::builder(
                TiqianTextContent::builder(Text::from("Hello World with English Words"))
                    .line_break_spans(vec![LineBreakSpan {
                        range: text_range(0, 11),
                        policy: LineBreakPolicy::ProgressiveTechnical,
                    }])
                    .build(),
                LayoutConstraints::with_defaults(50.0),
            )
            .decorations(vec![
                DecorationSpan {
                    range: text_range(0, 5),
                    kind: DecorationKind::Emphasis,
                },
                DecorationSpan {
                    range: text_range(6, 11),
                    kind: DecorationKind::ProperNoun,
                },
            ])
            .build(),
            HashMap::from([(
                text_range(0, 11),
                tiqian::common::HashSet::from([ProgressiveBreakTier::Structural]),
            )]),
        ),
        (
            LayoutInput::builder(
                TiqianTextContent::new(Text::from("VeryLongEnglishWordThatExceedsMeasure")),
                LayoutConstraints::with_defaults(30.0),
            )
            .build(),
            HashMap::new(),
        ),
    ] {
        let annotation = prepare_width_independent_annotation(
            &input,
            &rejected,
            engine.clreq_profile_resolver.as_ref(),
            engine.font_role_classifier.as_ref(),
            engine.fallback_resolver.as_ref(),
            engine.font_metrics_resolver.as_ref(),
            &engine.quote_pair_analyzer,
            engine.text_shaper.as_ref(),
            engine.hyphenator,
        );
        let prep = build_paragraph_layout_prep(
            &input,
            &annotation,
            &rejected,
            engine.text_shaper.as_ref(),
            engine.hyphenator,
            &engine.punctuation_atom_builder,
            &engine.punctuation_spacing_compressor,
        );
        assert!(!prep.clusters.is_empty());
    }
}

#[test]
#[should_panic(expected = "Conflicting OpenType features")]
fn conflicting_open_type_features_throws() {
    struct ConflictingFeatureShaper;

    impl TextShaper for ConflictingFeatureShaper {
        fn shape(&self, input: &ShapingInput) -> ShapingResult {
            let cluster = Cluster::with_display_text(
                input.range,
                input.text.slice_text(input.range),
                input.display_text.clone(),
                "test".to_owned(),
                16.0,
            );
            ShapingResult::new(
                vec![cluster],
                vec![
                    GlyphRun::with_open_type_features(
                        input.range,
                        "test".to_owned(),
                        vec![Glyph::builder(1, input.range, 8.0).build()],
                        8.0,
                        vec!["feat1".to_owned()],
                    ),
                    GlyphRun::with_open_type_features(
                        input.range,
                        "test".to_owned(),
                        vec![Glyph::builder(2, input.range, 8.0).x(8.0).build()],
                        8.0,
                        vec!["feat2".to_owned()],
                    ),
                ],
            )
        }
    }

    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.text_shaper = Box::new(ConflictingFeatureShaper);
    let input = LayoutInput::builder(
        TiqianTextContent::new(Text::from("测试")),
        LayoutConstraints::with_defaults(300.0),
    )
    .build();
    let annotation = prepare_width_independent_annotation(
        &input,
        &HashMap::new(),
        engine.clreq_profile_resolver.as_ref(),
        engine.font_role_classifier.as_ref(),
        engine.fallback_resolver.as_ref(),
        engine.font_metrics_resolver.as_ref(),
        &engine.quote_pair_analyzer,
        engine.text_shaper.as_ref(),
        engine.hyphenator,
    );
    build_paragraph_layout_prep(
        &input,
        &annotation,
        &HashMap::new(),
        engine.text_shaper.as_ref(),
        engine.hyphenator,
        &engine.punctuation_atom_builder,
        &engine.punctuation_spacing_compressor,
    );
}

#[test]
fn verbatim_ranges_and_auto_space_decisions() {
    let engine = ExplainableStubParagraphLayoutEngine::default();
    let text = Text::from("中文 English 混排测试 12345");
    let input = LayoutInput::builder(
        TiqianTextContent::builder(text)
            .auto_space_suppressed_ranges(vec![text_range(0, 15)])
            .build(),
        LayoutConstraints::with_defaults(300.0),
    )
    .inline_boxes(vec![InlineBoxSpan::with_all(
        text_range(2, 9),
        0.0,
        0.0,
        InlineBoxOuterSpacing::Narrow,
    )])
    .build();
    let annotation = prepare_width_independent_annotation(
        &input,
        &HashMap::new(),
        engine.clreq_profile_resolver.as_ref(),
        engine.font_role_classifier.as_ref(),
        engine.fallback_resolver.as_ref(),
        engine.font_metrics_resolver.as_ref(),
        &engine.quote_pair_analyzer,
        engine.text_shaper.as_ref(),
        engine.hyphenator,
    );
    let prep = build_paragraph_layout_prep(
        &input,
        &annotation,
        &HashMap::new(),
        engine.text_shaper.as_ref(),
        engine.hyphenator,
        &engine.punctuation_atom_builder,
        &engine.punctuation_spacing_compressor,
    );
    assert!(!prep.clusters.is_empty());
}

#[test]
fn ruby_spread_accumulation_and_edges() {
    let engine = ExplainableStubParagraphLayoutEngine::default();
    let input = LayoutInput::builder(
        TiqianTextContent::builder(Text::from("中文测试段落"))
            .source_boundaries(HashSet::from([scalar_offset(1), scalar_offset(2), scalar_offset(3), scalar_offset(4), scalar_offset(5)]))
            .build(),
        LayoutConstraints::with_defaults(300.0),
    )
    .ruby_spans(vec![
        RubySpan::new(text_range(0, 2), Text::from("zhōngwén")),
        RubySpan::new(text_range(2, 4), Text::from("cèshìchángdà")),
        RubySpan::new(text_range(4, 6), Text::from("duànluòchángdà")),
        RubySpan::new(text_range(99, 100), Text::from("invalid")),
    ])
    .build();
    let annotation = prepare_width_independent_annotation(
        &input,
        &HashMap::new(),
        engine.clreq_profile_resolver.as_ref(),
        engine.font_role_classifier.as_ref(),
        engine.fallback_resolver.as_ref(),
        engine.font_metrics_resolver.as_ref(),
        &engine.quote_pair_analyzer,
        engine.text_shaper.as_ref(),
        engine.hyphenator,
    );
    let prep = build_paragraph_layout_prep(
        &input,
        &annotation,
        &HashMap::new(),
        engine.text_shaper.as_ref(),
        engine.hyphenator,
        &engine.punctuation_atom_builder,
        &engine.punctuation_spacing_compressor,
    );
    assert!(!prep.ruby_and_bopomofo_spread.is_empty());
}

#[test]
fn ruby_spread_second_visit_and_zero_first_cluster() {
    let engine = ExplainableStubParagraphLayoutEngine::default();
    let input = LayoutInput::builder(
        TiqianTextContent::builder(Text::from("一二三四五六七八"))
            .source_boundaries(HashSet::from([scalar_offset(0), scalar_offset(1), scalar_offset(2), scalar_offset(3), scalar_offset(4), scalar_offset(5), scalar_offset(6), scalar_offset(7)]))
            .build(),
        LayoutConstraints::with_defaults(300.0),
    )
    .ruby_spans(vec![
        RubySpan::new(text_range(0, 1), Text::from("chángdàchángdà")),
        RubySpan::new(text_range(0, 1), Text::from("chángdàchángdà")),
        RubySpan::new(text_range(2, 3), Text::from("chángdàchángdàchángdà")),
        RubySpan::new(text_range(2, 3), Text::from("chángdàchángdàchángdà")),
    ])
    .build();
    let annotation = prepare_width_independent_annotation(
        &input,
        &HashMap::new(),
        engine.clreq_profile_resolver.as_ref(),
        engine.font_role_classifier.as_ref(),
        engine.fallback_resolver.as_ref(),
        engine.font_metrics_resolver.as_ref(),
        &engine.quote_pair_analyzer,
        engine.text_shaper.as_ref(),
        engine.hyphenator,
    );
    let prep = build_paragraph_layout_prep(
        &input,
        &annotation,
        &HashMap::new(),
        engine.text_shaper.as_ref(),
        engine.hyphenator,
        &engine.punctuation_atom_builder,
        &engine.punctuation_spacing_compressor,
    );
    assert!(!prep.ruby_and_bopomofo_spread.is_empty());
}

#[test]
fn paired_punctuation_with_zero_capacity() {
    let engine = ExplainableStubParagraphLayoutEngine::default();
    let input = LayoutInput::builder(
        TiqianTextContent::new(Text::from("（括号）")),
        LayoutConstraints::with_defaults(300.0),
    )
    .build();
    let annotation = prepare_width_independent_annotation(
        &input,
        &HashMap::new(),
        engine.clreq_profile_resolver.as_ref(),
        engine.font_role_classifier.as_ref(),
        engine.fallback_resolver.as_ref(),
        engine.font_metrics_resolver.as_ref(),
        &engine.quote_pair_analyzer,
        engine.text_shaper.as_ref(),
        engine.hyphenator,
    );
    let prep = build_paragraph_layout_prep(
        &input,
        &annotation,
        &HashMap::new(),
        engine.text_shaper.as_ref(),
        engine.hyphenator,
        &engine.punctuation_atom_builder,
        &engine.punctuation_spacing_compressor,
    );
    assert!(!prep.clusters.is_empty());
}

#[test]
fn adjacent_inline_object_boundaries_merging_and_conflicts() {
    let engine = ExplainableStubParagraphLayoutEngine::default();
    for uniform1 in [true, false] {
        for uniform2 in [true, false] {
            for prevents1 in [true, false] {
                for prevents2 in [true, false] {
                    let object1 = InlineObjectSpan::with_trailing_boundary(
                        text_range(1, 2),
                        20.0,
                        12.0,
                        4.0,
                        InlineObjectBoundaryAdjustment::builder()
                            .participates_in_uniform_stretch(uniform1)
                            .preferred_stretch(InlineObjectPreferredStretch::new(
                                InlineObjectPreferredStretchKind::PunctuationTrailing,
                                10.0,
                                15.0,
                            ))
                            .shrink_capacity(2.0)
                            .line_end_discardable_advance(1.0)
                            .prevents_line_break(prevents1)
                            .build(),
                    );
                    let object2 = InlineObjectSpan::with_leading_boundary(
                        text_range(2, 3),
                        20.0,
                        12.0,
                        4.0,
                        InlineObjectBoundaryAdjustment::builder()
                            .participates_in_uniform_stretch(uniform2)
                            .preferred_stretch(InlineObjectPreferredStretch::new(
                                InlineObjectPreferredStretchKind::PunctuationTrailing,
                                10.0,
                                20.0,
                            ))
                            .prevents_line_break(prevents2)
                            .build(),
                    );
                    let input = LayoutInput::builder(
                        TiqianTextContent::new(Text::from("一二三四")),
                        LayoutConstraints::with_defaults(300.0),
                    )
                    .inline_objects(vec![object1, object2])
                    .build();
                    let annotation = prepare_width_independent_annotation(
                        &input,
                        &HashMap::new(),
                        engine.clreq_profile_resolver.as_ref(),
                        engine.font_role_classifier.as_ref(),
                        engine.fallback_resolver.as_ref(),
                        engine.font_metrics_resolver.as_ref(),
                        &engine.quote_pair_analyzer,
                        engine.text_shaper.as_ref(),
                        engine.hyphenator,
                    );
                    let prep = build_paragraph_layout_prep(
                        &input,
                        &annotation,
                        &HashMap::new(),
                        engine.text_shaper.as_ref(),
                        engine.hyphenator,
                        &engine.punctuation_atom_builder,
                        &engine.punctuation_spacing_compressor,
                    );
                    assert!(!prep.clusters.is_empty());
                }
            }
        }
    }

    let object1 = InlineObjectSpan::with_trailing_boundary(
        text_range(1, 2),
        20.0,
        12.0,
        4.0,
        InlineObjectBoundaryAdjustment::builder()
            .preferred_stretch(InlineObjectPreferredStretch::new(
                InlineObjectPreferredStretchKind::PunctuationTrailing,
                10.0,
                15.0,
            ))
            .build(),
    );
    let object2 = InlineObjectSpan::with_leading_boundary(
        text_range(2, 3),
        20.0,
        12.0,
        4.0,
        InlineObjectBoundaryAdjustment::builder()
            .preferred_stretch(InlineObjectPreferredStretch::new(
                InlineObjectPreferredStretchKind::Relation,
                10.0,
                20.0,
            ))
            .build(),
    );
    let input = LayoutInput::builder(
        TiqianTextContent::new(Text::from("一二三四")),
        LayoutConstraints::with_defaults(300.0),
    )
    .inline_objects(vec![object1, object2])
    .build();
    let annotation = prepare_width_independent_annotation(
        &input,
        &HashMap::new(),
        engine.clreq_profile_resolver.as_ref(),
        engine.font_role_classifier.as_ref(),
        engine.fallback_resolver.as_ref(),
        engine.font_metrics_resolver.as_ref(),
        &engine.quote_pair_analyzer,
        engine.text_shaper.as_ref(),
        engine.hyphenator,
    );
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        build_paragraph_layout_prep(
            &input,
            &annotation,
            &HashMap::new(),
            engine.text_shaper.as_ref(),
            engine.hyphenator,
            &engine.punctuation_atom_builder,
            &engine.punctuation_spacing_compressor,
        )
    }));
    let message = match result {
        Ok(_) => panic!("expected conflicting inline-object stretch classes"),
        Err(payload) => payload.downcast::<String>().unwrap(),
    };
    assert!(message.contains("Conflicting inline-object stretch classes"));
}

#[test]
fn centered_punct_before_attached_reference_keeps_leading_glue_only() {
    struct NarrowInkShaper;

    impl TextShaper for NarrowInkShaper {
        fn shape(&self, input: &ShapingInput) -> ShapingResult {
            let result = tiqian::shaping::text_shaper::ExplainableStubTextShaper.shape(input);
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
                                        .bounds(Some(tiqian::core::geometry::Rect {
                                            left: 4.0,
                                            top: 2.0,
                                            right: 12.0,
                                            bottom: 10.0,
                                        }))
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

    let text = "正文：“内容·[1]，后文";
    let byte_start = text.find("[1]").unwrap();
    let attach_start = text[..byte_start].chars().count() as i32;
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.text_shaper = Box::new(NarrowInkShaper);
    let result = engine.layout(
        LayoutInput::builder(
            TiqianTextContent::builder(Text::from(text))
                .spans(vec![TextSpan {
                    range: text_range(attach_start, attach_start + 3),
                    style: TextStyle::builder()
                        .inline_attachment(InlineAttachment::Previous)
                        .build(),
                }])
                .build(),
            LayoutConstraints::with_defaults(320.0),
        )
        .paragraph_style(
            ParagraphStyle::builder()
                .first_line_indent(Some(Ic::ZERO))
                .build(),
        )
        .build(),
    );
    let decision = result
        .debug
        .spacing_decisions
        .iter()
        .find(|decision| {
            decision
                .reason
                .starts_with("AttachedInlineVirtualPunctuationBoundary")
        })
        .unwrap();
    assert_eq!(
        "AttachedInlineVirtualPunctuationBoundary:adjacent-punctuation",
        decision.reason
    );
    assert_eq!('·', decision.left_char);
    assert_eq!('，', decision.right_char);
    assert!(decision.natural_inner_glue > 0.0);
    assert!(decision.reduction > 0.0);
    assert_eq!(
        scalar_offset(text[..text.find('·').unwrap()].chars().count() as i32),
        decision.reduction_target_range.start()
    );
}

#[test]
fn prepare_width_independent_annotation_branches() {
    let engine = ExplainableStubParagraphLayoutEngine::default();
    let input = LayoutInput::builder(
        TiqianTextContent::builder(Text::from("测试文本【中文】与English，以及注音与行内框。"))
            .spans(vec![
                TextSpan {
                    range: text_range(0, 0),
                    style: TextStyle::builder().font_size(10.0).build(),
                },
                TextSpan {
                    range: text_range(0, 1),
                    style: TextStyle::builder().font_size(18.0).font_weight(500).build(),
                },
                TextSpan {
                    range: text_range(1, 4),
                    style: TextStyle::builder().font_size(18.0).font_weight(500).build(),
                },
                TextSpan {
                    range: text_range(4, 8),
                    style: TextStyle::builder().font_size(14.0).font_weight(300).build(),
                },
            ])
            .line_break_spans(vec![LineBreakSpan {
                range: text_range(8, 15),
                policy: LineBreakPolicy::ProgressiveTechnical,
            }])
            .source_boundaries(HashSet::from([scalar_offset(1), scalar_offset(2), scalar_offset(3), scalar_offset(4), scalar_offset(6)]))
            .build(),
        LayoutConstraints::with_defaults(300.0),
    )
    .text_style(
        TextStyle::builder()
            .font_size(16.0)
            .locale("zh-CN".to_owned())
            .font_weight(400)
            .build(),
    )
    .decorations(vec![
        DecorationSpan {
            range: text_range(0, 4),
            kind: DecorationKind::Emphasis,
        },
        DecorationSpan {
            range: text_range(4, 8),
            kind: DecorationKind::ProperNoun,
        },
    ])
    .ruby_spans(vec![
        RubySpan::builder(text_range(0, 2), Text::from("cèshì"))
            .locale(Some("zh-Latn".to_owned()))
            .build(),
        RubySpan::new(text_range(2, 4), Text::new()),
        RubySpan::with_kind(text_range(0, 1), Text::from("˙ㄅ"), RubyKind::Bopomofo),
        RubySpan::with_kind(text_range(0, 1), Text::from("ㄆ"), RubyKind::Bopomofo),
        RubySpan::with_kind(text_range(99, 100), Text::from("invalid"), RubyKind::Bopomofo),
    ])
    .inline_boxes(vec![
        InlineBoxSpan::with_edges(text_range(15, 17), 4.0, 0.0),
        InlineBoxSpan::with_edges(text_range(17, 19), 0.0, 4.0),
        InlineBoxSpan::with_all(text_range(19, 21), 0.0, 0.0, InlineBoxOuterSpacing::Narrow),
        InlineBoxSpan::with_all(text_range(21, 23), 0.0, 0.0, InlineBoxOuterSpacing::Source),
    ])
    .inline_objects(vec![InlineObjectSpan::with_fixed_boundaries(
        text_range(23, 24),
        20.0,
        12.0,
        4.0,
    )])
    .build();
    let annotation = prepare_width_independent_annotation(
        &input,
        &HashMap::new(),
        engine.clreq_profile_resolver.as_ref(),
        engine.font_role_classifier.as_ref(),
        engine.fallback_resolver.as_ref(),
        engine.font_metrics_resolver.as_ref(),
        &engine.quote_pair_analyzer,
        engine.text_shaper.as_ref(),
        engine.hyphenator,
    );
    assert_eq!(18.0, (annotation.font_size_at)(scalar_offset(0)));
    assert_eq!(14.0, (annotation.font_size_at)(scalar_offset(5)));
    assert_eq!(16.0, (annotation.font_size_at)(scalar_offset(24)));
    assert_eq!(800, (annotation.bopomofo_font_weight_at)(scalar_offset(0)));
    assert_eq!(600, (annotation.bopomofo_font_weight_at)(scalar_offset(5)));
    assert_eq!(700, (annotation.bopomofo_font_weight_at)(scalar_offset(24)));
    assert_eq!(18.0, (annotation.style_at)(scalar_offset(0)).font_size);
    assert_eq!(18.0, (annotation.style_at)(scalar_offset(3)).font_size);
    assert_eq!(14.0, (annotation.style_at)(scalar_offset(4)).font_size);
    assert_eq!(14.0, (annotation.style_at)(scalar_offset(7)).font_size);
    assert_eq!(16.0, (annotation.style_at)(scalar_offset(8)).font_size);
    assert_eq!(16.0, (annotation.style_at)(scalar_offset(25)).font_size);

    let prep = build_paragraph_layout_prep(
        &input,
        &annotation,
        &HashMap::new(),
        engine.text_shaper.as_ref(),
        engine.hyphenator,
        &engine.punctuation_atom_builder,
        &engine.punctuation_spacing_compressor,
    );
    assert!(!prep.ruby_and_bopomofo_spread.is_empty());
}

#[test]
fn shrink_opportunities_cover_all_punctuation_classes_and_spaces() {
    struct TaiwanProfileResolver;

    impl ClreqProfileResolver for TaiwanProfileResolver {
        fn resolve(&self, _: &tiqian::core::text_model::LayoutProfileId) -> ClreqProfile {
            ClreqProfile::taiwan_horizontal()
        }
    }

    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.clreq_profile_resolver = Box::new(TaiwanProfileResolver);
    let text = Text::from("「引用」·中点‧间隔•中点，逗号。句号！问号？．点号、顿号以及 English words 间距");
    let spans: Vec<TextSpan> = text.scalar_indices()
        .map(|(offset, _)| TextSpan {
            range: tiqian::core::geometry::TextRange::new(offset, offset + 1),
            style: TextStyle::builder().font_size(16.0).build(),
        })
        .collect();
    for allow_inline_stop in [true, false] {
        for allow_sino_western in [true, false] {
            let input = LayoutInput::builder(
                TiqianTextContent::builder(text.clone())
                    .spans(spans.clone())
                    .source_boundaries(text.scalar_indices().map(|(offset, _)| offset).collect())
                    .build(),
                LayoutConstraints::with_defaults(300.0),
            )
            .inline_objects(vec![InlineObjectSpan::with_trailing_boundary(
                text_range(0, 1),
                20.0,
                12.0,
                4.0,
                InlineObjectBoundaryAdjustment::builder()
                    .shrink_capacity(5.0)
                    .build(),
            )])
            .build();
            let mut annotation = prepare_width_independent_annotation(
                &input,
                &HashMap::new(),
                engine.clreq_profile_resolver.as_ref(),
                engine.font_role_classifier.as_ref(),
                engine.fallback_resolver.as_ref(),
                engine.font_metrics_resolver.as_ref(),
                &engine.quote_pair_analyzer,
                engine.text_shaper.as_ref(),
                engine.hyphenator,
            );
            annotation.clreq_profile.adjustment.allow_inline_stop_compression = allow_inline_stop;
            annotation.clreq_profile.adjustment.allow_sino_western_gap_adjustment =
                allow_sino_western;
            let prep = build_paragraph_layout_prep(
                &input,
                &annotation,
                &HashMap::new(),
                engine.text_shaper.as_ref(),
                engine.hyphenator,
                &engine.punctuation_atom_builder,
                &engine.punctuation_spacing_compressor,
            );
            assert!(!prep.shrink_opportunities.is_empty());
        }
    }
}

#[test]
fn style_at_and_emphasis_italic_at_and_dynamic_shaping_branches() {
    let engine = ExplainableStubParagraphLayoutEngine::default();
    let input = LayoutInput::builder(
        TiqianTextContent::builder(Text::from("English 中文 混排 Latin 测试 样式"))
            .spans(vec![TextSpan {
                range: text_range(8, 10),
                style: TextStyle::builder().font_size(24.0).build(),
            }])
            .line_break_spans(vec![LineBreakSpan {
                range: text_range(0, 7),
                policy: LineBreakPolicy::ProgressiveTechnical,
            }])
            .build(),
        LayoutConstraints::with_defaults(50.0),
    )
    .decorations(vec![
        DecorationSpan {
            range: text_range(0, 7),
            kind: DecorationKind::Emphasis,
        },
        DecorationSpan {
            range: text_range(11, 13),
            kind: DecorationKind::ProperNoun,
        },
    ])
    .build();
    let mut annotation = prepare_width_independent_annotation(
        &input,
        &HashMap::new(),
        engine.clreq_profile_resolver.as_ref(),
        engine.font_role_classifier.as_ref(),
        engine.fallback_resolver.as_ref(),
        engine.font_metrics_resolver.as_ref(),
        &engine.quote_pair_analyzer,
        engine.text_shaper.as_ref(),
        engine.hyphenator,
    );
    assert_eq!(24.0, (annotation.font_size_at)(scalar_offset(8)));
    assert_eq!(24.0, (annotation.font_size_at)(scalar_offset(9)));
    for offset in [0, 7, 10, 20, 100] {
        assert_eq!(input.text_style.font_size, (annotation.font_size_at)(scalar_offset(offset)));
    }
    let rejected = HashMap::from([(
        text_range(0, 7),
        HashSet::from([ProgressiveBreakTier::Structural]),
    )]);
    let prep = build_paragraph_layout_prep(
        &input,
        &annotation,
        &rejected,
        engine.text_shaper.as_ref(),
        engine.hyphenator,
        &engine.punctuation_atom_builder,
        &engine.punctuation_spacing_compressor,
    );
    assert!(!prep.clusters.is_empty());

    for width in [500.0, 1.0] {
        let no_break_input = LayoutInput::builder(
            TiqianTextContent::new(Text::from("English")),
            LayoutConstraints::with_defaults(width),
        )
        .build();
        let no_break_annotation = prepare_width_independent_annotation(
            &no_break_input,
            &HashMap::new(),
            engine.clreq_profile_resolver.as_ref(),
            engine.font_role_classifier.as_ref(),
            engine.fallback_resolver.as_ref(),
            engine.font_metrics_resolver.as_ref(),
            &engine.quote_pair_analyzer,
            engine.text_shaper.as_ref(),
            engine.hyphenator,
        );
        let prep = build_paragraph_layout_prep(
            &no_break_input,
            &no_break_annotation,
            &HashMap::new(),
            engine.text_shaper.as_ref(),
            engine.hyphenator,
            &engine.punctuation_atom_builder,
            &engine.punctuation_spacing_compressor,
        );
        assert!(!prep.clusters.is_empty());
    }

    annotation.font_decisions.truncate(1);
    let prep_unknown_roles = build_paragraph_layout_prep(
        &input,
        &annotation,
        &HashMap::new(),
        engine.text_shaper.as_ref(),
        engine.hyphenator,
        &engine.punctuation_atom_builder,
        &engine.punctuation_spacing_compressor,
    );
    assert!(!prep_unknown_roles.clusters.is_empty());
}

#[test]
fn dynamic_shaping_emphasis_italic_at_and_zero_paired_capacity_branches() {
    let engine = ExplainableStubParagraphLayoutEngine::default();
    let input = LayoutInput::builder(
        TiqianTextContent::builder(Text::from("Hello World Latin"))
            .line_break_spans(vec![LineBreakSpan {
                range: text_range(0, 17),
                policy: LineBreakPolicy::ProgressiveTechnical,
            }])
            .build(),
        LayoutConstraints::with_defaults(100.0),
    )
    .decorations(vec![
        DecorationSpan {
            range: text_range(0, 5),
            kind: DecorationKind::ProperNoun,
        },
        DecorationSpan {
            range: text_range(6, 11),
            kind: DecorationKind::Emphasis,
        },
    ])
    .build();
    let mut annotation = prepare_width_independent_annotation(
        &input,
        &HashMap::new(),
        engine.clreq_profile_resolver.as_ref(),
        engine.font_role_classifier.as_ref(),
        engine.fallback_resolver.as_ref(),
        engine.font_metrics_resolver.as_ref(),
        &engine.quote_pair_analyzer,
        engine.text_shaper.as_ref(),
        engine.hyphenator,
    );
    annotation.segment_shaping_cache = HashMap::new();
    let rejected = HashMap::from([(
        text_range(0, 17),
        HashSet::from([ProgressiveBreakTier::Structural]),
    )]);
    let prep = build_paragraph_layout_prep(
        &input,
        &annotation,
        &rejected,
        engine.text_shaper.as_ref(),
        engine.hyphenator,
        &engine.punctuation_atom_builder,
        &engine.punctuation_spacing_compressor,
    );
    assert!(!prep.clusters.is_empty());
}