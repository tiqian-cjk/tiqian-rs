use tiqian::clreq::clreq_profile::{ClreqProfile, ClreqProfileResolver, KinsokuLevel, KinsokuMode};
use tiqian::core::geometry::LayoutConstraints;
use tiqian::core::text::Text;
use tiqian::core::text_model::{
    LayoutInput, LineLengthGrid, ParagraphStyle, TiqianTextContent,
};
use tiqian::core::units::Ic;
use tiqian::layout::line_breaker::{GreedyLineBreaker, LineBreaker, LookaheadLineBreaker};
use tiqian::layout::paragraph_layout_engine::{
    ExplainableStubParagraphLayoutEngine, ParagraphLayoutEngine,
};
use tiqian::linebreak::hyphenation::NoHyphenator;

struct FixedKinsokuProfile(KinsokuLevel);

impl ClreqProfileResolver for FixedKinsokuProfile {
    fn resolve(&self, _: &tiqian::core::text_model::LayoutProfileId) -> ClreqProfile {
        let mut profile = ClreqProfile::mainland_horizontal();
        profile.kinsoku_mode = KinsokuMode::fixed(self.0);
        profile
    }
}

fn layout(
    text: &str,
    max_width: f32,
    breaker: Box<dyn LineBreaker>,
    level: KinsokuLevel,
) -> tiqian::core::layout_model::LayoutResult {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.line_breaker = breaker;
    engine.hyphenator = &NoHyphenator;
    engine.clreq_profile_resolver = Box::new(FixedKinsokuProfile(level));
    let indexed = Text::from(text);
    engine.layout(
        LayoutInput::builder(
            TiqianTextContent::builder(indexed.clone())
                .source_boundaries((0..=indexed.utf16_len()).collect())
                .build(),
            LayoutConstraints::with_defaults(max_width),
        )
        .paragraph_style(
            ParagraphStyle::builder()
                .first_line_indent(Some(Ic::ZERO))
                .line_length_grid(LineLengthGrid::with_enabled(false))
                .build(),
        )
        .build(),
    )
}

fn line_texts<'a>(
    result: &tiqian::core::layout_model::LayoutResult,
    text: &'a str,
) -> Vec<&'a str> {
    let indexed = Text::from(text);
    result
        .lines
        .iter()
        .map(|line| {
            let start = indexed.utf8_byte_index_at(line.range.start()).unwrap();
            let end = indexed.utf8_byte_index_at(line.range.end()).unwrap();
            &text[start..end]
        })
        .collect()
}

#[test]
fn bracket_boundaries_remain_protected_across_western_spaces() {
    for (label, make_breaker) in [
        ("greedy", (|| Box::new(GreedyLineBreaker::default()) as Box<dyn LineBreaker>) as fn() -> Box<dyn LineBreaker>),
        ("lookahead", (|| Box::new(LookaheadLineBreaker::default()) as Box<dyn LineBreaker>) as fn() -> Box<dyn LineBreaker>),
    ] {
        for width in (48..=80).step_by(4) {
            let opening_text = "ABCD(  EFGH";
            let opening = layout(opening_text, width as f32, make_breaker(), KinsokuLevel::Basic);
            let opening_lines = line_texts(&opening, opening_text);
            assert!(
                opening_lines
                    .iter()
                    .all(|line| !line.trim_end().ends_with('(')),
                "{label} width={width} opening_lines={opening_lines:?}"
            );

            let closing_text = "ABCD  )EFGH";
            let closing = layout(closing_text, width as f32, make_breaker(), KinsokuLevel::Basic);
            let closing_lines = line_texts(&closing, closing_text);
            assert!(
                closing_lines
                    .iter()
                    .all(|line| !line.trim_start().starts_with(')')),
                "{label} width={width} closing_lines={closing_lines:?}"
            );
        }
    }
}

#[test]
fn unmatched_western_curly_double_quotes_retain_their_direction() {
    for (label, result) in [
        ("greedy", layout("ABCD”E", 32.0, Box::new(GreedyLineBreaker::default()), KinsokuLevel::Basic)),
        ("lookahead", layout("ABCD”E", 32.0, Box::new(LookaheadLineBreaker::default()), KinsokuLevel::Basic)),
    ] {
        assert!(
            line_texts(&result, "ABCD”E")
                .iter()
                .all(|line| !line.starts_with('”')),
            "{label} closing"
        );
        let decision = result
            .debug
            .contextual_kinsoku_decisions
            .iter()
            .find(|decision| decision.source_text == "”")
            .unwrap();
        assert_eq!("LineStart", decision.forbidden_position);
        assert_eq!("Uax14WesternPunctuationBoundary:LB19", decision.reason);
    }

    for (label, result) in [
        ("greedy", layout("ABCD“E", 40.0, Box::new(GreedyLineBreaker::default()), KinsokuLevel::Basic)),
        ("lookahead", layout("ABCD“E", 40.0, Box::new(LookaheadLineBreaker::default()), KinsokuLevel::Basic)),
    ] {
        assert!(
            line_texts(&result, "ABCD“E")
                .iter()
                .all(|line| !line.ends_with('“')),
            "{label} opening"
        );
        let decision = result
            .debug
            .contextual_kinsoku_decisions
            .iter()
            .find(|decision| decision.source_text == "“")
            .unwrap();
        assert_eq!("LineEnd", decision.forbidden_position);
        assert_eq!("Uax14WesternPunctuationBoundary:LB19", decision.reason);
    }
}

#[test]
fn unmatched_elision_apostrophe_binds_forward_instead_of_being_guessed_as_a_closer() {
    let text = "AB ’90s";
    let result = layout(text, 16.0, Box::new(GreedyLineBreaker::default()), KinsokuLevel::Basic);
    assert!(
        result
            .debug
            .contextual_kinsoku_decisions
            .iter()
            .all(|decision| !(decision.source_text == "’" && decision.forbidden_position == "LineStart"))
    );
    let decision = result
        .debug
        .contextual_kinsoku_decisions
        .iter()
        .find(|decision| decision.source_text == "’" && decision.forbidden_position == "LineEnd")
        .unwrap();
    assert_eq!("Uax14WesternPunctuationBoundary:LB19", decision.reason);
}

#[test]
fn western_baseline_survives_clreq_kinsoku_none() {
    let result = layout("ABCD)E", 32.0, Box::new(GreedyLineBreaker::default()), KinsokuLevel::None);
    assert_eq!(
        "Uax14WesternPunctuationBoundary:LB13",
        result
            .debug
            .contextual_kinsoku_decisions
            .iter()
            .find(|decision| decision.source_text == ")")
            .unwrap()
            .reason,
    );
}
