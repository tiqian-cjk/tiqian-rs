use tiqian::core::geometry::{LayoutConstraints, Size, TextRange};
use tiqian::core::int_range::IntRange;
use tiqian::core::layout_model::{
    BopomofoDecisionInfo, BopomofoGlyphPlacement, BopomofoGlyphRole, Cluster,
    DecorationDecisionInfo, DecorationSegmentInfo, FontDecisionInfo, Glyph,
    GlyphRun, LayoutDebugInfo, LayoutResult, LineBox, PunctuationDecisionInfo, RubyDecisionInfo,
    ShapingDecisionInfo, ZeroWidthBreakDecisionInfo,
};
use tiqian::core::text::Text;
use tiqian::core::text_model::{
    DecorationKind, DecorationSpan, InlineBoxSpan, InlineObjectSpan, LayoutInput, TextSpan, TextStyle,
    TiqianTextContent,
};
use tiqian::layout::prepared_paragraph::{
    to_plan_with_diagnostics_json, to_prepared_paragraph_json,
};

fn line(range: TextRange, clusters: IntRange, width: f32) -> LineBox {
    LineBox::builder(range, clusters, 20.0, 0.0, 24.0, width, width, width).build()
}

fn result(
    input: LayoutInput,
    clusters: Vec<Cluster>,
    glyph_runs: Vec<GlyphRun>,
    debug: LayoutDebugInfo,
    width: f32,
) -> LayoutResult {
    let range_end = input.content.text.utf16_len();
    let last_cluster = clusters.len() as i32 - 1;
    LayoutResult::with_debug(
        input,
        Size { width, height: 24.0 },
        clusters,
        glyph_runs,
        vec![line(TextRange::new(0, range_end), IntRange::new(0, last_cluster), width)],
        debug,
    )
}

#[test]
fn open_type_features_and_render_font_family_attach_per_cluster() {
    let input = LayoutInput::builder(TiqianTextContent::new(Text::from("汉")), LayoutConstraints::with_defaults(480.0)).build();
    let cluster = Cluster::new(TextRange::new(0, 1), Text::from("汉"), "cjk".into(), 16.0);
    let glyph = Glyph::builder(7, TextRange::new(0, 1), 16.0).render_font_key(Some("Noto Serif CJK".into())).build();
    let json = to_prepared_paragraph_json(&result(input, vec![cluster], vec![GlyphRun::with_open_type_features(TextRange::new(0, 1), "cjk".into(), vec![glyph], 16.0, vec!["kern".into(), "liga".into()])], LayoutDebugInfo::default(), 480.0), true);
    assert!(json.contains("\"openTypeFeatures\":[\"kern\",\"liga\"]"), "{json}");
    assert!(json.contains("\"renderFontFamily\":\"Noto Serif CJK\""), "{json}");
    assert!(!json.contains("shapingBoundary"), "{json}");
}

#[test]
fn multi_unit_cluster_marks_shaping_boundary() {
    let input = LayoutInput::builder(TiqianTextContent::new(Text::from("AB")), LayoutConstraints::with_defaults(480.0)).build();
    let cluster = Cluster::new(TextRange::new(0, 2), Text::from("AB"), "latin".into(), 18.0);
    let glyph = Glyph::builder(1, TextRange::new(0, 2), 18.0).build();
    let json = to_prepared_paragraph_json(&result(input, vec![cluster], vec![GlyphRun::new(TextRange::new(0, 2), "latin".into(), vec![glyph], 18.0)], LayoutDebugInfo::default(), 480.0), false);
    assert!(json.contains("\"shapingBoundary\":true"), "{json}");
}

#[test]
fn inline_object_cell_emits_advance_override() {
    let input = LayoutInput::builder(TiqianTextContent::new(Text::from("汉图")), LayoutConstraints::with_defaults(480.0))
        .inline_objects(vec![InlineObjectSpan::with_fixed_boundaries(TextRange::new(1, 2), 24.0, 12.0, 4.0)])
        .build();
    let clusters = vec![
        Cluster::new(TextRange::new(0, 1), Text::from("汉"), "cjk".into(), 16.0),
        Cluster::new(TextRange::new(1, 2), Text::from("图"), "inline".into(), 10.0),
    ];
    let runs = vec![
        GlyphRun::new(TextRange::new(0, 1), "cjk".into(), vec![Glyph::builder(1, TextRange::new(0, 1), 16.0).build()], 16.0),
        GlyphRun::new(TextRange::new(1, 2), "inline".into(), vec![Glyph::builder(2, TextRange::new(1, 2), 24.0).build()], 24.0),
    ];
    let built = result(input, clusters, runs, LayoutDebugInfo::default(), 40.0);
    let evidence = to_prepared_paragraph_json(&built, true);
    assert!(evidence.contains("\"inlineObject\":24"), "{evidence}");
    assert!(evidence.contains("\"advance\":10"), "{evidence}");
    let empty_display = LayoutResult { clusters: built.clusters.iter().map(|cluster| if cluster.range.start() == 1 { Cluster::with_display_text(cluster.range, cluster.text.clone(), Text::new(), cluster.font_key.clone(), cluster.advance) } else { cluster.clone() }).collect(), ..built };
    let plain = to_prepared_paragraph_json(&empty_display, false);
    assert!(!plain.contains("\"inlineObject\""), "{plain}");
    assert!(!plain.contains("\"rangeStart\":1"), "{plain}");
    assert!(to_prepared_paragraph_json(&empty_display, true).contains("\"inlineObject\":24"));
}

#[test]
fn style_delta_lists_only_paint_fields() {
    let input = LayoutInput::builder(
        TiqianTextContent::builder(Text::from("汉A字"))
            .spans(vec![
                TextSpan { range: TextRange::new(0, 1), style: TextStyle::builder().font_size(20.0).font_weight(700).italic(true).build() },
                TextSpan { range: TextRange::new(1, 2), style: TextStyle::builder().font_families(vec!["Kai".into()]).build() },
            ])
            .build(),
        LayoutConstraints::with_defaults(480.0),
    ).build();
    let clusters = vec![Cluster::new(TextRange::new(0, 1), Text::from("汉"), "cjk".into(), 16.0), Cluster::new(TextRange::new(1, 2), Text::from("A"), "latin".into(), 10.0), Cluster::new(TextRange::new(2, 3), Text::from("字"), "cjk".into(), 16.0)];
    let glyphs = clusters.iter().enumerate().map(|(index, cluster)| Glyph::builder(index as u32 + 1, cluster.range, cluster.advance).build()).collect();
    let json = to_prepared_paragraph_json(&result(input, clusters, vec![GlyphRun::new(TextRange::new(0, 3), "k".into(), glyphs, 42.0)], LayoutDebugInfo::default(), 42.0), true);
    assert!(json.contains("\"style\":{\"fontSize\":20,\"fontWeight\":700,\"italic\":true}"), "{json}");
    assert!(json.contains("\"style\":{}"), "{json}");
    assert_eq!(2, json.matches("\"style\":").count(), "{json}");
}

#[test]
fn dash_cluster_emits_shaping_evidence_block() {
    let input = LayoutInput::builder(TiqianTextContent::new(Text::from("汉——")), LayoutConstraints::with_defaults(480.0)).build();
    let clusters = vec![Cluster::new(TextRange::new(0, 1), Text::from("汉"), "cjk".into(), 16.0), Cluster::new(TextRange::new(1, 3), Text::from("——"), "cjk".into(), 32.0)];
    let runs = vec![GlyphRun::new(TextRange::new(0, 1), "cjk".into(), vec![Glyph::builder(1, TextRange::new(0, 1), 16.0).build()], 16.0), GlyphRun::new(TextRange::new(1, 3), "cjk".into(), vec![Glyph::builder(9, TextRange::new(1, 3), 32.0).render_font_key(Some("Noto Sans CJK".into())).build(), Glyph::builder(10, TextRange::new(1, 3), 0.0).build()], 32.0)];
    let decision = ShapingDecisionInfo::builder(TextRange::new(1, 3), Text::from("——"), Text::from("——"), "cjk".into(), 2, 32.0, "ShapingStage".into(), "dash-reason".into()).strategy(Some("PairedEmDash".into())).language(Some("zh-Hans".into())).resolved_face(Some("NotoSansCJK".into())).build();
    let json = to_prepared_paragraph_json(&result(input, clusters, runs, LayoutDebugInfo::builder().shaping_decisions(vec![decision]).build(), 48.0), true);
    for expected in ["\"dashStrategy\":\"PairedEmDash\"", "\"shapingLanguage\":\"zh-Hans\"", "\"resolvedFace\":\"NotoSansCJK\"", "\"glyphIds\":\"9,10\"", "\"shapingEvidence\":\"dash-reason\"", "\"naturalWidth\":32"] { assert!(json.contains(expected), "missing {expected}: {json}"); }
}

#[test]
fn shaping_decision_preserves_recorded_script_metadata() {
    let decision = ShapingDecisionInfo::builder(
        TextRange::new(0, 1),
        Text::from("A"),
        Text::from("A"),
        "latin".into(),
        1,
        8.0,
        "RecordedShapingEvidence".into(),
        "recorded".into(),
    )
    .script(Some("Latn".into()))
    .build();

    assert_eq!(Some("Latn".into()), decision.script);
}

#[test]
fn punctuation_ink_floor_and_latin_role_mark_cells() {
    let input = LayoutInput::builder(TiqianTextContent::new(Text::from("。A中")), LayoutConstraints::with_defaults(480.0)).build();
    let clusters = vec![Cluster::new(TextRange::new(0, 1), Text::from("。"), "cjk".into(), 16.0), Cluster::new(TextRange::new(1, 2), Text::from("A"), "latin".into(), 10.0), Cluster::new(TextRange::new(2, 3), Text::from("中"), "cjk".into(), 16.0)];
    let glyphs = clusters.iter().enumerate().map(|(index, cluster)| Glyph::builder(index as u32 + 1, cluster.range, cluster.advance).build()).collect();
    let punctuation = vec![
        PunctuationDecisionInfo::builder(TextRange::new(0, 1), '。', "PauseOrStop".into(), 16.0, 16.0, 0.0, 0.0, "centre".into()).ink_containment_body_floor(Some(6.0)).ink_containment_applied(true).build(),
        PunctuationDecisionInfo::builder(TextRange::new(1, 2), 'A', "Other".into(), 10.0, 10.0, 0.0, 0.0, "centre".into()).ink_containment_applied(true).build(),
        PunctuationDecisionInfo::builder(TextRange::new(2, 3), '中', "Other".into(), 16.0, 16.0, 0.0, 0.0, "centre".into()).build(),
    ];
    let font = FontDecisionInfo { range: TextRange::new(1, 2), source_text: Text::from("A"), display_text: Text::from("A"), role: "LatinText".into(), font_key: "latin".into(), reason: "latin-run".into(), substitution_reason: "none".into() };
    let json = to_prepared_paragraph_json(&result(input, clusters, vec![GlyphRun::new(TextRange::new(0, 3), "k".into(), glyphs, 42.0)], LayoutDebugInfo::builder().punctuation_decisions(punctuation).font_decisions(vec![font]).build(), 42.0), true);
    assert!(json.contains("\"punctuationInkFloor\":6"), "{json}");
    assert!(json.contains("\"punctuationBodyWidth\":16"), "{json}");
    assert_eq!(1, json.matches("\"punctuationInkFloor\":").count(), "{json}");
    assert!(json.contains("\"latin\":true"), "{json}");
}

#[test]
fn zero_width_break_cluster_survives_empty_display_text() {
    let input = LayoutInput::builder(TiqianTextContent::new(Text::from("汉字")), LayoutConstraints::with_defaults(480.0)).build();
    let clusters = vec![Cluster::new(TextRange::new(0, 1), Text::from("汉"), "cjk".into(), 16.0), Cluster::with_display_text(TextRange::new(1, 2), Text::from(""), Text::new(), "cjk".into(), 0.0), Cluster::new(TextRange::new(2, 3), Text::from("字"), "cjk".into(), 16.0)];
    let runs = vec![GlyphRun::new(TextRange::new(0, 1), "cjk".into(), vec![Glyph::builder(1, TextRange::new(0, 1), 16.0).build()], 16.0), GlyphRun::new(TextRange::new(2, 3), "cjk".into(), vec![Glyph::builder(3, TextRange::new(2, 3), 16.0).build()], 16.0)];
    let decision = ShapingDecisionInfo::builder(TextRange::new(1, 2), Text::from(""), Text::new(), "cjk".into(), 0, 0.0, "ShapingStage".into(), "no-shape".into()).strategy(Some("ZeroWidthNoShape".into())).build();
    let debug = LayoutDebugInfo::builder().zero_width_break_decisions(vec![ZeroWidthBreakDecisionInfo::new(TextRange::new(1, 2), Text::from(""), 1)]).shaping_decisions(vec![decision]).build();
    let built = result(input, clusters, runs, debug, 32.0);
    let plain = to_prepared_paragraph_json(&built, false);
    assert!(plain.contains("\"display\":\"\",\"drawX\":16"), "{plain}");
    assert_eq!(3, plain.matches("\"source\":").count(), "{plain}");
    let evidence = to_prepared_paragraph_json(&built, true);
    assert!(evidence.contains("\"dashStrategy\":\"ZeroWidthNoShape\""), "{evidence}");
    assert!(evidence.contains("\"shapingEvidence\":\"no-shape\""), "{evidence}");
    assert!(!evidence.contains("shapingLanguage"), "{evidence}");
    assert!(!evidence.contains("resolvedFace"), "{evidence}");
    assert!(!evidence.contains("glyphIds"), "{evidence}");
}

#[test]
fn paragraph_evidence_emits_every_section() {
    let input = LayoutInput::builder(TiqianTextContent::new(Text::from("汉注")), LayoutConstraints::with_defaults(480.0))
        .decorations(vec![DecorationSpan { range: TextRange::new(0, 1), kind: DecorationKind::Emphasis }, DecorationSpan { range: TextRange::new(1, 2), kind: DecorationKind::Emphasis }])
        .inline_boxes(vec![InlineBoxSpan::with_edges(TextRange::new(0, 1), 2.0, 0.0), InlineBoxSpan::with_edges(TextRange::new(0, 1), 0.5, 0.0), InlineBoxSpan::with_edges(TextRange::new(1, 2), 0.0, 3.0), InlineBoxSpan::with_edges(TextRange::new(0, 2), 0.0, 1.5)])
        .build();
    let clusters = vec![Cluster::new(TextRange::new(0, 1), Text::from("汉"), "cjk".into(), 16.0), Cluster::new(TextRange::new(1, 2), Text::from("注"), "cjk".into(), 16.0)];
    let glyphs = vec![Glyph::builder(1, TextRange::new(0, 1), 16.0).build(), Glyph::builder(2, TextRange::new(1, 2), 16.0).build()];
    let ruby = RubyDecisionInfo::builder(TextRange::new(0, 1), Text::from("hàn"), 0, 8.0, 2.0, 8.0, 0.5).ascent(6.0).font_families(vec!["RubyKai".into(), "RubyLatin".into()]).build();
    let bopomofo = BopomofoDecisionInfo::builder(TextRange::new(1, 2), Text::from("ㄓㄨˋ"), 0, vec![BopomofoGlyphPlacement::new(Text::from("ㄓ"), 1.0, 2.0, 4.0, 4.0, BopomofoGlyphRole::Symbol), BopomofoGlyphPlacement::new(Text::from("ˋ"), 2.0, 0.0, 2.0, 2.0, BopomofoGlyphRole::Tone)]).font_families(vec!["BopomofoKai".into(), "BopomofoLatin".into()]).build();
    let debug = LayoutDebugInfo::builder().ruby_decisions(vec![ruby]).bopomofo_decisions(vec![bopomofo]).decoration_segments(vec![DecorationSegmentInfo { source_range: TextRange::new(0, 1), kind: "ProperNoun".into(), line_index: 0, left: 0.0, top: 20.0, right: 16.0, bottom: 22.0, open_start: false, open_end: false, reason: "proper-noun".into() }, DecorationSegmentInfo { source_range: TextRange::new(1, 2), kind: "BookTitle".into(), line_index: 0, left: 16.0, top: 20.0, right: 32.0, bottom: 22.0, open_start: false, open_end: false, reason: "book-title".into() }, DecorationSegmentInfo { source_range: TextRange::new(0, 2), kind: "Emphasis".into(), line_index: 0, left: 0.0, top: 0.0, right: 32.0, bottom: 4.0, open_start: false, open_end: false, reason: "filtered".into() }]).decoration_decisions(vec![DecorationDecisionInfo::builder(TextRange::new(0, 1), Text::from("汉"), "Emphasis".into(), true, "dot".into()).anchor_x(8.0).anchor_y(22.0).dot_diameter(2.0).build(), DecorationDecisionInfo::builder(TextRange::new(1, 2), Text::from("注"), "Emphasis".into(), false, "skip".into()).anchor_x(24.0).anchor_y(22.0).dot_diameter(2.0).build()]).build();
    let json = to_prepared_paragraph_json(&result(input, clusters, vec![GlyphRun::new(TextRange::new(0, 2), "cjk".into(), glyphs, 32.0)], debug, 480.0), true);
    for expected in ["\"emphasisRanges\":[[0,1],[1,2]]", "\"inlineEdges\":[{\"offset\":0,\"inlineStart\":2.5},{\"offset\":2,\"inlineEnd\":4.5}]", "\"rubyDecisions\":[{\"baseRangeStart\":0", "\"ascent\":6", "\"fontFamilies\":[\"RubyKai\",\"RubyLatin\"]", "\"bopomofoDecisions\":[{\"baseRangeStart\":1", "\"role\":\"Symbol\"", "\"role\":\"Tone\"", "\"decorationSegments\":[{\"kind\":\"ProperNoun\"", "\"kind\":\"BookTitle\"", "\"emphasisDots\":[{\"clusterRangeStart\":0,\"anchorX\":8,\"anchorY\":22,\"dotDiameter\":2}]", "\"fontSize\":16", "\"overlayWidth\":480"] { assert!(json.contains(expected), "missing {expected}: {json}"); }
    assert!(!json.contains("\"kind\":\"Emphasis\""), "{json}");
}

#[test]
fn negative_zero_and_exponent_widths_normalize() {
    let input = LayoutInput::builder(TiqianTextContent::new(Text::from("汉")), LayoutConstraints::with_defaults(1.0e21)).build();
    let cluster = Cluster::new(TextRange::new(0, 1), Text::from("汉"), "cjk".into(), 16.0);
    let glyph = Glyph::builder(1, TextRange::new(0, 1), 16.0).build();
    let mut line = line(TextRange::new(0, 1), IntRange::new(0, 0), 16.0);
    line.indent = -0.0;
    line.hyphen_advance = -0.0;
    let built = LayoutResult::new(input, Size { width: 1.0e21, height: -0.0 }, vec![cluster], vec![GlyphRun::new(TextRange::new(0, 1), "cjk".into(), vec![glyph], 16.0)], vec![line]);
    let json = to_prepared_paragraph_json(&built, false);
    assert!(json.contains("\"width\":1.0000000200408773e+21"), "{json}");
    assert!(json.contains("\"height\":0"), "{json}");
    assert!(json.contains("\"indent\":0"), "{json}");
    assert!(json.contains("\"hyphenAdvance\":0"), "{json}");
}

#[test]
fn json_string_escapes_quotes_backslashes_and_control_characters() {
    let tricky = "\"\\\u{08}\u{0c}\n\r\t\u{0001}";
    let input = LayoutInput::builder(TiqianTextContent::new(Text::from(tricky)), LayoutConstraints::with_defaults(480.0)).build();
    let cluster = Cluster::new(TextRange::new(0, 8), Text::from(tricky), "cjk".into(), 8.0);
    let glyph = Glyph::builder(1, TextRange::new(0, 8), 8.0).build();
    let json = to_prepared_paragraph_json(&result(input, vec![cluster], vec![GlyphRun::new(TextRange::new(0, 8), "cjk".into(), vec![glyph], 8.0)], LayoutDebugInfo::default(), 8.0), false);
    for escaped in ["\\\"", "\\\\", "\\b", "\\f", "\\n", "\\r", "\\t", "\\u0001"] { assert!(json.contains(escaped), "missing {escaped}: {json}"); }
}

#[test]
fn plan_with_diagnostics_lists_capability_issues_and_advance_suspects() {
    let input = LayoutInput::builder(TiqianTextContent::new(Text::from("汉零臣")), LayoutConstraints::with_defaults(480.0)).build();
    let clusters = vec![Cluster::new(TextRange::new(0, 1), Text::from("汉"), "cjk".into(), 32.0), Cluster::new(TextRange::new(1, 2), Text::from("零"), "cjk".into(), 0.0), Cluster::new(TextRange::new(2, 3), Text::from("臣"), "cjk".into(), 16.0)];
    let glyphs = vec![Glyph::builder(1, TextRange::new(0, 1), 32.0).build(), Glyph::builder(2, TextRange::new(1, 2), 0.0).build(), Glyph::builder(3, TextRange::new(2, 3), 16.0).build()];
    let decisions = vec![
        ShapingDecisionInfo::builder(TextRange::new(0, 1), Text::from("汉"), Text::from("汉"), "cjk".into(), 1, 32.0, "ShapingStage".into(), "capability-reason".into()).capability_issue(Some("InvalidWebShapingAdvance".into())).build(),
        ShapingDecisionInfo::builder(TextRange::new(2, 3), Text::from("臣"), Text::from("臣"), "cjk".into(), 1, f32::INFINITY, "ShapingStage".into(), "infinite-capability".into()).capability_issue(Some("MissingInkBoundsFallback".into())).build(),
        ShapingDecisionInfo::builder(TextRange::new(1, 2), Text::from("零"), Text::from("零"), "cjk".into(), 1, 0.0, "ShapingStage".into(), "zero-advance".into()).build(),
        ShapingDecisionInfo::builder(TextRange::new(2, 3), Text::from("臣"), Text::from("臣"), "cjk".into(), 1, f32::NAN, "ShapingStage".into(), "nan-advance".into()).build(),
        ShapingDecisionInfo::builder(TextRange::new(2, 3), Text::from("臣"), Text::from("臣"), "cjk".into(), 1, f32::INFINITY, "ShapingStage".into(), "infinite-advance".into()).build(),
    ];
    let built = result(input, clusters, vec![GlyphRun::new(TextRange::new(0, 3), "cjk".into(), glyphs, 48.0)], LayoutDebugInfo::builder().shaping_decisions(decisions).build(), 48.0);
    let envelope = to_plan_with_diagnostics_json(&built, false, 0.5);
    for expected in ["\"name\":\"InvalidWebShapingAdvance\"", "\"reason\":\"capability-reason\"", "\"rangeStart\":0", "\"rangeEnd\":1", "\"displayText\":\"零\"", "\"advance\":\"0\"", "\"advance\":\"NaN\"", "\"advance\":\"Infinity\""] { assert!(envelope.contains(expected), "missing {expected}: {envelope}"); }
    assert!(!envelope.contains("\"advance\":\"32\""), "{envelope}");
    assert!(envelope.starts_with("{\"plan\":\""), "{envelope}");
}
