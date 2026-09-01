use tiqian::core::geometry::{LayoutConstraints, TextRange};
use tiqian::core::layout_model::BopomofoGlyphRole;
use tiqian::core::text::Text;
use tiqian::core::text_model::{
    LayoutInput, ParagraphStyle, RubyKind, RubySpan, TextSpan, TextStyle, TiqianTextContent,
};
use tiqian::core::units::Ic;
use tiqian::layout::paragraph_layout_engine::{
    ExplainableStubParagraphLayoutEngine, ParagraphLayoutEngine,
};

fn layout(
    ruby_spans: Vec<RubySpan>,
    spans: Vec<TextSpan>,
) -> tiqian::core::layout_model::LayoutResult {
    ExplainableStubParagraphLayoutEngine::default().layout(
        LayoutInput::builder(
            TiqianTextContent::builder(Text::from("中文"))
                .spans(spans)
                .build(),
            LayoutConstraints::with_defaults(4000.0),
        )
        .paragraph_style(
            ParagraphStyle::builder()
                .first_line_indent(Some(Ic::ZERO))
                .build(),
        )
        .ruby_spans(ruby_spans)
        .build(),
    )
}

#[test]
fn bopomofo_symbols_and_tones_occupy_right_side_annotation_zone() {
    let result = layout(
        vec![
            RubySpan::with_kind(
                TextRange::new(0, 1),
                Text::from("ㄓㄨㄥ"),
                RubyKind::Bopomofo,
            ),
            RubySpan::with_kind(
                TextRange::new(1, 2),
                Text::from("ㄔㄤˊ"),
                RubyKind::Bopomofo,
            ),
        ],
        Vec::new(),
    );
    let zhong = result
        .debug
        .bopomofo_decisions
        .iter()
        .find(|decision| decision.base_range.start() == 0)
        .unwrap();
    let chang = result
        .debug
        .bopomofo_decisions
        .iter()
        .find(|decision| decision.base_range.start() == 1)
        .unwrap();

    assert_eq!(700, zhong.font_weight);
    assert_eq!(
        3,
        zhong
            .placements
            .iter()
            .filter(|placement| placement.role == BopomofoGlyphRole::Symbol)
            .count()
    );
    assert!(
        zhong
            .placements
            .iter()
            .all(|placement| placement.left >= 15.9)
    );
    assert_eq!(
        2,
        chang
            .placements
            .iter()
            .filter(|placement| placement.role == BopomofoGlyphRole::Symbol)
            .count()
    );
    assert_eq!(
        1,
        chang
            .placements
            .iter()
            .filter(|placement| placement.role == BopomofoGlyphRole::Tone)
            .count()
    );
}

#[test]
fn bopomofo_reserves_annotated_base_without_changing_unannotated_neighbor() {
    let plain = layout(Vec::new(), Vec::new());
    let annotated = layout(
        vec![RubySpan::with_kind(
            TextRange::new(0, 1),
            Text::from("ㄓㄨㄥ"),
            RubyKind::Bopomofo,
        )],
        Vec::new(),
    );

    assert!(annotated.clusters[0].advance > plain.clusters[0].advance);
    assert_eq!(plain.clusters[1].advance, annotated.clusters[1].advance);
}

#[test]
fn bopomofo_font_weight_follows_annotated_base_plus_three_steps() {
    let weighted = layout(
        vec![
            RubySpan::with_kind(
                TextRange::new(0, 1),
                Text::from("ㄓㄨㄥ"),
                RubyKind::Bopomofo,
            ),
            RubySpan::with_kind(
                TextRange::new(1, 2),
                Text::from("ㄨㄣˊ"),
                RubyKind::Bopomofo,
            ),
        ],
        vec![
            TextSpan {
                range: TextRange::new(0, 1),
                style: TextStyle::builder().font_weight(500).build(),
            },
            TextSpan {
                range: TextRange::new(1, 2),
                style: TextStyle::builder().font_weight(700).build(),
            },
        ],
    );
    assert_eq!(
        800,
        weighted
            .debug
            .bopomofo_decisions
            .iter()
            .find(|decision| decision.base_range == TextRange::new(0, 1))
            .unwrap()
            .font_weight
    );
    assert_eq!(
        900,
        weighted
            .debug
            .bopomofo_decisions
            .iter()
            .find(|decision| decision.base_range == TextRange::new(1, 2))
            .unwrap()
            .font_weight
    );
}

#[test]
fn bopomofo_decision_keeps_source_reading_for_copy() {
    let neutral = layout(
        vec![RubySpan::with_kind(
            TextRange::new(0, 1),
            Text::from("˙ㄉㄜ"),
            RubyKind::Bopomofo,
        )],
        Vec::new(),
    );
    let decision = &neutral.debug.bopomofo_decisions[0];
    assert_eq!("˙ㄉㄜ", decision.text);
    assert_eq!(
        vec!["˙", "ㄉ", "ㄜ"],
        decision
            .placements
            .iter()
            .map(|placement| placement.text.as_str())
            .collect::<Vec<_>>()
    );
    assert_eq!(BopomofoGlyphRole::Neutral, decision.placements[0].role);
}

#[test]
fn bopomofo_annotation_locale_does_not_replace_simplified_base_locale() {
    let neutral = layout(
        vec![RubySpan::with_kind(
            TextRange::new(0, 1),
            Text::from("˙ㄉㄜ"),
            RubyKind::Bopomofo,
        )],
        Vec::new(),
    );
    let decision = &neutral.debug.bopomofo_decisions[0];
    assert_eq!("zh-Hans", neutral.input.text_style.locale);
    assert_eq!("zh-TW", decision.locale);
}
