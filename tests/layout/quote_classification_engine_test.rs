use tiqian::common::HashSet;
use tiqian::core::geometry::{LayoutConstraints, Rect, TextRange};
use tiqian::core::layout_model::{Cluster, Glyph, GlyphRun};
use tiqian::core::layout_queries::positioned_clusters;
use tiqian::core::text::Text;
use tiqian::core::text_model::{
    LayoutInput, ParagraphStyle, TextSpan, TextStyle, TiqianTextContent,
};
use tiqian::core::units::Ic;
use tiqian::layout::line_breaker::LookaheadLineBreaker;
use tiqian::layout::paragraph_layout_engine::{
    ExplainableStubParagraphLayoutEngine, ParagraphLayoutEngine,
};
use tiqian::linebreak::hyphenation::NoHyphenator;
use tiqian::shaping::text_shaper::{
    ExplainableStubTextShaper, ShapingInput, ShapingResult, TextShaper,
};

fn layout(text: &str) -> tiqian::core::layout_model::LayoutResult {
    ExplainableStubParagraphLayoutEngine::default().layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from(text)),
            LayoutConstraints::with_defaults(320.0),
        )
        .paragraph_style(
            ParagraphStyle::builder()
                .first_line_indent(Some(Ic::ZERO))
                .build(),
        )
        .build(),
    )
}

#[test]
fn latin_technical_punctuation_stays_in_latin_run() {
    let result = layout("well-known/path");

    assert_eq!(
        "well-known/path",
        result
            .clusters
            .iter()
            .map(|cluster| cluster.text.as_str())
            .collect::<String>()
    );
    assert!(
        result
            .clusters
            .iter()
            .all(|cluster| cluster.font_key == "latin-primary")
    );
    assert!(
        result
            .clusters
            .iter()
            .any(|cluster| cluster.text == "well-")
    );
}

#[test]
fn ascii_brackets_remain_latin_inside_cjk_text() {
    let result = layout("中文(中文)");

    for bracket in ["(", ")"] {
        let cluster = result
            .clusters
            .iter()
            .find(|cluster| cluster.text == bracket)
            .unwrap();
        assert_eq!("latin-primary", cluster.font_key, "{bracket}");
        let decision = result
            .debug
            .font_decisions
            .iter()
            .find(|decision| decision.range == cluster.range)
            .unwrap();
        assert_eq!("LatinText", decision.role, "{bracket}");
    }
}

#[test]
fn western_quote_pair_reaches_latin_font_pipeline_without_cjk_geometry() {
    let result = layout("“Hello” world");

    assert_eq!(3, result.clusters.len());
    assert_eq!("“Hello”", result.clusters[0].text);
    assert_eq!("latin-primary", result.clusters[0].font_key);
    let overrides = result
        .debug
        .role_overrides
        .iter()
        .filter(|override_info| matches!(override_info.range.start(), 0 | 6))
        .collect::<Vec<_>>();
    assert_eq!(2, overrides.len());
    assert!(
        overrides
            .iter()
            .all(|override_info| override_info.overridden_role == "LatinText")
    );
    assert!(
        result
            .debug
            .punctuation_decisions
            .iter()
            .all(|decision| !matches!(decision.ch, '“' | '”'))
    );
}

#[test]
fn cjk_quote_pair_reaches_punctuation_geometry_with_outer_context_evidence() {
    let result = layout("中“文”中");

    let quote_overrides = result
        .debug
        .role_overrides
        .iter()
        .filter(|override_info| matches!(override_info.range.start(), 1 | 3))
        .collect::<Vec<_>>();
    assert_eq!(2, quote_overrides.len());
    assert!(
        quote_overrides
            .iter()
            .all(|override_info| override_info.overridden_role == "CjkPunctuation")
    );
    assert!(
        quote_overrides
            .iter()
            .all(|override_info| override_info.source == "PairedPunctuationOuterScriptContext")
    );
    assert_eq!(
        vec![1, 3],
        result
            .debug
            .punctuation_decisions
            .iter()
            .filter(|decision| matches!(decision.ch, '“' | '”'))
            .map(|decision| decision.range.start())
            .collect::<Vec<_>>(),
    );
}

#[test]
fn mixed_paragraph_start_quote_uses_paragraph_language_fallback() {
    let result = layout("“Json是谁？”");

    let quote_overrides = result
        .debug
        .role_overrides
        .iter()
        .filter(|override_info| matches!(override_info.range.start(), 0 | 8))
        .collect::<Vec<_>>();
    assert_eq!(2, quote_overrides.len());
    assert!(
        quote_overrides
            .iter()
            .all(|override_info| override_info.overridden_role == "CjkPunctuation")
    );
    assert!(
        quote_overrides
            .iter()
            .all(|override_info| override_info.source == "ParagraphLanguageQuoteContext")
    );
    assert_eq!("“Json是谁？”", result.input.content.text);
}

#[test]
fn contraction_apostrophe_stays_latin_inside_cjk_single_quotes() {
    let result = layout("中‘that’s’中");

    let contraction = result
        .debug
        .font_decisions
        .iter()
        .find(|decision| decision.source_text == "that’s")
        .unwrap();
    assert_eq!("LatinText", contraction.role);
    assert_eq!("latin-primary", contraction.font_key);
    assert!(
        result
            .debug
            .punctuation_decisions
            .iter()
            .all(|decision| decision.range.start() != 6)
    );
}

#[test]
fn latin_word_internal_curly_quotes_stay_in_latin_run_inside_mixed_paragraph() {
    let result = layout("中文 Latin: le“t”ters 中文");

    let word = result
        .clusters
        .iter()
        .find(|cluster| cluster.text == "le“t”ters")
        .unwrap();
    assert_eq!("latin-primary", word.font_key);
    let overrides = result
        .debug
        .role_overrides
        .iter()
        .filter(|override_info| matches!(override_info.source_text.as_str(), "“" | "”"))
        .collect::<Vec<_>>();
    assert_eq!(2, overrides.len());
    assert!(
        overrides
            .iter()
            .all(|override_info| override_info.overridden_role == "LatinText")
    );
}

#[test]
fn latin_word_internal_quotes_support_supplementary_letters() {
    let result = layout("中文 a“𝐀”b 中文");

    let overrides = result
        .debug
        .role_overrides
        .iter()
        .filter(|override_info| matches!(override_info.source_text.as_str(), "“" | "”"))
        .collect::<Vec<_>>();
    assert_eq!(2, overrides.len());
    assert!(overrides.iter().all(|override_info| {
        override_info.overridden_role == "LatinText"
            && override_info.source == "NonCjkWordInternalQuotePair"
    }));
}

#[test]
fn numeric_word_internal_quotes_stay_latin() {
    let result = layout("中文1“2”3中文");

    let overrides = result
        .debug
        .role_overrides
        .iter()
        .filter(|override_info| matches!(override_info.source_text.as_str(), "“" | "”"))
        .collect::<Vec<_>>();
    assert_eq!(2, overrides.len());
    assert!(overrides.iter().all(|override_info| {
        override_info.overridden_role == "LatinText"
            && override_info.source == "NonCjkWordInternalQuotePair"
    }));
}

#[test]
fn empty_word_internal_quotes_stay_latin() {
    let result = layout("中文a“”b中文");

    let overrides = result
        .debug
        .role_overrides
        .iter()
        .filter(|override_info| matches!(override_info.source_text.as_str(), "“" | "”"))
        .collect::<Vec<_>>();
    assert_eq!(2, overrides.len());
    assert!(overrides.iter().all(|override_info| {
        override_info.overridden_role == "LatinText"
            && override_info.source == "NonCjkWordInternalQuotePair"
    }));
}

#[test]
fn quote_roles_survive_style_and_source_boundaries() {
    let text = "中‘that’s’中";
    let result = ExplainableStubParagraphLayoutEngine::default().layout(
        LayoutInput::builder(
            TiqianTextContent::builder(Text::from(text))
                .spans(vec![TextSpan {
                    range: TextRange::new(2, 7),
                    style: TextStyle::builder().font_weight(700).build(),
                }])
                .source_boundaries(HashSet::from([1, 2, 6, 7, 8, 9]))
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

    let roles_by_index = result
        .debug
        .role_overrides
        .iter()
        .map(|override_info| {
            (
                override_info.range.start(),
                override_info.overridden_role.as_str(),
            )
        })
        .collect::<std::collections::HashMap<_, _>>();
    assert_eq!(Some(&"CjkPunctuation"), roles_by_index.get(&1));
    assert_eq!(Some(&"LatinText"), roles_by_index.get(&6));
    assert_eq!(Some(&"CjkPunctuation"), roles_by_index.get(&8));
    assert_eq!(
        "latin-primary",
        result
            .clusters
            .iter()
            .find(|cluster| cluster.range.start() == 6)
            .unwrap()
            .font_key
    );
    assert_eq!(
        text,
        result
            .clusters
            .iter()
            .map(|cluster| cluster.text.as_str())
            .collect::<String>()
    );
}

#[test]
fn adjacent_quoted_list_items_keep_cjk_quote_geometry_across_mixed_content() {
    for text in [
        "便延伸出了“乃子”“大波”“大灯”“大雷”“大扎”“对A”“波霸”这些词",
        "这些太直白了是吧，\n “欧派”“double”“double may”呢",
    ] {
        let result = layout(text);
        let quote_indices = text
            .encode_utf16()
            .enumerate()
            .filter_map(|(index, code_unit)| {
                matches!(code_unit, 0x2018 | 0x2019 | 0x201C | 0x201D).then_some(index as i32)
            })
            .collect::<HashSet<_>>();

        assert_eq!(
            quote_indices,
            result
                .debug
                .font_decisions
                .iter()
                .filter(|decision| {
                    decision.role == "CjkPunctuation"
                        && quote_indices.contains(&decision.range.start())
                })
                .map(|decision| decision.range.start())
                .collect::<HashSet<_>>(),
            "{text}",
        );
        assert_eq!(
            quote_indices,
            result
                .debug
                .punctuation_decisions
                .iter()
                .filter(|decision| matches!(decision.ch, '‘' | '’' | '“' | '”'))
                .map(|decision| decision.range.start())
                .collect::<HashSet<_>>(),
            "{text}",
        );
        let final_open = text
            .encode_utf16()
            .enumerate()
            .filter_map(|(index, unit)| (unit == 0x201C).then_some(index as i32))
            .last()
            .unwrap();
        let final_close = text
            .encode_utf16()
            .enumerate()
            .filter_map(|(index, unit)| (unit == 0x201D).then_some(index as i32))
            .last()
            .unwrap();
        let final_overrides = result
            .debug
            .role_overrides
            .iter()
            .filter(|override_info| {
                matches!(override_info.range.start(), index if index == final_open || index == final_close)
            })
            .collect::<Vec<_>>();
        assert_eq!(2, final_overrides.len(), "{text}");
        assert!(
            final_overrides
                .iter()
                .all(|override_info| override_info.source == "PairedPunctuationOuterScriptContext"),
            "{text}",
        );
        assert_eq!(text, result.input.content.text);
    }
}

#[test]
fn mi10s_adjacent_latin_transcriptions_keep_final_quote_pair_in_cjk_context() {
    let text = "所以这个和 “骑ji” “说shui”“斜xiá”不一样，港台是从众的，大陆读音大多数源自韵书。";
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.line_breaker = Box::new(LookaheadLineBreaker::default());
    engine.hyphenator = &NoHyphenator;
    let result = engine.layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from(text)),
            LayoutConstraints::with_defaults(160.0),
        )
        .paragraph_style(
            ParagraphStyle::builder()
                .first_line_indent(Some(Ic::ZERO))
                .build(),
        )
        .build(),
    );

    let final_open = text
        .encode_utf16()
        .enumerate()
        .filter_map(|(index, unit)| (unit == 0x201C).then_some(index as i32))
        .last()
        .unwrap();
    let final_close = text
        .encode_utf16()
        .enumerate()
        .filter_map(|(index, unit)| (unit == 0x201D).then_some(index as i32))
        .last()
        .unwrap();
    let final_overrides = result
        .debug
        .role_overrides
        .iter()
        .filter(|override_info| {
            matches!(override_info.range.start(), index if index == final_open || index == final_close)
        })
        .collect::<Vec<_>>();
    assert_eq!(2, final_overrides.len());
    assert!(
        final_overrides
            .iter()
            .all(|override_info| override_info.overridden_role == "CjkPunctuation")
    );
    assert!(
        final_overrides
            .iter()
            .all(|override_info| override_info.source == "PairedPunctuationOuterScriptContext")
    );
    assert!(result.lines.iter().all(|line| {
        !result
            .input
            .content
            .text
            .slice_text(line.range)
            .as_str()
            .starts_with('”')
    }));
}

#[test]
fn skips_neutral_dash_before_latin_quote_pair_in_layout() {
    let result = layout("English — “hello”");

    let quoted = result
        .clusters
        .iter()
        .find(|cluster| cluster.text.as_str().contains("“hello”"))
        .unwrap();
    assert_eq!("latin-primary", quoted.font_key);
}

#[test]
fn records_role_overrides_for_resolved_quote_pairs() {
    let result = layout("“Hello” world");

    let opening = result
        .debug
        .role_overrides
        .iter()
        .find(|override_info| override_info.range.start() == 0)
        .unwrap();
    let closing = result
        .debug
        .role_overrides
        .iter()
        .find(|override_info| override_info.range.start() == 6)
        .unwrap();
    assert_eq!("LatinText", opening.overridden_role);
    assert_eq!("CjkPunctuation", opening.original_role);
    assert_eq!("PairedPunctuationOuterScriptContext", opening.source);
    assert_eq!("LatinText", closing.overridden_role);
}

#[test]
fn keeps_numbered_cjk_quote_pair_on_cjk_face() {
    let text = "1.“你知道李白是怎么死的吗？”";
    let result = layout(text);

    let opening = result
        .debug
        .font_decisions
        .iter()
        .find(|decision| decision.range.start() == 2)
        .unwrap();
    assert_eq!("CjkPunctuation", opening.role);
    assert_eq!("cjk-primary", opening.font_key);

    let override_info = result
        .debug
        .role_overrides
        .iter()
        .find(|override_info| override_info.range.start() == 2)
        .unwrap();
    assert_eq!(
        "PairedPunctuationContentScriptContext",
        override_info.source
    );
    assert_eq!("quoted-content-script", override_info.reason);
    assert_eq!("CjkPunctuation", override_info.overridden_role);
}

struct ProportionalQuoteTextShaper;

impl TextShaper for ProportionalQuoteTextShaper {
    fn shape(&self, input: &ShapingInput) -> ShapingResult {
        let result = ExplainableStubTextShaper.shape(input);
        if input.display_text.as_str() != "“" && input.display_text.as_str() != "”" {
            return result;
        }
        assert_eq!(vec!["fwid=1"], input.open_type_features);
        let advance = 6.0;
        ShapingResult::with_decisions(
            result
                .clusters
                .iter()
                .cloned()
                .map(|cluster| Cluster { advance, ..cluster })
                .collect(),
            result
                .glyph_runs
                .iter()
                .map(|run| {
                    GlyphRun::new(
                        run.range,
                        run.font_key.clone(),
                        run.glyphs
                            .iter()
                            .map(|glyph| {
                                Glyph::builder(glyph.id, glyph.cluster_range, advance)
                                    .bounds(Some(Rect {
                                        left: 1.0,
                                        top: -10.0,
                                        right: 5.0,
                                        bottom: 0.0,
                                    }))
                                    .build()
                            })
                            .collect(),
                        advance,
                    )
                })
                .collect(),
            result
                .decisions
                .iter()
                .cloned()
                .map(|decision| tiqian::core::layout_model::ShapingDecisionInfo {
                    advance,
                    ..decision
                })
                .collect(),
        )
    }
}

#[test]
fn requests_full_width_cjk_quotes_and_synthesizes_cell_for_proportional_glyphs() {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.text_shaper = Box::new(ProportionalQuoteTextShaper);
    let result = engine.layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from("中“文”中")),
            LayoutConstraints::with_defaults(320.0),
        )
        .paragraph_style(
            ParagraphStyle::builder()
                .first_line_indent(Some(Ic::ZERO))
                .build(),
        )
        .build(),
    );

    let opening = result
        .clusters
        .iter()
        .find(|cluster| cluster.text == "“")
        .unwrap();
    let closing = result
        .clusters
        .iter()
        .find(|cluster| cluster.text == "”")
        .unwrap();
    assert_eq!(16.0, opening.advance);
    assert_eq!(16.0, closing.advance);
    assert_eq!(10.0, opening.glyph_inline_shift);
    assert_eq!(0.0, closing.glyph_inline_shift);

    let opening_decision = result
        .debug
        .punctuation_decisions
        .iter()
        .find(|decision| decision.ch == '“')
        .unwrap();
    let closing_decision = result
        .debug
        .punctuation_decisions
        .iter()
        .find(|decision| decision.ch == '”')
        .unwrap();
    assert_eq!(10.0, opening_decision.advance_expansion);
    assert_eq!(
        Some("UnderwidthPunctuationFullWidthBoxPlacement".to_owned()),
        opening_decision.glyph_placement_reason
    );
    assert_eq!(None, closing_decision.glyph_placement_reason);
    assert_eq!(
        "InkBoundsFittedBodyCompression",
        opening_decision.geometry_source
    );
    assert_eq!(
        "InkBoundsFittedBodyCompression",
        closing_decision.geometry_source
    );

    let positioned = positioned_clusters(&result);
    let positioned_opening = positioned
        .iter()
        .find(|cluster| cluster.range == opening.range)
        .unwrap();
    let positioned_closing = positioned
        .iter()
        .find(|cluster| cluster.range == closing.range)
        .unwrap();
    assert_eq!(positioned_opening.left + 10.0, positioned_opening.draw_x);
    assert_eq!(positioned_closing.left, positioned_closing.draw_x);

    let line_start = engine.layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from("“文")),
            LayoutConstraints::with_defaults(320.0),
        )
        .paragraph_style(
            ParagraphStyle::builder()
                .first_line_indent(Some(Ic::ZERO))
                .build(),
        )
        .build(),
    );
    let line_start_quote = line_start.clusters.first().unwrap();
    let line_start_positioned = positioned_clusters(&line_start);
    assert_eq!(8.0, line_start_quote.advance);
    assert_eq!(2.0, line_start_positioned[0].draw_x);
    assert_eq!(8.0, line_start_positioned[1].left);
}
