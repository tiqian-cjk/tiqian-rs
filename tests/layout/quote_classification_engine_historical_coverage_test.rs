use tiqian::common::HashSet;
use tiqian::core::geometry::{LayoutConstraints, Rect, ScalarOffset, scalar_offset, text_range};
use tiqian::core::layout_model::{Cluster, Glyph, GlyphRun, ShapingDecisionInfo};
use tiqian::core::layout_queries::positioned_clusters;
use tiqian::core::text::Text;
use tiqian::core::text_model::{LayoutInput, ParagraphStyle, TextSpan, TextStyle, TiqianTextContent};
use tiqian::core::units::Ic;
use tiqian::layout::line_breaker::LookaheadLineBreaker;
use tiqian::layout::paragraph_layout_engine::{ExplainableStubParagraphLayoutEngine, ParagraphLayoutEngine};
use tiqian::linebreak::hyphenation::NoHyphenator;
use tiqian::shaping::text_shaper::{ExplainableStubTextShaper, ShapingInput, ShapingResult, TextShaper};

fn layout(text: &str) -> tiqian::core::layout_model::LayoutResult {
    ExplainableStubParagraphLayoutEngine::default().layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from(text)),
            LayoutConstraints::with_defaults(320.0),
        )
        .paragraph_style(ParagraphStyle::builder().first_line_indent(Some(Ic::ZERO)).build())
        .build(),
    )
}

fn scalar_index_of(text: &str, needle: char) -> ScalarOffset {
    scalar_offset(text.find(needle).map(|byte_index| text[..byte_index].chars().count() as i32).unwrap())
}

fn scalar_last_index_of(text: &str, needle: char) -> ScalarOffset {
    scalar_offset(text.rfind(needle).map(|byte_index| text[..byte_index].chars().count() as i32).unwrap())
}

fn curly_quote_indices(text: &str) -> HashSet<ScalarOffset> {
    text.chars()
        .enumerate()
        .filter_map(|(index, character)| matches!(character, '‘' | '’' | '“' | '”').then_some(scalar_offset(index as i32)))
        .collect()
}

#[test]
fn keeps_latin_technical_punctuation_in_latin_run() {
    let result = layout("well-known/path");
    assert_eq!("well-known/path", result.clusters.iter().map(|cluster| cluster.text.as_str()).collect::<String>());
    assert!(result.clusters.iter().all(|cluster| cluster.font_key == "latin-primary"));
    assert!(result.clusters.iter().any(|cluster| cluster.text == "well-"));
}

#[test]
fn classifies_ascii_brackets_as_latin_regardless_of_surrounding_context() {
    let result = layout("中文(English)中文");
    let latin_cluster = result.clusters.iter().find(|cluster| cluster.text == "(English)").unwrap();
    assert_eq!("latin-primary", latin_cluster.font_key);
    assert_eq!("LatinText", result.debug.font_decisions.iter().find(|decision| decision.source_text == "(English)").unwrap().role);
}

#[test]
fn classifies_ascii_brackets_as_latin_inside_pure_cjk_content() {
    let result = layout("中文(中文)");
    for bracket in ["(", ")"] {
        assert_eq!("latin-primary", result.clusters.iter().find(|cluster| cluster.text == bracket).unwrap().font_key);
    }
}

#[test]
fn ascii_closing_bracket_with_cjk_interior_is_forbidden_at_line_start() {
    let text = "如今已占据超七成份额(国产品牌)，互联网大厂排队抢购？";
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.line_breaker = Box::new(LookaheadLineBreaker::default());
    let result = engine.layout(
        LayoutInput::builder(TiqianTextContent::new(Text::from(text)), LayoutConstraints::with_defaults(232.0))
            .paragraph_style(ParagraphStyle::builder().first_line_indent(Some(Ic::ZERO)).build())
            .build(),
    );
    let source = Text::from(text);
    assert!(result.lines.iter().all(|line| !source.slice_text(line.range).as_str().starts_with(')')), "{:#?}", result.lines);
    assert_eq!("latin-primary", result.clusters.iter().find(|cluster| cluster.text == ")").unwrap().font_key);
}

#[test]
fn ascii_opening_bracket_with_cjk_interior_is_forbidden_at_line_end() {
    let text = "如今已占据超七成份额(国产品牌)，互联网大厂排队抢购？";
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.line_breaker = Box::new(LookaheadLineBreaker::default());
    let result = engine.layout(
        LayoutInput::builder(TiqianTextContent::new(Text::from(text)), LayoutConstraints::with_defaults(168.0))
            .paragraph_style(ParagraphStyle::builder().first_line_indent(Some(Ic::ZERO)).build())
            .build(),
    );
    let source = Text::from(text);
    assert!(result.lines.iter().all(|line| !source.slice_text(line.range).as_str().ends_with('(')), "{:#?}", result.lines);
    assert_eq!("latin-primary", result.clusters.iter().find(|cluster| cluster.text == "(").unwrap().font_key);
}

#[test]
fn keeps_text_start_latin_quote_pair_in_latin_run() {
    let result = layout("“Hello” world");
    assert_eq!(3, result.clusters.len());
    assert_eq!("“Hello”", result.clusters[0].text);
    assert_eq!("latin-primary", result.clusters[0].font_key);
    assert!(result.debug.font_decisions.iter().any(|decision| decision.source_text == "“Hello” world" && decision.role == "LatinText"));
}

#[test]
fn mixed_quote_contexts_reach_the_font_and_punctuation_pipeline() {
    let text = "中“文”中；that’s；（如 ‘O’, ‘Q’）；他说：“She said ‘hello’.”";
    let result = ExplainableStubParagraphLayoutEngine::default().layout(
        LayoutInput::builder(TiqianTextContent::new(Text::from(text)), LayoutConstraints::with_defaults(1_000.0))
            .paragraph_style(ParagraphStyle::builder().first_line_indent(Some(Ic::ZERO)).build())
            .build(),
    );
    let cjk_quote_indices = HashSet::from([
        scalar_index_of(text, '“'), scalar_index_of(text, '”'), scalar_last_index_of(text, '“'), scalar_last_index_of(text, '”'),
    ]);
    let all_quote_indices = curly_quote_indices(text);
    for index in &cjk_quote_indices {
        assert_eq!("CjkPunctuation", result.debug.font_decisions.iter().find(|decision| *index >= decision.range.start() && *index < decision.range.end()).unwrap().role);
    }
    for index in all_quote_indices.difference(&cjk_quote_indices) {
        assert_eq!("LatinText", result.debug.font_decisions.iter().find(|decision| *index >= decision.range.start() && *index < decision.range.end()).unwrap().role);
    }
    assert_eq!(cjk_quote_indices, result.debug.punctuation_decisions.iter().filter(|decision| matches!(decision.ch, '‘' | '’' | '“' | '”')).map(|decision| decision.range.start()).collect());
    let overrides: std::collections::HashMap<_, _> = result.debug.role_overrides.iter().map(|override_info| (override_info.range.start(), override_info.overridden_role.as_str())).collect();
    assert_eq!(all_quote_indices, overrides.keys().copied().collect());
    for (index, role) in overrides {
        assert_eq!(if cjk_quote_indices.contains(&index) { "CjkPunctuation" } else { "LatinText" }, role);
    }
    assert_eq!(text, result.input.content.text);
}

#[test]
fn quote_roles_survive_style_and_source_boundaries() {
    let text = "中‘that’s’中";
    let result = ExplainableStubParagraphLayoutEngine::default().layout(
        LayoutInput::builder(
            TiqianTextContent::builder(Text::from(text))
                .spans(vec![TextSpan { range: text_range(2, 7), style: TextStyle::builder().font_weight(700).build() }])
                .source_boundaries(HashSet::from([scalar_offset(1), scalar_offset(2), scalar_offset(6), scalar_offset(7), scalar_offset(8), scalar_offset(9)]))
                .build(),
            LayoutConstraints::with_defaults(320.0),
        )
        .paragraph_style(ParagraphStyle::builder().first_line_indent(Some(Ic::ZERO)).build())
        .build(),
    );
    let roles: std::collections::HashMap<_, _> = result.debug.role_overrides.iter().map(|override_info| (override_info.range.start(), override_info.overridden_role.as_str())).collect();
    assert_eq!(Some(&"CjkPunctuation"), roles.get(&scalar_offset(1)));
    assert_eq!(Some(&"LatinText"), roles.get(&scalar_offset(6)));
    assert_eq!(Some(&"CjkPunctuation"), roles.get(&scalar_offset(8)));
    assert_eq!("latin-primary", result.clusters.iter().find(|cluster| cluster.range.start().value() == 6).unwrap().font_key);
    assert_eq!(text, result.clusters.iter().map(|cluster| cluster.text.as_str()).collect::<String>());
}

#[test]
fn adjacent_quoted_list_items_keep_cjk_quote_geometry_across_mixed_content() {
    for text in [
        "便延伸出了“乃子”“大波”“大灯”“大雷”“大扎”“对A”“波霸”这些词",
        "这些太直白了是吧，\n “欧派”“double”“double may”呢",
    ] {
        let result = ExplainableStubParagraphLayoutEngine::default().layout(
            LayoutInput::builder(TiqianTextContent::new(Text::from(text)), LayoutConstraints::with_defaults(1_000.0))
                .paragraph_style(ParagraphStyle::builder().first_line_indent(Some(Ic::ZERO)).build())
                .build(),
        );
        let quote_indices = curly_quote_indices(text);
        assert_eq!(quote_indices, result.debug.font_decisions.iter().filter(|decision| decision.role == "CjkPunctuation" && quote_indices.contains(&decision.range.start())).map(|decision| decision.range.start()).collect());
        assert_eq!(quote_indices, result.debug.punctuation_decisions.iter().filter(|decision| matches!(decision.ch, '‘' | '’' | '“' | '”')).map(|decision| decision.range.start()).collect());
        let final_indices: HashSet<ScalarOffset> = HashSet::from([
            scalar_last_index_of(text, '“'),
            scalar_last_index_of(text, '”'),
        ]);
        let final_overrides: Vec<_> = result.debug.role_overrides.iter().filter(|override_info| final_indices.contains(&override_info.range.start())).collect();
        assert_eq!(2, final_overrides.len(), "{text}");
        assert!(final_overrides.iter().all(|override_info| override_info.source == "PairedPunctuationOuterScriptContext"), "{text}");
        assert_eq!(text, result.input.content.text);
    }
}

#[test]
fn mi10s_adjacent_latin_transcriptions_keep_the_final_quote_pair_in_cjk_context() {
    let text = "所以这个和 “骑ji” “说shui”“斜xiá”不一样，港台是从众的，大陆读音大多数源自韵书。";
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.line_breaker = Box::new(LookaheadLineBreaker::default());
    engine.hyphenator = &NoHyphenator;
    let result = engine.layout(
        LayoutInput::builder(TiqianTextContent::new(Text::from(text)), LayoutConstraints::with_defaults(160.0))
            .paragraph_style(ParagraphStyle::builder().first_line_indent(Some(Ic::ZERO)).build())
            .build(),
    );
    let final_indices: HashSet<ScalarOffset> = HashSet::from([
        scalar_last_index_of(text, '“'),
        scalar_last_index_of(text, '”'),
    ]);
    let final_overrides: Vec<_> = result.debug.role_overrides.iter().filter(|override_info| final_indices.contains(&override_info.range.start())).collect();
    assert_eq!(2, final_overrides.len());
    assert!(final_overrides.iter().all(|override_info| override_info.overridden_role == "CjkPunctuation"));
    assert!(final_overrides.iter().all(|override_info| override_info.source == "PairedPunctuationOuterScriptContext"));
    assert!(result.lines.iter().all(|line| !result.input.content.text.slice_text(line.range).as_str().starts_with('”')));
}

#[test]
fn skips_neutral_dash_before_latin_quote_pair_in_layout() {
    let result = layout("English — “hello”");
    assert_eq!("latin-primary", result.clusters.iter().find(|cluster| cluster.text.as_str().contains("“hello”")).unwrap().font_key);
}

#[test]
fn keeps_slash_led_latin_technical_run_out_of_cjk_punctuation_geometry() {
    let result = layout("恐跨/TERFism。如果");
    let latin_run = result.debug.font_decisions.iter().find(|decision| decision.source_text == "/TERFism").unwrap();
    assert_eq!("LatinText", latin_run.role);
    assert!(result.debug.punctuation_decisions.iter().all(|decision| decision.range != latin_run.range));
    let cluster = result.clusters.iter().find(|cluster| cluster.text == "/TERFism").unwrap();
    assert_eq!("latin-primary", cluster.font_key);
    assert!(cluster.advance > 16.0);
}

#[test]
fn records_role_overrides_for_resolved_quote_pairs() {
    let result = layout("“Hello” world");
    let opening = result.debug.role_overrides.iter().find(|override_info| override_info.range.start().value() == 0).unwrap();
    let closing = result.debug.role_overrides.iter().find(|override_info| override_info.range.start().value() == 6).unwrap();
    assert_eq!("LatinText", opening.overridden_role);
    assert_eq!("CjkPunctuation", opening.original_role);
    assert_eq!("PairedPunctuationOuterScriptContext", opening.source);
    assert_eq!("LatinText", closing.overridden_role);
}

#[test]
fn mixed_chinese_question_at_paragraph_start_keeps_cjk_quote_geometry() {
    let text = "“Json是谁？”";
    let result = layout(text);
    let quote_indices: HashSet<ScalarOffset> = HashSet::from([scalar_offset(0), scalar_offset(text.chars().count() as i32 - 1)]);
    let overrides: Vec<_> = result.debug.role_overrides.iter().filter(|override_info| quote_indices.contains(&override_info.range.start())).collect();
    assert_eq!(quote_indices, overrides.iter().map(|override_info| override_info.range.start()).collect());
    assert!(overrides.iter().all(|override_info| override_info.overridden_role == "CjkPunctuation"));
    assert!(overrides.iter().all(|override_info| override_info.source == "ParagraphLanguageQuoteContext"));
    assert_eq!(quote_indices, result.debug.punctuation_decisions.iter().filter(|decision| matches!(decision.ch, '“' | '”')).map(|decision| decision.range.start()).collect());
    assert_eq!(text, result.clusters.iter().map(|cluster| cluster.text.as_str()).collect::<String>());
}

#[test]
fn keeps_numbered_cjk_quote_pair_on_cjk_face() {
    let result = layout("1.“你知道李白是怎么死的吗？”");
    let opening = result.debug.font_decisions.iter().find(|decision| decision.range.start().value() == 2).unwrap();
    assert_eq!("CjkPunctuation", opening.role);
    assert_eq!("cjk-primary", opening.font_key);
    let override_info = result.debug.role_overrides.iter().find(|override_info| override_info.range.start().value() == 2).unwrap();
    assert_eq!("PairedPunctuationContentScriptContext", override_info.source);
    assert_eq!("quoted-content-script", override_info.reason);
    assert_eq!("CjkPunctuation", override_info.overridden_role);
}

struct ProportionalQuoteTextShaper;

impl TextShaper for ProportionalQuoteTextShaper {
    fn shape(&self, input: &ShapingInput) -> ShapingResult {
        let result = ExplainableStubTextShaper.shape(input);
        if !matches!(input.display_text.as_str(), "“" | "”") {
            return result;
        }
        assert_eq!(vec!["fwid=1"], input.open_type_features);
        let advance = 6.0;
        ShapingResult::with_decisions(
            result.clusters.into_iter().map(|cluster| Cluster { advance, ..cluster }).collect(),
            result.glyph_runs.into_iter().map(|run| GlyphRun::new(
                run.range,
                run.font_key,
                run.glyphs.into_iter().map(|glyph| Glyph::builder(glyph.id, glyph.cluster_range, advance).bounds(Some(Rect { left: 1.0, top: -10.0, right: 5.0, bottom: 0.0 })).build()).collect(),
                advance,
            )).collect(),
            result.decisions.into_iter().map(|decision| ShapingDecisionInfo { advance, ..decision }).collect(),
        )
    }
}

#[test]
fn requests_full_width_cjk_quotes_and_synthesizes_the_cell_when_the_font_stays_proportional() {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.text_shaper = Box::new(ProportionalQuoteTextShaper);
    let input = |text| LayoutInput::builder(TiqianTextContent::new(Text::from(text)), LayoutConstraints::with_defaults(320.0))
        .paragraph_style(ParagraphStyle::builder().first_line_indent(Some(Ic::ZERO)).build())
        .build();
    let result = engine.layout(input("中“文”中"));
    let opening = result.clusters.iter().find(|cluster| cluster.text == "“").unwrap();
    let closing = result.clusters.iter().find(|cluster| cluster.text == "”").unwrap();
    assert_eq!(16.0, opening.advance);
    assert_eq!(16.0, closing.advance);
    assert_eq!(10.0, opening.glyph_inline_shift);
    assert_eq!(0.0, closing.glyph_inline_shift);
    let opening_decision = result.debug.punctuation_decisions.iter().find(|decision| decision.ch == '“').unwrap();
    let closing_decision = result.debug.punctuation_decisions.iter().find(|decision| decision.ch == '”').unwrap();
    assert_eq!(10.0, opening_decision.advance_expansion);
    assert_eq!(Some("UnderwidthPunctuationFullWidthBoxPlacement".to_owned()), opening_decision.glyph_placement_reason);
    assert_eq!(None, closing_decision.glyph_placement_reason);
    assert_eq!("InkBoundsFittedBodyCompression", opening_decision.geometry_source);
    assert_eq!("InkBoundsFittedBodyCompression", closing_decision.geometry_source);
    let positioned = positioned_clusters(&result);
    let positioned_opening = positioned.iter().find(|cluster| cluster.range == opening.range).unwrap();
    let positioned_closing = positioned.iter().find(|cluster| cluster.range == closing.range).unwrap();
    assert_eq!(positioned_opening.left + 10.0, positioned_opening.draw_x);
    assert_eq!(positioned_closing.left, positioned_closing.draw_x);
    let line_start = engine.layout(input("“文"));
    assert_eq!(8.0, line_start.clusters[0].advance);
    let positioned_start = positioned_clusters(&line_start);
    assert_eq!(2.0, positioned_start[0].draw_x);
    assert_eq!(8.0, positioned_start[1].left);
}

#[test]
fn leaves_latin_context_curly_quotes_outside_cjk_punctuation_geometry() {
    let result = layout("“Hello” world");
    assert!(result.debug.punctuation_decisions.iter().all(|decision| !matches!(decision.ch, '“' | '”')));
    assert!(result.clusters.iter().all(|cluster| cluster.glyph_inline_shift == 0.0));
}

#[test]
fn keeps_contraction_apostrophe_latin_inside_cjk_single_quotes() {
    let result = layout("中‘that’s’中");
    let opening = result.debug.font_decisions.iter().find(|decision| decision.range == text_range(1, 2)).unwrap();
    let contraction = result.debug.font_decisions.iter().find(|decision| decision.range == text_range(2, 8)).unwrap();
    let closing = result.debug.font_decisions.iter().find(|decision| decision.range == text_range(8, 9)).unwrap();
    assert_eq!("CjkPunctuation", opening.role);
    assert_eq!("LatinText", contraction.role);
    assert_eq!("that’s", contraction.source_text);
    assert_eq!("latin-primary", contraction.font_key);
    assert_eq!("CjkPunctuation", closing.role);
    assert_eq!("latin-primary", result.clusters.iter().find(|cluster| cluster.text == "that’s").unwrap().font_key);
    assert!(result.debug.punctuation_decisions.iter().all(|decision| decision.range != text_range(6, 7)));
}

#[test]
fn keeps_latin_word_internal_curly_quotes_in_latin_run_inside_mixed_paragraph() {
    let result = layout("中文 Latin: le“t”ters 中文");
    assert_eq!("latin-primary", result.clusters.iter().find(|cluster| cluster.text == "le“t”ters").unwrap().font_key);
    let overrides: Vec<_> = result.debug.role_overrides.iter().filter(|override_info| matches!(override_info.source_text.as_str(), "“" | "”")).collect();
    assert_eq!(2, overrides.len());
    assert!(overrides.iter().all(|override_info| override_info.overridden_role == "LatinText"));
}

#[test]
fn supports_supplementary_letters_inside_latin_word_internal_quotes() {
    let result = layout("中文 a“𝐀”b 中文");
    let overrides: Vec<_> = result.debug.role_overrides.iter().filter(|override_info| matches!(override_info.source_text.as_str(), "“" | "”")).collect();
    assert_eq!(2, overrides.len());
    assert!(overrides.iter().all(|override_info| override_info.overridden_role == "LatinText" && override_info.source == "NonCjkWordInternalQuotePair"));
}

#[test]
fn keeps_letter_bounded_word_internal_quotes_latin() {
    let result = layout("中a“b”c文");
    let overrides: Vec<_> = result.debug.role_overrides.iter().filter(|override_info| matches!(override_info.source_text.as_str(), "“" | "”")).collect();
    assert_eq!(2, overrides.len());
    assert!(overrides.iter().all(|override_info| override_info.overridden_role == "LatinText" && override_info.source == "NonCjkWordInternalQuotePair"));
}

#[test]
fn keeps_digit_content_inside_letter_bounded_quotes_latin() {
    let result = layout("中a“1”c文");
    let overrides: Vec<_> = result.debug.role_overrides.iter().filter(|override_info| matches!(override_info.source_text.as_str(), "“" | "”")).collect();
    assert_eq!(2, overrides.len());
    assert!(overrides.iter().all(|override_info| override_info.overridden_role == "LatinText" && override_info.source == "NonCjkWordInternalQuotePair"));
}

#[test]
fn keeps_digit_bounded_word_internal_quotes_cjk() {
    let result = layout("中1“1”2文");
    let overrides: Vec<_> = result.debug.role_overrides.iter().filter(|override_info| matches!(override_info.source_text.as_str(), "“" | "”")).collect();
    assert_eq!(2, overrides.len());
    assert!(overrides.iter().all(|override_info| override_info.overridden_role == "CjkPunctuation" && override_info.source == "PairedPunctuationOuterScriptContext"));
}

#[test]
fn keeps_fullwidth_letter_bounded_word_internal_quotes_cjk() {
    let result = layout("中Ａ“Ｂ”Ｃ文");
    let overrides: Vec<_> = result.debug.role_overrides.iter().filter(|override_info| matches!(override_info.source_text.as_str(), "“" | "”")).collect();
    assert_eq!(2, overrides.len());
    assert!(overrides.iter().all(|override_info| override_info.overridden_role == "CjkPunctuation" && override_info.source == "ParagraphLanguageQuoteContext"));
}

#[test]
fn keeps_empty_word_internal_quotes_latin() {
    let result = layout("中文a“”b中文");
    let overrides: Vec<_> = result.debug.role_overrides.iter().filter(|override_info| matches!(override_info.source_text.as_str(), "“" | "”")).collect();
    assert_eq!(2, overrides.len());
    assert!(overrides.iter().all(|override_info| override_info.overridden_role == "LatinText" && override_info.source == "NonCjkWordInternalQuotePair"));
}

#[test]
fn keeps_astral_letter_bounded_word_internal_quotes_latin() {
    let result = layout("中𝐀“b”𝐁文");
    let overrides: Vec<_> = result.debug.role_overrides.iter().filter(|override_info| matches!(override_info.source_text.as_str(), "“" | "”")).collect();
    assert_eq!(2, overrides.len());
    assert!(overrides.iter().all(|override_info| override_info.overridden_role == "LatinText" && override_info.source == "NonCjkWordInternalQuotePair"));
}

#[test]
fn keeps_space_inside_pair_out_of_word_internal_fast_path_latin() {
    let result = layout("中a“b c”d文");
    let overrides: Vec<_> = result.debug.role_overrides.iter().filter(|override_info| matches!(override_info.source_text.as_str(), "“" | "”")).collect();
    assert_eq!(2, overrides.len());
    assert!(overrides.iter().all(|override_info| override_info.overridden_role == "CjkPunctuation" && override_info.source == "ParagraphLanguageQuoteContext"));
}

#[test]
fn keeps_digit_bounded_single_quote_pair_cjk_via_enclosing_quotation() {
    let result = layout("尾号是“1‘2’3”。");
    let singles: Vec<_> = result.debug.role_overrides.iter().filter(|override_info| matches!(override_info.source_text.as_str(), "‘" | "’")).collect();
    assert_eq!(2, singles.len());
    assert!(singles.iter().all(|override_info| override_info.overridden_role == "CjkPunctuation" && override_info.source == "PairedPunctuationEnclosingQuoteContext"), "{singles:?}");
    let doubles: Vec<_> = result.debug.role_overrides.iter().filter(|override_info| matches!(override_info.source_text.as_str(), "“" | "”")).collect();
    assert!(doubles.iter().all(|override_info| override_info.overridden_role == "CjkPunctuation"));
}

#[test]
fn resolves_digit_bound_unmatched_quotes_as_primes() {
    let result = layout("他用时1’30”，屏幕是6.1”的。");
    let marks: Vec<_> = result.debug.role_overrides.iter().filter(|override_info| matches!(override_info.source_text.as_str(), "’" | "”")).collect();
    assert_eq!(3, marks.len());
    assert!(marks.iter().all(|override_info| override_info.overridden_role == "LatinText" && override_info.source == "NumericPrimeUnmatchedQuote"), "{marks:?}");
}

#[test]
fn keeps_decade_style_apostrophe_with_letter_flank_latin() {
    let result = layout("那是90’s的音乐。");
    let apostrophe = result.debug.role_overrides.iter().find(|override_info| override_info.source_text == "’").unwrap();
    assert_eq!("LatinText", apostrophe.overridden_role);
    assert_eq!("NonCjkInWordApostrophe", apostrophe.source);
}
