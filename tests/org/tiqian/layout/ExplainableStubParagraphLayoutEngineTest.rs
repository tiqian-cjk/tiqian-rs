use tiqian::common::HashSet;

use tiqian::core::Geometry::{LayoutConstraints, Rect, TextRange};
use tiqian::core::LayoutModel::{Cluster, Glyph, GlyphRun, LineEndReason};
use tiqian::core::Text::Text;
use tiqian::core::TextModel::{
    LayoutInput, ParagraphStyle, TextSpan, TextStyle, TiqianTextContent,
};
use tiqian::core::Units::Ic;
use tiqian::layout::LineBreaker::LookaheadLineBreaker;
use tiqian::layout::ParagraphLayoutEngine::{
    ExplainableStubParagraphLayoutEngine, ParagraphLayoutEngine,
};
use tiqian::shaping::TextShaper::{ShapingInput, ShapingResult, TextShaper};

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
fn paragraph_entry_returns_single_line_and_records_selected_breaker() {
    let mut greedy = ExplainableStubParagraphLayoutEngine::default();
    let result = greedy.layout(input("提椠"));
    assert_eq!(2, result.clusters.len());
    assert_eq!(1, result.lines.len());
    assert_eq!("greedy", result.debug.line_decisions[0].kind);

    let mut lookahead = ExplainableStubParagraphLayoutEngine::default();
    lookahead.line_breaker = Box::new(LookaheadLineBreaker::default());
    let result = lookahead.layout(input("提椠"));
    assert_eq!("lookahead", result.debug.line_decisions[0].kind);
}

#[test]
fn mandatory_break_controls_are_unshaped_and_preserve_crlf_and_trailing_blank_line() {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    let result = engine.layout(input("甲\r\n乙\n"));

    assert_eq!(3, result.lines.len());
    assert_eq!(LineEndReason::MandatoryBreak, result.lines[0].end_reason);
    assert_eq!(LineEndReason::MandatoryBreak, result.lines[1].end_reason);
    assert_eq!(LineEndReason::ParagraphEnd, result.lines[2].end_reason);
    let crlf = result
        .clusters
        .iter()
        .find(|cluster| cluster.text == "\r\n")
        .unwrap();
    assert_eq!(TextRange::new(1, 3), crlf.range);
    assert_eq!("", crlf.display_text);
    assert_eq!(0.0, crlf.advance);
    assert!(
        result
            .glyph_runs
            .iter()
            .flat_map(|run| &run.glyphs)
            .all(|glyph| glyph.cluster_range != crlf.range)
    );
    assert_eq!(TextRange::new(5, 5), result.lines[2].range);
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
fn paragraph_entry_preserves_shaper_glyph_bounds() {
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
fn paragraph_entry_records_fallback_and_substituted_shaping_ranges() {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
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
}

#[test]
fn combining_marks_remain_in_their_base_shaping_runs() {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
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
fn complex_emoji_remains_atomic_until_style_boundary_requires_a_split() {
    let text = "👩🏽‍💻";
    let length = text.encode_utf16().count() as i32;
    let atomic_content = TiqianTextContent::builder(Text::from(text))
        .source_boundaries(HashSet::from([2]))
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
        vec![TextRange::new(0, length)],
        atomic
            .debug
            .shaping_decisions
            .iter()
            .map(|decision| decision.range)
            .collect::<Vec<_>>()
    );

    let styled_content = TiqianTextContent::builder(Text::from(text))
        .spans(vec![TextSpan {
            range: TextRange::new(2, length),
            style: TextStyle::builder().font_weight(700).build(),
        }])
        .source_boundaries(HashSet::from([2]))
        .build();
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
        vec![TextRange::new(0, 2), TextRange::new(2, length)],
        styled
            .debug
            .shaping_decisions
            .iter()
            .map(|decision| decision.range)
            .collect::<Vec<_>>()
    );
}

#[test]
fn emoji_sequence_role_promotions_are_explainable() {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    let result = engine.layout(input("❤️与1️⃣"));

    assert_eq!(
        vec![
            (
                TextRange::new(0, 2),
                "Symbol",
                "Emoji",
                "EmojiStyleVariationSequence"
            ),
            (TextRange::new(3, 6), "LatinText", "Emoji", "KeycapSequence"),
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
