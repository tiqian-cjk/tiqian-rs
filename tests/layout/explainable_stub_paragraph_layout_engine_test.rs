use tiqian::common::HashSet;

use tiqian::core::geometry::{scalar_offset, text_range, LayoutConstraints, Rect};
use tiqian::core::layout_model::{Cluster, Glyph, GlyphRun, LineEndReason};
use tiqian::core::source_interaction_boundaries::source_grapheme_boundaries;
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
use tiqian::shaping::text_shaper::{ShapingInput, ShapingResult, TextShaper};

fn input(text: &str) -> LayoutInput {
    LayoutInput::builder(
        TiqianTextContent::new(Text::from(text)),
        LayoutConstraints::with_defaults(240.0),
    )
    .paragraph_style(
        ParagraphStyle::builder()
            .first_line_indent(Some(Ic::ZERO))
            .build(),
    )
    .build()
}

#[test]
fn returns_debuggable_single_line_result() {
    let mut greedy = ExplainableStubParagraphLayoutEngine::default();
    let result = greedy.layout(input("提椠"));
    assert_eq!(2, result.clusters.len());
    assert_eq!(1, result.lines.len());
    assert_eq!("greedy", result.debug.line_decisions[0].kind);
}

#[test]
fn records_injected_line_breaker_strategy_in_debug_decisions() {
    let mut lookahead = ExplainableStubParagraphLayoutEngine::default();
    lookahead.line_breaker = Box::new(LookaheadLineBreaker::default());
    let result = lookahead.layout(input("提椠"));
    assert_eq!("lookahead", result.debug.line_decisions[0].kind);
}

#[test]
fn mandatory_line_break_clusters_are_zero_width_and_not_shaped() {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.line_breaker = Box::new(LookaheadLineBreaker::default());
    let result = engine.layout(input("第一行\n第二行"));

    assert_eq!(2, result.lines.len());
    assert_eq!(LineEndReason::MandatoryBreak, result.lines[0].end_reason);
    assert_eq!(LineEndReason::ParagraphEnd, result.lines[1].end_reason);
    let break_cluster = result
        .clusters
        .iter()
        .find(|cluster| cluster.text == "\n")
        .unwrap();
    assert_eq!("", break_cluster.display_text);
    assert_eq!(0.0, break_cluster.advance);
    assert!(
        result
            .glyph_runs
            .iter()
            .flat_map(|run| &run.glyphs)
            .all(|glyph| glyph.cluster_range != break_cluster.range)
    );
    assert_eq!(
        vec![text_range(0, 3), text_range(4, 7)],
        result.glyph_runs.iter().map(|run| run.range).collect::<Vec<_>>(),
    );
    assert_eq!(
        break_cluster.range,
        result.debug.mandatory_break_decisions[0].range
    );
}

#[test]
fn consecutive_mandatory_line_breaks_create_one_empty_line_box() {
    let result = ExplainableStubParagraphLayoutEngine::default().layout(input("第一行\n\n第二行"));

    assert_eq!(3, result.lines.len());
    assert_eq!(LineEndReason::MandatoryBreak, result.lines[0].end_reason);
    assert_eq!(LineEndReason::MandatoryBreak, result.lines[1].end_reason);
    assert_eq!(LineEndReason::ParagraphEnd, result.lines[2].end_reason);
    let empty_line_cluster = &result.clusters[result.lines[1].cluster_range.first() as usize];
    assert_eq!("\n", empty_line_cluster.text);
    assert_eq!("", empty_line_cluster.display_text);
    assert_eq!(0.0, empty_line_cluster.advance);
    let line_height = result.debug.line_spacing_decision.as_ref().unwrap().resolved_height;
    assert!((result.lines[1].bottom - result.lines[1].top - line_height).abs() < 0.001);
    assert!((result.lines[1].baseline - result.lines[0].baseline - line_height).abs() < 0.001);
    assert!((result.lines[2].baseline - result.lines[1].baseline - line_height).abs() < 0.001);
}

#[test]
fn single_mandatory_break_after_wrapped_line_does_not_create_empty_line() {
    let text = "很久以前，曾经有一个名叫小红帽的孩子，生活在大森林的边上，大森林里充满了濒临灭绝的猫头鹰和珍稀植物，如果有人愿意花时间研究它们，就会发现癌症的治疗方法。\n小红帽和一位称为母亲的养育者一起生活";
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.line_breaker = Box::new(LookaheadLineBreaker::default());
    let result = engine.layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from(text)),
            LayoutConstraints::with_defaults(1200.0),
        )
        .text_style(TextStyle::builder().font_size(48.0).build())
        .paragraph_style(
            ParagraphStyle::builder()
                .first_line_indent(Some(Ic::ZERO))
                .build(),
        )
        .build(),
    );

    assert!(result.lines.len() >= 4, "{:?}", result.lines);
    assert!(result
        .lines
        .iter()
        .all(|line| Text::from(text).slice_text(line.range) != "\n"));
    let mandatory_end = scalar_offset(text[..text.find('\n').unwrap()].chars().count() as i32 + 1);
    assert_eq!(
        LineEndReason::MandatoryBreak,
        result
            .lines
            .iter()
            .find(|line| line.range.end() == mandatory_end)
            .unwrap()
            .end_reason
    );
    let line_height = result.debug.line_spacing_decision.as_ref().unwrap().resolved_height;
    for lines in result.lines.windows(2) {
        assert!((lines[1].baseline - lines[0].baseline - line_height).abs() < 0.001);
    }
}

#[test]
fn crlf_is_one_mandatory_break_cluster() {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.line_breaker = Box::new(LookaheadLineBreaker::default());
    let result = engine.layout(input("甲\r\n乙"));

    assert_eq!(2, result.lines.len());
    let break_cluster = result
        .clusters
        .iter()
        .find(|cluster| cluster.text == "\r\n")
        .unwrap();
    assert_eq!(1, result.debug.mandatory_break_decisions.len());
    assert_eq!(scalar_offset(1), break_cluster.range.start());
    assert_eq!(scalar_offset(3), break_cluster.range.end());
}

#[test]
fn consecutive_and_trailing_mandatory_breaks_preserve_blank_lines() {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.line_breaker = Box::new(LookaheadLineBreaker::default());
    let result = engine.layout(input("甲\n\n乙\n"));

    assert_eq!(4, result.lines.len());
    assert_eq!(LineEndReason::MandatoryBreak, result.lines[0].end_reason);
    assert_eq!(LineEndReason::MandatoryBreak, result.lines[1].end_reason);
    assert_eq!(LineEndReason::MandatoryBreak, result.lines[2].end_reason);
    assert_eq!(LineEndReason::ParagraphEnd, result.lines[3].end_reason);
    assert_eq!(0.0, result.lines[1].visual_width);
    assert_eq!(text_range(5, 5), result.lines[3].range);
}

#[test]
fn mandatory_break_line_is_not_justified() {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.line_breaker = Box::new(LookaheadLineBreaker::default());
    let result = engine.layout(input("短\n中文中文中文中文中文"));

    let mandatory_line = &result.lines[0];
    assert_eq!(LineEndReason::MandatoryBreak, mandatory_line.end_reason);
    assert_eq!(mandatory_line.natural_width, mandatory_line.adjusted_width);
    assert!(result
        .debug
        .justification_decisions
        .iter()
        .all(|decision| decision.line_range != mandatory_line.range));
}

struct BoundsTextShaper;

impl TextShaper for BoundsTextShaper {
    fn shape(&self, input: &ShapingInput) -> ShapingResult {
        let bounds = Rect {
            left: 1.0,
            top: -10.0,
            right: 12.0,
            bottom: 2.0,
        };
        ShapingResult::new(
            vec![Cluster::with_display_text(
                input.range,
                input.display_text.clone(),
                input.display_text.clone(),
                input.font_decision.candidate.key.clone(),
                20.0,
            )],
            vec![GlyphRun::new(
                input.range,
                input.font_decision.candidate.key.clone(),
                vec![
                    Glyph::builder(42, input.range, 20.0)
                        .bounds(Some(bounds))
                        .build(),
                ],
                20.0,
            )],
        )
    }
}

#[test]
#[should_panic(expected = "TextShaper must return clusters covering")]
fn rejects_shaper_clusters_that_do_not_cover_font_decision_range() {
    struct EmptyTextShaper;

    impl TextShaper for EmptyTextShaper {
        fn shape(&self, _: &ShapingInput) -> ShapingResult {
            ShapingResult::new(Vec::new(), Vec::new())
        }
    }

    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.text_shaper = Box::new(EmptyTextShaper);
    engine.layout(input("提椠"));
}

#[test]
fn preserves_shaper_glyph_bounds_in_layout_glyph_runs() {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.text_shaper = Box::new(BoundsTextShaper);
    let result = engine.layout(input("A"));

    let glyph = &result.glyph_runs[0].glyphs[0];
    assert_eq!(42, glyph.id);
    assert_eq!(20.0, glyph.advance);
    assert_eq!(
        Some(Rect {
            left: 1.0,
            top: -10.0,
            right: 12.0,
            bottom: 2.0
        }),
        glyph.bounds
    );
}

#[test]
fn records_fallback_decisions_per_cluster() {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.hyphenator = &NoHyphenator;
    let result = engine.layout(input("提椠……English——世界。"));

    assert!(result.debug.font_decisions.iter().any(|decision| {
        decision.source_text == "……"
            && decision.display_text == "⋯⋯"
            && decision.role == "CjkPunctuation"
            && decision.font_key == "cjk-primary"
    }));
    assert!(result.debug.font_decisions.iter().any(|decision| {
        decision.source_text == "English"
            && decision.role == "LatinText"
            && decision.font_key == "latin-primary"
    }));
    assert!(result.debug.shaping_decisions.iter().any(|decision| {
        decision.source_text == "——"
            && decision.display_text == "⸺"
            && decision.advance == 32.0
            && decision.source == "Stub"
    }));
    assert!(result.debug.font_decisions.iter().any(|decision| {
        decision.source_text == "English"
            && decision.role == "LatinText"
            && decision.font_key == "latin-primary"
    }));
    assert_eq!(
        "English",
        result
            .clusters
            .iter()
            .find(|cluster| cluster.text == "English")
            .unwrap()
            .text
    );
}

#[test]
fn combining_marks_stay_in_their_base_shaping_runs() {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.hyphenator = &NoHyphenator;
    let result = engine.layout(input("༎ຶ Ỏ̷"));

    assert!(
        result
            .debug
            .shaping_decisions
            .iter()
            .any(|decision| decision.source_text == "༎ຶ")
    );
    assert!(
        result
            .debug
            .shaping_decisions
            .iter()
            .any(|decision| decision.source_text == "Ỏ̷")
    );
    assert!(
        result
            .debug
            .shaping_decisions
            .iter()
            .all(|decision| { decision.source_text != "ຶ" && decision.source_text != "̷" })
    );
}

#[test]
fn complex_emoji_graphemes_stay_atomic_across_geometry_only_boundaries() {
    let text = "👩🏽‍💻";
    let length = text.chars().count() as i32;
    let atomic_content = TiqianTextContent::builder(Text::from(text))
        .source_boundaries(HashSet::from([scalar_offset(2)]))
        .build();
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    let atomic = engine.layout(
        LayoutInput::builder(atomic_content, LayoutConstraints::with_defaults(320.0))
            .paragraph_style(
                ParagraphStyle::builder()
                    .first_line_indent(Some(Ic::ZERO))
                    .build(),
            )
            .build(),
    );
    assert_eq!(
        vec![text_range(0, length)],
        atomic
            .debug
            .font_decisions
            .iter()
            .filter(|decision| decision.role == "Emoji")
            .map(|decision| decision.range)
            .collect::<Vec<_>>(),
    );
    assert_eq!(
        vec![text],
        atomic
            .debug
            .shaping_decisions
            .iter()
            .map(|decision| decision.source_text.as_str())
            .collect::<Vec<_>>()
    );
}

#[test]
fn complex_emoji_sequences_reach_the_shaper_as_complete_emoji_ranges() {
    let text = "前👩🏽‍💻后🇨🇳与1️⃣和❤️。";
    let result = ExplainableStubParagraphLayoutEngine::default().layout(input(text));

    assert_eq!(
        vec!["👩🏽‍💻", "🇨🇳", "1️⃣", "❤️"],
        result
            .debug
            .font_decisions
            .iter()
            .filter(|decision| decision.role == "Emoji")
            .map(|decision| decision.source_text.as_str())
            .collect::<Vec<_>>(),
    );
    assert_eq!(
        vec!["👩🏽‍💻", "🇨🇳", "1️⃣", "❤️"],
        result
            .debug
            .shaping_decisions
            .iter()
            .filter(|decision| decision.font_key == "symbol-fallback")
            .map(|decision| decision.source_text.as_str())
            .collect::<Vec<_>>(),
    );
}

#[test]
fn complex_emoji_graphemes_honor_text_span_style_boundaries() {
    let text = "👩🏽‍💻";
    let length = text.chars().count() as i32;
    let styled_content = TiqianTextContent::builder(Text::from(text))
        .spans(vec![TextSpan {
            range: text_range(2, length),
            style: TextStyle::builder().font_weight(700).build(),
        }])
        .source_boundaries(HashSet::from([scalar_offset(2)]))
        .build();
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    let styled = engine.layout(
        LayoutInput::builder(styled_content, LayoutConstraints::with_defaults(320.0))
            .paragraph_style(
                ParagraphStyle::builder()
                    .first_line_indent(Some(Ic::ZERO))
                    .build(),
            )
            .build(),
    );
    assert_eq!(
        vec![text_range(0, 2), text_range(2, 3), text_range(3, length)],
        styled
            .debug
            .shaping_decisions
            .iter()
            .map(|decision| decision.range)
            .collect::<Vec<_>>()
    );
}

#[test]
fn source_grapheme_boundaries_do_not_join_zwj_with_ordinary_text() {
    let left = Text::from("👩‍中");
    assert_eq!(
        vec![scalar_offset(0), scalar_offset(2), scalar_offset(3)],
        source_grapheme_boundaries(&left, text_range(0, left.scalar_len().value()))
    );
    let right = Text::from("中‍👩");
    assert_eq!(
        vec![scalar_offset(0), scalar_offset(2), scalar_offset(3)],
        source_grapheme_boundaries(&right, text_range(0, right.scalar_len().value()))
    );
}

#[test]
fn records_unicode_emoji_sequence_role_promotions() {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    let result = engine.layout(input("❤️与1️⃣"));

    assert_eq!(
        vec![
            (
                text_range(0, 2),
                "Symbol",
                "Emoji",
                "EmojiStyleVariationSequence"
            ),
            (text_range(3, 6), "LatinText", "Emoji", "KeycapSequence"),
        ],
        result
            .debug
            .role_overrides
            .iter()
            .map(|info| (
                info.range,
                info.original_role.as_str(),
                info.overridden_role.as_str(),
                info.reason.as_str(),
            ))
            .collect::<Vec<_>>(),
    );
}

#[test]
fn emoji_role_matrix_separates_supported_sequences_from_adjacent_and_unrelated_text() {
    let tag_flag = "🏴\u{E0067}\u{E0062}\u{E0065}\u{E006E}\u{E0067}\u{E007F}";
    let tag_flag_case = format!("a{tag_flag}中");
    let cases = vec![
        ("a1️⃣", vec![("a", "LatinText"), ("1️⃣", "Emoji")]),
        ("1️⃣a", vec![("1️⃣", "Emoji"), ("a", "LatinText")]),
        (
            "a😀中",
            vec![("a", "LatinText"), ("😀", "Emoji"), ("中", "CjkText")],
        ),
        (
            "a❤️中",
            vec![("a", "LatinText"), ("❤️", "Emoji"), ("中", "CjkText")],
        ),
        (
            "a©️中",
            vec![("a", "LatinText"), ("©️", "Emoji"), ("中", "CjkText")],
        ),
        (
            "a⌚︎中",
            vec![("a", "LatinText"), ("⌚︎", "Emoji"), ("中", "CjkText")],
        ),
        (
            "a1⃣中",
            vec![("a", "LatinText"), ("1⃣", "Emoji"), ("中", "CjkText")],
        ),
        (
            "a👍🏽中",
            vec![("a", "LatinText"), ("👍🏽", "Emoji"), ("中", "CjkText")],
        ),
        (
            "a👩🏽‍💻中",
            vec![("a", "LatinText"), ("👩🏽‍💻", "Emoji"), ("中", "CjkText")],
        ),
        (
            "a🏳️‍⚧️中",
            vec![("a", "LatinText"), ("🏳️‍⚧️", "Emoji"), ("中", "CjkText")],
        ),
        (
            "a🇨🇳中",
            vec![("a", "LatinText"), ("🇨🇳", "Emoji"), ("中", "CjkText")],
        ),
        (
            tag_flag_case.as_str(),
            vec![("a", "LatinText"), (tag_flag, "Emoji"), ("中", "CjkText")],
        ),
        ("中\u{FE0F}", vec![("中\u{FE0F}", "CjkText")]),
        ("a\u{FE0F}", vec![("a\u{FE0F}", "LatinText")]),
        ("a⃣中", vec![("a⃣", "LatinText"), ("中", "CjkText")]),
        (
            "a1\u{FE0F}中",
            vec![("a1\u{FE0F}", "LatinText"), ("中", "CjkText")],
        ),
        ("中🏽", vec![("中", "CjkText"), ("🏽", "Emoji")]),
        (
            "a👩‍中",
            vec![("a", "LatinText"), ("👩‍", "Emoji"), ("中", "CjkText")],
        ),
        (
            "中‍👩a",
            vec![
                ("中", "CjkText"),
                ("‍", "Unknown"),
                ("👩", "Emoji"),
                ("a", "LatinText"),
            ],
        ),
    ];

    for (text, expected) in cases {
        let mut engine = ExplainableStubParagraphLayoutEngine::default();
        let result = engine.layout(input(&text));
        let actual = result
            .debug
            .font_decisions
            .iter()
            .map(|decision| (decision.source_text.as_str(), decision.role.as_str()))
            .collect::<Vec<_>>();

        assert_eq!(expected, actual, "emoji role mismatch for {text:?}");
    }
}
