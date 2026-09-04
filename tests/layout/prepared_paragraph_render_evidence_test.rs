use tiqian::core::geometry::{text_range, LayoutConstraints};
use tiqian::core::text::Text;
use tiqian::core::text_model::{
    DecorationKind, DecorationSpan, InlineBoxSpan, LayoutInput, RubyKind, RubySpan, TextSpan,
    TextStyle, TiqianTextContent,
};
use tiqian::layout::paragraph_layout_engine::{
    ExplainableStubParagraphLayoutEngine, ParagraphLayoutEngine,
};
use tiqian::layout::prepared_paragraph::to_prepared_paragraph_json;

fn layout(input: LayoutInput) -> tiqian::core::layout_model::LayoutResult {
    ExplainableStubParagraphLayoutEngine::default().layout(input)
}

#[test]
fn pinyin_ruby_emits_ruby_decisions() {
    let result = layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from("北京是首都。")),
            LayoutConstraints::with_defaults(200.0),
        )
        .ruby_spans(vec![RubySpan::new(text_range(0, 2), Text::from("Běijīng"))])
        .build(),
    );

    assert!(!to_prepared_paragraph_json(&result, false).contains("rubyDecisions"));
    let evidence = to_prepared_paragraph_json(&result, true);
    for expected in [
        "\"rubyDecisions\":[",
        "\"baseRangeStart\":0",
        "\"baseRangeEnd\":2",
        "\"text\":\"Běijīng\"",
        "\"centerX\":",
        "\"baselineY\":",
        "\"fontSize\":",
        "\"ascent\":",
        "\"fontWeight\":500",
    ] {
        assert!(evidence.contains(expected), "missing {expected}: {evidence}");
    }
}

#[test]
fn bopomofo_ruby_emits_bopomofo_decisions() {
    let result = layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from("好文。")),
            LayoutConstraints::with_defaults(200.0),
        )
        .ruby_spans(vec![RubySpan::with_kind(
            text_range(0, 1),
            Text::from("ㄏㄠˇ"),
            RubyKind::Bopomofo,
        )])
        .build(),
    );

    assert!(!to_prepared_paragraph_json(&result, false).contains("bopomofoDecisions"));
    let evidence = to_prepared_paragraph_json(&result, true);
    assert!(evidence.contains("\"bopomofoDecisions\":["), "{evidence}");
    assert!(evidence.contains("\"placements\":["), "{evidence}");
    assert!(evidence.contains("\"role\":\""), "{evidence}");
    assert!(!evidence.contains("\"rubyDecisions\":"), "{evidence}");
}

#[test]
fn decorations_emit_segments_dots_and_ranges() {
    let result = layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from("鲁迅的小说在中国现代文学里很重要。")),
            LayoutConstraints::with_defaults(200.0),
        )
        .decorations(vec![
            DecorationSpan { range: text_range(0, 2), kind: DecorationKind::ProperNoun },
            DecorationSpan { range: text_range(3, 5), kind: DecorationKind::BookTitle },
            DecorationSpan { range: text_range(6, 9), kind: DecorationKind::Emphasis },
        ])
        .build(),
    );

    let plain = to_prepared_paragraph_json(&result, false);
    for omitted in ["decorationSegments", "emphasisDots", "emphasisRanges"] {
        assert!(!plain.contains(omitted), "{plain}");
    }
    let evidence = to_prepared_paragraph_json(&result, true);
    for expected in [
        "\"decorationSegments\":[",
        "\"kind\":\"ProperNoun\"",
        "\"kind\":\"BookTitle\"",
        "\"sourceRangeStart\":0",
        "\"emphasisRanges\":[[6,9]]",
    ] {
        assert!(evidence.contains(expected), "missing {expected}: {evidence}");
    }
    if evidence.contains("\"emphasisDots\"") {
        assert!(evidence.contains("\"anchorX\":"), "{evidence}");
        assert!(evidence.contains("\"dotDiameter\":"), "{evidence}");
    }
}

#[test]
fn style_delta_emits_per_cell_style_block() {
    let result = layout(
        LayoutInput::builder(
            TiqianTextContent::builder(Text::from("普通字与小字混排的段落。"))
                .spans(vec![TextSpan {
                    range: text_range(4, 6),
                    style: TextStyle::builder().font_size(12.0).font_weight(700).build(),
                }])
                .build(),
            LayoutConstraints::with_defaults(200.0),
        )
        .build(),
    );

    assert!(!to_prepared_paragraph_json(&result, false).contains("\"style\":{"));
    let evidence = to_prepared_paragraph_json(&result, true);
    assert!(evidence.contains("\"style\":{\"fontSize\":"), "{evidence}");
    assert!(evidence.contains("\"fontWeight\":700"), "{evidence}");
}

#[test]
fn inline_boxes_emit_inline_edges() {
    let result = layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from("文字与边距。")),
            LayoutConstraints::with_defaults(200.0),
        )
        .inline_boxes(vec![InlineBoxSpan::with_edges(text_range(0, 1), 2.0, 3.0)])
        .build(),
    );

    assert!(!to_prepared_paragraph_json(&result, false).contains("inlineEdges"));
    let evidence = to_prepared_paragraph_json(&result, true);
    for expected in [
        "\"inlineEdges\":[",
        "\"offset\":0",
        "\"offset\":1",
        "\"inlineStart\":2",
        "\"inlineEnd\":3",
    ] {
        assert!(evidence.contains(expected), "missing {expected}: {evidence}");
    }
}
