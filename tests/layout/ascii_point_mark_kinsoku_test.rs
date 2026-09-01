use tiqian::clreq::clreq_profile::{
    ClreqProfile, ClreqProfileResolver, HangingPunctuationStyle, KinsokuLevel, KinsokuMode,
};
use tiqian::core::geometry::{LayoutConstraints, TextRange};
use tiqian::core::text::Text;
use tiqian::core::text_model::{
    LayoutInput, LineLengthGrid, ParagraphStyle, RubyKind, RubySpan, TextSpan, TextStyle,
    TiqianTextContent,
};
use tiqian::core::units::Ic;
use tiqian::layout::line_breaker::{GreedyLineBreaker, LookaheadLineBreaker};
use tiqian::layout::paragraph_layout_engine::{
    ExplainableStubParagraphLayoutEngine, ParagraphLayoutEngine,
};
use tiqian::linebreak::hyphenation::NoHyphenator;

struct FixedKinsokuProfile {
    level: KinsokuLevel,
    hanging: HangingPunctuationStyle,
}

impl ClreqProfileResolver for FixedKinsokuProfile {
    fn resolve(&self, _: &tiqian::core::text_model::LayoutProfileId) -> ClreqProfile {
        let mut profile = ClreqProfile::mainland_horizontal();
        profile.kinsoku_mode = KinsokuMode::fixed_with_hanging(self.level, self.hanging);
        profile
    }
}

fn layout_with_greedy(text: &str) -> tiqian::core::layout_model::LayoutResult {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.line_breaker = Box::new(GreedyLineBreaker::default());
    engine.hyphenator = &NoHyphenator;
    engine.layout(input(text))
}

fn layout_with_lookahead(text: &str) -> tiqian::core::layout_model::LayoutResult {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.line_breaker = Box::new(LookaheadLineBreaker::default());
    engine.hyphenator = &NoHyphenator;
    engine.layout(input(text))
}

fn input(text: &str) -> LayoutInput {
    LayoutInput::builder(
        TiqianTextContent::new(Text::from(text)),
        LayoutConstraints::with_defaults(64.0),
    )
    .paragraph_style(
        ParagraphStyle::builder()
            .first_line_indent(Some(Ic::ZERO))
            .build(),
    )
    .build()
}

fn layout_with(
    text: &str,
    max_width: f32,
    breaker: Box<dyn tiqian::layout::line_breaker::LineBreaker>,
    level: KinsokuLevel,
    hanging: HangingPunctuationStyle,
    first_line_indent: Option<Ic>,
    ruby_spans: Vec<RubySpan>,
    spans: Vec<TextSpan>,
    grid: bool,
) -> tiqian::core::layout_model::LayoutResult {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.line_breaker = breaker;
    engine.hyphenator = &NoHyphenator;
    engine.clreq_profile_resolver = Box::new(FixedKinsokuProfile { level, hanging });
    engine.layout(
        LayoutInput::builder(
            TiqianTextContent::builder(Text::from(text)).spans(spans).build(),
            LayoutConstraints::with_defaults(max_width),
        )
        .paragraph_style(
            ParagraphStyle::builder()
                .first_line_indent(first_line_indent)
                .line_length_grid(LineLengthGrid::with_enabled(grid))
                .build(),
        )
        .ruby_spans(ruby_spans)
        .build(),
    )
}

fn greedy_and_lookahead() -> [Box<dyn tiqian::layout::line_breaker::LineBreaker>; 2] {
    [Box::new(GreedyLineBreaker::default()), Box::new(LookaheadLineBreaker::default())]
}

fn line_texts(result: &tiqian::core::layout_model::LayoutResult, text: &str) -> Vec<String> {
    let source = Text::from(text);
    result
        .lines
        .iter()
        .map(|line| source.slice_text(line.range).to_string())
        .collect()
}

#[test]
fn cjk_attached_ascii_point_mark_is_separate_from_following_latin_run() {
    for (name, result) in [
        ("greedy", layout_with_greedy("中文,anyway继续")),
        ("lookahead", layout_with_lookahead("中文,anyway继续")),
    ] {
        assert!(
            result.clusters.iter().any(|cluster| cluster.text == ","),
            "{name}: {:?}",
            result.clusters
        );
        assert!(
            result
                .debug
                .font_decisions
                .iter()
                .any(|decision| decision.source_text == "anyway"),
            "{name}: {:?}",
            result.debug.font_decisions
        );
        assert!(
            result
                .clusters
                .iter()
                .all(|cluster| cluster.text != ",anyway"),
            "{name}: {:?}",
            result.clusters
        );
    }
}

#[test]
fn latin_tokens_keep_existing_internal_ascii_punctuation_segmentation() {
    let result = ExplainableStubParagraphLayoutEngine::default().layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from("foo,bar 1,234 50% \"quoted\"")),
            LayoutConstraints::with_defaults(1000.0),
        )
        .paragraph_style(
            ParagraphStyle::builder()
                .first_line_indent(Some(Ic::ZERO))
                .build(),
        )
        .build(),
    );
    let texts = result
        .clusters
        .iter()
        .map(|cluster| cluster.text.as_str())
        .collect::<Vec<_>>();

    for token in ["foo,bar", "1,234", "50%", "\"quoted\""] {
        assert!(texts.contains(&token), "missing {token}: {texts:?}");
    }
}

#[test]
fn cjk_attached_ascii_point_marks_cannot_start_wrapped_lines_and_stay_latin() {
    for mark in [',', '.', ':', ';', '!', '?'] {
        for breaker in greedy_and_lookahead() {
            let text = format!("中文中文{mark}中文");
            let result = layout_with(&text, 64.0, breaker, KinsokuLevel::Basic, HangingPunctuationStyle::Disabled, Some(Ic::ZERO), Vec::new(), Vec::new(), true);
            assert!(line_texts(&result, &text).iter().all(|line| !line.starts_with(mark)));
            let point = result.clusters.iter().find(|cluster| cluster.text == mark.to_string()).unwrap();
            assert_eq!("latin-primary", point.font_key);
            assert_eq!("LatinText", result.debug.font_decisions.iter().find(|decision| decision.range == point.range).unwrap().role);
            assert!(result.debug.punctuation_decisions.iter().all(|decision| decision.range != point.range));
            let decision = result.debug.contextual_kinsoku_decisions.iter().find(|decision| decision.range == point.range).unwrap();
            assert_eq!("LineStart", decision.forbidden_position);
            assert_eq!("AttachedAsciiPointMarkKinsoku", decision.reason);
        }
    }
}

#[test]
fn leading_point_mark_run_is_split_from_following_latin_text() {
    let text = "中文,anyway继续";
    for breaker in greedy_and_lookahead() {
        let result = layout_with(text, 64.0, breaker, KinsokuLevel::Basic, HangingPunctuationStyle::Disabled, Some(Ic::ZERO), Vec::new(), Vec::new(), true);
        assert!(line_texts(&result, text).iter().all(|line| !line.starts_with(',')));
        assert!(result.clusters.iter().any(|cluster| cluster.text == ","));
        assert!(result.debug.font_decisions.iter().any(|decision| decision.source_text == "anyway"));
        assert!(result.clusters.iter().all(|cluster| cluster.text != ",anyway"));
    }
}

#[test]
fn latin_tokens_and_ambiguous_ascii_characters_keep_existing_segmentation() {
    let result = layout_with("foo,bar 1,234 50% \"quoted\"", 1000.0, Box::new(GreedyLineBreaker::default()), KinsokuLevel::Basic, HangingPunctuationStyle::Disabled, Some(Ic::ZERO), Vec::new(), Vec::new(), true);
    let texts: Vec<_> = result.clusters.iter().map(|cluster| cluster.text.as_str()).collect();
    for token in ["foo,bar", "1,234", "50%", "\"quoted\""] {
        assert!(texts.contains(&token), "missing {token}: {texts:?}");
    }
    let positions: std::collections::HashSet<_> = result.debug.contextual_kinsoku_decisions.iter().filter(|decision| decision.source_text == "\"quoted\"").map(|decision| decision.forbidden_position.as_str()).collect();
    assert_eq!(std::collections::HashSet::from(["LineStart", "LineEnd"]), positions);
}

#[test]
fn point_mark_split_from_an_overlong_latin_token_still_cannot_start_a_line() {
    let text = "anyway,你";
    for width in [32.0, 36.0, 40.0, 48.0] {
        for breaker in greedy_and_lookahead() {
            let result = layout_with(text, width, breaker, KinsokuLevel::Basic, HangingPunctuationStyle::Disabled, Some(Ic::ZERO), Vec::new(), Vec::new(), true);
            assert!(line_texts(&result, text).iter().all(|line| !line.starts_with(',')));
        }
    }
}

#[test]
fn point_mark_exposed_by_a_second_stage_latin_cut_is_split_from_its_suffix() {
    let text = ".,A中";
    for breaker in greedy_and_lookahead() {
        let result = layout_with(text, 32.0, breaker, KinsokuLevel::Basic, HangingPunctuationStyle::Disabled, Some(Ic::ZERO), Vec::new(), Vec::new(), true);
        let lines = line_texts(&result, text);
        assert!(lines.iter().all(|line| !line.starts_with(',')));
        assert_eq!(".,", lines[0]);
        assert!(result.clusters.iter().any(|cluster| cluster.text == ","));
        assert!(result.clusters.iter().all(|cluster| cluster.text != ",A"));
    }
}

#[test]
fn impossible_measure_hangs_the_point_mark_instead_of_leaving_it_at_line_start() {
    let text = "中,文";
    for width in [1.0, 8.0, 15.0, 23.0, 31.0] {
        for breaker in greedy_and_lookahead() {
            let result = layout_with(text, width, breaker, KinsokuLevel::Basic, HangingPunctuationStyle::Disabled, Some(Ic::ZERO), Vec::new(), Vec::new(), true);
            assert!(line_texts(&result, text).iter().all(|line| !line.starts_with(',')));
            assert_eq!("AttachedAsciiPointMarkImpossibleMeasureHang", result.debug.contextual_kinsoku_decisions[0].impossible_measure_fallback.as_deref().unwrap());
            assert!(result.debug.line_decisions.iter().any(|decision| decision.repair.as_deref() == Some("Hang")));
        }
    }
}

#[test]
fn first_line_indent_uses_the_same_impossible_measure_fallback() {
    let text = "中,文";
    for breaker in greedy_and_lookahead() {
        let result = layout_with(text, 32.0, breaker, KinsokuLevel::Basic, HangingPunctuationStyle::Disabled, None, Vec::new(), Vec::new(), true);
        assert!(line_texts(&result, text).iter().all(|line| !line.starts_with(',')));
        assert_eq!("AttachedAsciiPointMarkImpossibleMeasureHang", result.debug.contextual_kinsoku_decisions[0].impossible_measure_fallback.as_deref().unwrap());
    }
}

#[test]
fn line_break_geometry_includes_bopomofo_spread_when_choosing_the_fallback() {
    let text = "中,文";
    for breaker in greedy_and_lookahead() {
        let result = layout_with(text, 32.0, breaker, KinsokuLevel::Basic, HangingPunctuationStyle::Disabled, Some(Ic::ZERO), vec![RubySpan::with_kind(TextRange::new(0, 1), Text::from("ㄅ"), RubyKind::Bopomofo)], Vec::new(), true);
        assert!(line_texts(&result, text).iter().all(|line| !line.starts_with(',')));
        assert_eq!("AttachedAsciiPointMarkImpossibleMeasureHang", result.debug.contextual_kinsoku_decisions[0].impossible_measure_fallback.as_deref().unwrap());
    }
}

#[test]
fn styled_point_mark_run_can_extend_one_impossible_measure_hang() {
    let text = "中!,文";
    let spans = vec![TextSpan { range: TextRange::new(2, 3), style: TextStyle::builder().font_weight(700).build() }];
    for breaker in greedy_and_lookahead() {
        let result = layout_with(text, 15.0, breaker, KinsokuLevel::Basic, HangingPunctuationStyle::Disabled, Some(Ic::ZERO), Vec::new(), spans.clone(), true);
        assert!(line_texts(&result, text).iter().all(|line| !line.starts_with('!') && !line.starts_with(',')));
        assert!(result.clusters.iter().any(|cluster| cluster.text == "!"));
        assert!(result.clusters.iter().any(|cluster| cluster.text == ","));
        assert_eq!(2, result.debug.contextual_kinsoku_decisions.iter().filter(|decision| decision.impossible_measure_fallback.as_deref() == Some("AttachedAsciiPointMarkImpossibleMeasureHang")).count());
        let hanging = result.lines.iter().find(|line| line.hanging_punctuation_advance > 0.0).unwrap();
        let expected: f32 = result.clusters.iter().filter(|cluster| cluster.text == "!" || cluster.text == ",").map(|cluster| cluster.advance).sum();
        assert_eq!(expected, hanging.hanging_punctuation_advance);
    }
}

#[test]
fn contextual_run_can_extend_a_profile_hang_only_within_the_same_protected_group() {
    let text = "中，,文";
    for breaker in greedy_and_lookahead() {
        let result = layout_with(text, 15.0, breaker, KinsokuLevel::Basic, HangingPunctuationStyle::PauseStops, Some(Ic::ZERO), Vec::new(), Vec::new(), true);
        assert!(line_texts(&result, text).iter().all(|line| !line.starts_with(',') && !line.starts_with('，')));
        assert_eq!("AttachedAsciiPointMarkImpossibleMeasureHang", result.debug.contextual_kinsoku_decisions[0].impossible_measure_fallback.as_deref().unwrap());
    }
}

#[test]
fn adjacent_impossible_groups_do_not_share_hang_provenance() {
    let text = "中!，?";
    for breaker in greedy_and_lookahead() {
        let result = layout_with(text, 15.0, breaker, KinsokuLevel::Basic, HangingPunctuationStyle::PauseStops, Some(Ic::ZERO), Vec::new(), Vec::new(), true);
        assert_eq!(2, result.lines.iter().filter(|line| line.hanging_punctuation_advance > 0.0).count());
        assert!(line_texts(&result, text).iter().all(|line| !line.starts_with('!') && !line.starts_with('?')));
    }
}

#[test]
fn compressed_closing_and_point_mark_pair_does_not_report_an_unused_hang_fallback() {
    let text = "）,文";
    for breaker in greedy_and_lookahead() {
        let result = layout_with(text, 24.0, breaker, KinsokuLevel::Basic, HangingPunctuationStyle::Disabled, Some(Ic::ZERO), Vec::new(), Vec::new(), false);
        assert_eq!("）,", line_texts(&result, text)[0]);
        assert!(result.debug.line_decisions.iter().all(|decision| decision.repair.as_deref() != Some("Hang")));
        assert_eq!(None, result.debug.contextual_kinsoku_decisions.iter().find(|decision| decision.source_text == ",").unwrap().impossible_measure_fallback);
    }
}

#[test]
fn kinsoku_none_disables_clreq_but_keeps_the_uax14_ascii_point_mark_boundary() {
    let text = "中文中文,中文";
    for breaker in greedy_and_lookahead() {
        let result = layout_with(text, 64.0, breaker, KinsokuLevel::None, HangingPunctuationStyle::Disabled, Some(Ic::ZERO), Vec::new(), Vec::new(), true);
        assert!(line_texts(&result, text).iter().all(|line| !line.starts_with(',')));
        assert_eq!("Uax14WesternPunctuationBoundary:LB15d", result.debug.contextual_kinsoku_decisions.iter().find(|decision| decision.source_text == ",").unwrap().reason);
    }
}

#[test]
fn authored_whitespace_and_mandatory_break_do_not_create_contextual_kinsoku() {
    for text in ["中\n,文", ",中文"] {
        for breaker in greedy_and_lookahead() {
            let result = layout_with(text, 1000.0, breaker, KinsokuLevel::Basic, HangingPunctuationStyle::Disabled, Some(Ic::ZERO), Vec::new(), Vec::new(), true);
            assert!(result.debug.contextual_kinsoku_decisions.is_empty());
        }
    }
    for breaker in greedy_and_lookahead() {
        let result = layout_with("中 ,文", 1000.0, breaker, KinsokuLevel::Basic, HangingPunctuationStyle::Disabled, Some(Ic::ZERO), Vec::new(), Vec::new(), true);
        assert_eq!("Uax14WesternPunctuationBoundary:LB15d", result.debug.contextual_kinsoku_decisions.iter().find(|decision| decision.source_text == ",").unwrap().reason);
    }
}

#[test]
fn mandatory_break_control_after_a_hung_point_mark_stays_in_the_trailing_suffix() {
    let text = "中,\n文";
    for breaker in greedy_and_lookahead() {
        let result = layout_with(text, 15.0, breaker, KinsokuLevel::Basic, HangingPunctuationStyle::Disabled, Some(Ic::ZERO), Vec::new(), Vec::new(), true);
        assert!(line_texts(&result, text).iter().all(|line| !line.starts_with(',')));
        assert_eq!("AttachedAsciiPointMarkImpossibleMeasureHang", result.debug.contextual_kinsoku_decisions[0].impossible_measure_fallback.as_deref().unwrap());
    }
}

#[test]
fn reported_real_world_paragraph_never_wraps_directly_before_an_ascii_comma() {
    let text = "对于你冒犯的断言不敢苟同,你以一种理所当然的语气声明\"明显的已经越过了人际尊重的基本门槛\",如此注重逻辑推导的作者居然会对论断的前提条件如此宽松以至于不留回旋余地?当然不是,在回复的一开头,聪明的作者就已经强调了自己作为被冒犯者有权力定义自己的感受,当然有权力!,但是这种感受是否可以无限扩展到\"人际尊重的基本门槛\",还是值得商榷的,逻辑严谨如你岂能放过如此基础的逻辑漏洞?";
    for width in [36.0, 40.0, 160.0, 240.0, 320.0] {
        for breaker in greedy_and_lookahead() {
            let result = layout_with(text, width, breaker, KinsokuLevel::Basic, HangingPunctuationStyle::Disabled, Some(Ic::ZERO), Vec::new(), Vec::new(), true);
            assert!(line_texts(&result, text).iter().all(|line| !line.starts_with(',')), "width={width}");
        }
    }
}
