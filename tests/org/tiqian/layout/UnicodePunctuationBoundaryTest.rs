use tiqian::common::HashSet;

use tiqian::org::tiqian::clreq::ClreqProfile::{
    ClreqProfile, ClreqProfileResolver, KinsokuLevel, KinsokuMode,
};
use tiqian::org::tiqian::core::Geometry::{LayoutConstraints, TextRange};
use tiqian::org::tiqian::core::LayoutModel::Cluster;
use tiqian::org::tiqian::core::Text::Text;
use tiqian::org::tiqian::core::TextModel::{LayoutInput, ParagraphStyle, TiqianTextContent};
use tiqian::org::tiqian::core::Units::Ic;
use tiqian::org::tiqian::font::FontPolicy::FontRole;
use tiqian::org::tiqian::layout::LineBreaker::{GreedyLineBreaker, LookaheadLineBreaker};
use tiqian::org::tiqian::layout::ParagraphLayoutEngine::{
    ExplainableStubParagraphLayoutEngine, ParagraphLayoutEngine,
};
use tiqian::org::tiqian::layout::UnicodePunctuationBoundaryResolver::resolve_western_bracket_cjk_inter_char_boundaries;
use tiqian::org::tiqian::linebreak::Hyphenation::NoHyphenator;

struct KinsokuNoneProfile;

impl ClreqProfileResolver for KinsokuNoneProfile {
    fn resolve(&self, _: &tiqian::org::tiqian::core::TextModel::LayoutProfileId) -> ClreqProfile {
        let mut profile = ClreqProfile::mainland_horizontal();
        profile.kinsoku_mode = KinsokuMode::fixed(KinsokuLevel::None);
        profile
    }
}

fn layout_with_greedy(
    text: &str,
    max_width: f32,
) -> tiqian::org::tiqian::core::LayoutModel::LayoutResult {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.line_breaker = Box::new(GreedyLineBreaker::default());
    engine.hyphenator = &NoHyphenator;
    engine.clreq_profile_resolver = Box::new(KinsokuNoneProfile);
    engine.layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from(text)),
            LayoutConstraints::with_defaults(max_width),
        )
        .paragraph_style(
            ParagraphStyle::builder()
                .first_line_indent(Some(Ic::ZERO))
                .build(),
        )
        .build(),
    )
}

fn layout_with_lookahead(
    text: &str,
    max_width: f32,
) -> tiqian::org::tiqian::core::LayoutModel::LayoutResult {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.line_breaker = Box::new(LookaheadLineBreaker::default());
    engine.hyphenator = &NoHyphenator;
    engine.clreq_profile_resolver = Box::new(KinsokuNoneProfile);
    engine.layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from(text)),
            LayoutConstraints::with_defaults(max_width),
        )
        .paragraph_style(
            ParagraphStyle::builder()
                .first_line_indent(Some(Ic::ZERO))
                .build(),
        )
        .build(),
    )
}

fn line_texts<'a>(
    result: &tiqian::org::tiqian::core::LayoutModel::LayoutResult,
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
fn western_brackets_touching_cjk_expose_all_inter_char_boundaries() {
    let text = Text::from("育(中文)后");
    let clusters = text
        .chars()
        .enumerate()
        .map(|(index, character)| {
            Cluster::new(
                TextRange::new(index as i32, index as i32 + 1),
                Text::from(character.to_string()),
                if matches!(character, '(' | ')') {
                    "latin".to_owned()
                } else {
                    "cjk".to_owned()
                },
                16.0,
            )
        })
        .collect::<Vec<_>>();
    let roles = text
        .chars()
        .map(|character| {
            if matches!(character, '(' | ')') {
                FontRole::LatinText
            } else {
                FontRole::CjkText
            }
        })
        .collect::<Vec<_>>();

    assert_eq!(
        HashSet::from([0, 1, 3, 4]),
        resolve_western_bracket_cjk_inter_char_boundaries(&text, &clusters, &roles)
    );
}

#[test]
fn western_closing_punctuation_is_forbidden_at_automatic_line_start() {
    for (name, result) in [
        ("greedy", layout_with_greedy("中文)文", 32.0)),
        ("lookahead", layout_with_lookahead("中文)文", 32.0)),
    ] {
        assert!(
            line_texts(&result, "中文)文")
                .iter()
                .all(|line| !line.starts_with(')')),
            "{name}"
        );
        let decision = result
            .debug
            .contextual_kinsoku_decisions
            .iter()
            .find(|decision| decision.source_text == ")")
            .unwrap();
        assert_eq!("LineStart", decision.forbidden_position);
        assert_eq!("Uax14WesternPunctuationBoundary:LB13", decision.reason);
    }
}

#[test]
fn western_opening_bracket_is_forbidden_at_automatic_line_end() {
    for (name, result) in [
        ("greedy", layout_with_greedy("ABCD(E", 40.0)),
        ("lookahead", layout_with_lookahead("ABCD(E", 40.0)),
    ] {
        assert!(
            line_texts(&result, "ABCD(E")
                .iter()
                .all(|line| !line.ends_with('(')),
            "{name}"
        );
        let decision = result
            .debug
            .contextual_kinsoku_decisions
            .iter()
            .find(|decision| decision.source_text == "(")
            .unwrap();
        assert_eq!("LineEnd", decision.forbidden_position);
        assert_eq!("Uax14WesternPunctuationBoundary:LB14", decision.reason);
    }
}

#[test]
fn attached_ascii_point_mark_remains_latin_and_cannot_start_wrapped_line() {
    for (name, result) in [
        ("greedy", layout_with_greedy("中文中文,中文", 64.0)),
        ("lookahead", layout_with_lookahead("中文中文,中文", 64.0)),
    ] {
        assert!(
            line_texts(&result, "中文中文,中文")
                .iter()
                .all(|line| !line.starts_with(',')),
            "{name}"
        );
        let comma = result
            .clusters
            .iter()
            .find(|cluster| cluster.text == ",")
            .unwrap();
        assert_eq!("latin-primary", comma.font_key);
        assert!(
            result
                .debug
                .punctuation_decisions
                .iter()
                .all(|decision| decision.range != comma.range)
        );
        let decision = result
            .debug
            .contextual_kinsoku_decisions
            .iter()
            .find(|decision| decision.range == comma.range)
            .unwrap();
        assert_eq!("LineStart", decision.forbidden_position);
        assert_eq!("Uax14WesternPunctuationBoundary:LB15d", decision.reason);
    }
}

#[test]
fn paired_latin_curly_quotes_keep_content_across_both_line_edges() {
    for (name, result) in [
        ("greedy", layout_with_greedy("“ABCD”E", 40.0)),
        ("lookahead", layout_with_lookahead("“ABCD”E", 40.0)),
    ] {
        assert!(
            line_texts(&result, "“ABCD”E")
                .iter()
                .all(|line| !line.starts_with('”')),
            "{name}"
        );
        let decision = result
            .debug
            .contextual_kinsoku_decisions
            .iter()
            .find(|decision| decision.source_text == "”")
            .unwrap();
        assert_eq!(
            "Uax14WesternPunctuationBoundary:PairedClosingQuote",
            decision.reason
        );
    }
}
