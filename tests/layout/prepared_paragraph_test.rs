use tiqian::core::geometry::{LayoutConstraints, Size, TextRange};
use tiqian::core::int_range::IntRange;
use tiqian::core::layout_model::{
    BopomofoDecisionInfo, BopomofoGlyphPlacement, BopomofoGlyphRole, Cluster,
    DecorationDecisionInfo, DecorationSegmentInfo, FontDecisionInfo, Glyph, GlyphRun,
    LayoutDebugInfo, LayoutResult, LineBox, PunctuationDecisionInfo, RubyDecisionInfo,
    ShapingDecisionInfo,
};
use tiqian::core::text::Text;
use tiqian::core::text_model::{
    DecorationKind, DecorationSpan, InlineBoxSpan, InlineObjectSpan, LayoutInput, ParagraphStyle,
    RubyKind, RubySpan, TextSpan, TextStyle, TiqianTextContent,
};
use tiqian::core::units::Ic;
use tiqian::layout::paragraph_layout_engine::{
    ExplainableStubParagraphLayoutEngine, ParagraphLayoutEngine,
};
use tiqian::layout::prepared_paragraph::{ecma_json_number, to_prepared_paragraph_json};

fn evidence_result() -> LayoutResult {
    let input = LayoutInput::builder(
        TiqianTextContent::builder(Text::from("。A"))
            .spans(vec![TextSpan {
                range: TextRange::new(1, 2),
                style: TextStyle::builder()
                    .font_size(20.0)
                    .font_weight(700)
                    .italic(true)
                    .build(),
            }])
            .build(),
        LayoutConstraints::with_defaults(48.0),
    )
    .decorations(vec![DecorationSpan {
        range: TextRange::new(0, 1),
        kind: DecorationKind::Emphasis,
    }])
    .inline_boxes(vec![InlineBoxSpan::with_edges(TextRange::new(0, 1), 2.0, 3.0)])
    .inline_objects(vec![InlineObjectSpan::with_fixed_boundaries(
        TextRange::new(1, 2),
        24.0,
        12.0,
        4.0,
    )])
    .build();
    let clusters = vec![
        Cluster::new(TextRange::new(0, 1), Text::from("。"), "cjk".to_owned(), 16.0),
        Cluster::new(TextRange::new(1, 2), Text::from("A"), "latin".to_owned(), 10.0),
    ];
    let glyph_runs = vec![
        GlyphRun::with_open_type_features(
            TextRange::new(0, 1),
            "cjk".to_owned(),
            vec![Glyph::builder(9, TextRange::new(0, 1), 16.0)
                .render_font_key(Some("Noto Serif CJK".to_owned()))
                .build()],
            16.0,
            vec!["kern".to_owned(), "liga".to_owned()],
        ),
        GlyphRun::new(
            TextRange::new(1, 2),
            "latin".to_owned(),
            vec![Glyph::builder(10, TextRange::new(1, 2), 24.0).build()],
            24.0,
        ),
    ];
    let lines = vec![
        LineBox::builder(
            TextRange::new(0, 2),
            IntRange::new(0, 1),
            20.0,
            0.0,
            24.0,
            40.0,
            40.0,
            40.0,
        )
        .build(),
    ];
    let ruby = RubyDecisionInfo::builder(
        TextRange::new(0, 1),
        Text::from("diǎn"),
        0,
        8.0,
        2.0,
        8.0,
        0.0,
    )
    .ascent(6.0)
    .font_families(vec!["RubyKai".to_owned()])
    .font_weight(500)
    .build();
    let bopomofo = BopomofoDecisionInfo::builder(
        TextRange::new(1, 2),
        Text::from("ㄟ"),
        0,
        vec![BopomofoGlyphPlacement::new(
            Text::from("ㄟ"),
            16.0,
            2.0,
            4.0,
            4.0,
            BopomofoGlyphRole::Symbol,
        )],
    )
    .font_families(vec!["BopomofoKai".to_owned()])
    .font_weight(700)
    .build();
    let debug = LayoutDebugInfo::builder()
        .font_decisions(vec![FontDecisionInfo {
            range: TextRange::new(1, 2),
            source_text: Text::from("A"),
            display_text: Text::from("A"),
            role: "LatinText".to_owned(),
            font_key: "latin".to_owned(),
            reason: "latin-run".to_owned(),
            substitution_reason: "none".to_owned(),
        }])
        .shaping_decisions(vec![
            ShapingDecisionInfo::builder(
                TextRange::new(0, 1),
                Text::from("。"),
                Text::from("。"),
                "cjk".to_owned(),
                1,
                16.0,
                "ShapingStage".to_owned(),
                "dash-reason".to_owned(),
            )
            .strategy(Some("PairedEmDash".to_owned()))
            .language(Some("zh-Hans".to_owned()))
            .resolved_face(Some("NotoSansCJK".to_owned()))
            .build(),
        ])
        .punctuation_decisions(vec![
            PunctuationDecisionInfo::builder(
                TextRange::new(0, 1),
                '。',
                "PauseOrStop".to_owned(),
                16.0,
                16.0,
                0.0,
                0.0,
                "centre".to_owned(),
            )
            .ink_containment_body_floor(Some(6.0))
            .ink_containment_applied(true)
            .build(),
        ])
        .ruby_decisions(vec![ruby])
        .bopomofo_decisions(vec![bopomofo])
        .decoration_segments(vec![DecorationSegmentInfo {
            source_range: TextRange::new(0, 1),
            kind: "ProperNoun".to_owned(),
            line_index: 0,
            left: 0.0,
            top: 20.0,
            right: 16.0,
            bottom: 22.0,
            open_start: false,
            open_end: false,
            reason: "proper-noun".to_owned(),
        }])
        .decoration_decisions(vec![
            DecorationDecisionInfo::builder(
                TextRange::new(0, 1),
                Text::from("。"),
                "Emphasis".to_owned(),
                true,
                "dot-applied".to_owned(),
            )
            .anchor_x(8.0)
            .anchor_y(22.0)
            .dot_diameter(2.0)
            .build(),
        ])
        .build();
    LayoutResult::with_debug(input, Size { width: 48.0, height: 24.0 }, clusters, glyph_runs, lines, debug)
}

#[test]
fn ecma_json_number_matches_kotlin_vectors() {
    for (value, expected) in [
        (0.0, "0"),
        (-0.0, "0"),
        (1.0, "1"),
        (200.0, "200"),
        (1.0e15, "999999986991104"),
        (1.0e16, "10000000272564224"),
        (1.0e20, "100000002004087730000"),
        (9_007_199_254_740_992.0, "9007199254740992"),
        (1.5, "1.5"),
        (12.5, "12.5"),
        (1_000_000.5, "1000000.5"),
        (0.1, "0.10000000149011612"),
        (0.45, "0.44999998807907104"),
        (0.05, "0.05000000074505806"),
        (0.01, "0.009999999776482582"),
        (0.0001, "0.00009999999747378752"),
        (0.00035, "0.0003499999875202775"),
        (1.0e21, "1.0000000200408773e+21"),
        (1.0e22, "9.999999778196308e+21"),
        (1.5e22, "1.4999999667294463e+22"),
        (2.5e22, "2.499999944549077e+22"),
        (1.5e24, "1.5000000207726418e+24"),
        (1.0e-7, "1.0000000116860974e-7"),
        (1.5e-7, "1.500000053056283e-7"),
        (-1.5, "-1.5"),
        (-2.5e-7, "-2.499999993688107e-7"),
        (5.960464477539063e-8, "5.960464477539062e-8"),
        (2.9802322387695312e-8, "2.9802322387695312e-8"),
        (1.7432641983032227, "1.7432641983032227"),
        (1_152_921_504_606_846_976.0, "1152921504606847000"),
        (5.684341886080802e-14, "5.684341886080801e-14"),
        (5.316911983139664e36, "5.316911983139663e+36"),
        (f32::from_bits(1), "1.401298464324817e-45"),
        (f32::from_bits(3), "4.203895392974451e-45"),
        (f32::from_bits(0x5AEDDA3D), "33474762504142850"),
        (f32::from_bits(0x5BB7FB0F), "103571925162262530"),
        (12_500_000.0, "12500000"),
    ] {
        assert_eq!(expected, ecma_json_number(value));
    }
    assert_eq!("NaN", ecma_json_number(f32::NAN));
    assert_eq!("Infinity", ecma_json_number(f32::INFINITY));
    assert_eq!("-Infinity", ecma_json_number(f32::NEG_INFINITY));
}

#[test]
fn render_evidence_is_append_only_and_emits_kotlin_cell_and_paragraph_fields() {
    let result = evidence_result();
    let plain = to_prepared_paragraph_json(&result, false);
    let evidence = to_prepared_paragraph_json(&result, true);

    assert_eq!(plain, to_prepared_paragraph_json(&result, false));
    let pure_input = LayoutInput::builder(
        TiqianTextContent::new(Text::from("中文")),
        LayoutConstraints::with_defaults(320.0),
    )
    .paragraph_style(ParagraphStyle::builder().first_line_indent(Some(Ic::ZERO)).build())
    .build();
    let pure_result = ExplainableStubParagraphLayoutEngine::default().layout(pure_input);
    let pure_plain = to_prepared_paragraph_json(&pure_result, false);
    let pure_evidence = to_prepared_paragraph_json(&pure_result, true);
    assert!(pure_evidence.starts_with(pure_plain.strip_suffix('}').unwrap()));
    for expected in [
        "\"renderFontFamily\":\"Noto Serif CJK\"",
        "\"dashStrategy\":\"PairedEmDash\"",
        "\"shapingLanguage\":\"zh-Hans\"",
        "\"resolvedFace\":\"NotoSansCJK\"",
        "\"glyphIds\":\"9\"",
        "\"punctuationInkFloor\":6",
        "\"latin\":true",
        "\"style\":{\"fontSize\":20,\"fontWeight\":700,\"italic\":true}",
        "\"inlineObject\":24",
        "\"advance\":10",
        "\"fontSize\":16",
        "\"overlayWidth\":48",
        "\"emphasisRanges\":[[0,1]]",
        "\"inlineEdges\":[{\"offset\":0,\"inlineStart\":2},{\"offset\":1,\"inlineEnd\":3}]",
        "\"rubyDecisions\":[{\"baseRangeStart\":0",
        "\"ascent\":6",
        "\"bopomofoDecisions\":[{\"baseRangeStart\":1",
        "\"role\":\"Symbol\"",
        "\"decorationSegments\":[{\"kind\":\"ProperNoun\"",
        "\"emphasisDots\":[{\"clusterRangeStart\":0,\"anchorX\":8,\"anchorY\":22,\"dotDiameter\":2}]",
    ] {
        assert!(evidence.contains(expected), "missing {expected}: {evidence}");
    }
}

#[test]
fn real_ruby_and_bopomofo_layouts_emit_evidence_only_when_requested() {
    let input = LayoutInput::builder(
        TiqianTextContent::new(Text::from("中文")),
        LayoutConstraints::with_defaults(320.0),
    )
    .paragraph_style(ParagraphStyle::builder().first_line_indent(Some(Ic::ZERO)).build())
    .ruby_spans(vec![
        RubySpan::new(TextRange::new(0, 1), Text::from("zhōng")),
        RubySpan::with_kind(TextRange::new(1, 2), Text::from("ㄨㄣˊ"), RubyKind::Bopomofo),
    ])
    .build();
    let result = ExplainableStubParagraphLayoutEngine::default().layout(input);
    let plain = to_prepared_paragraph_json(&result, false);
    let evidence = to_prepared_paragraph_json(&result, true);

    assert!(!plain.contains("rubyDecisions"));
    assert!(!plain.contains("bopomofoDecisions"));
    assert!(evidence.contains("\"rubyDecisions\":["));
    assert!(evidence.contains("\"ascent\":"));
    assert!(evidence.contains("\"bopomofoDecisions\":["));
    assert!(evidence.contains("\"placements\":["));
}
