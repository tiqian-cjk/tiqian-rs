use tiqian::core::geometry::{LayoutConstraints, TextRange};
use tiqian::core::text::Text;
use tiqian::core::text_model::{
    LayoutInput, LineBreakPolicy, LineBreakSpan, LineLengthGrid, ParagraphStyle, TiqianTextContent,
};
use tiqian::core::units::Ic;
use tiqian::layout::line_breaker::{GreedyLineBreaker, LineBreaker, LookaheadLineBreaker};
use tiqian::layout::paragraph_dp_line_breaker::ParagraphDpLineBreaker;
use tiqian::layout::paragraph_layout_engine::{
    ExplainableStubParagraphLayoutEngine, ParagraphLayoutEngine,
};
use tiqian::linebreak::hyphenation::{Hyphenator, NoHyphenator};

fn layout(
    text: &str,
    max_width: f32,
    spans: Vec<LineBreakSpan>,
    no_hyphenation: bool,
) -> tiqian::core::layout_model::LayoutResult {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    if no_hyphenation {
        engine.hyphenator = &NoHyphenator;
    }
    engine.layout(
        LayoutInput::builder(
            TiqianTextContent::builder(Text::from(text))
                .line_break_spans(spans)
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

fn layout_with_default_grid(
    text: &str,
    max_width: f32,
    no_hyphenation: bool,
) -> tiqian::core::layout_model::LayoutResult {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    if no_hyphenation {
        engine.hyphenator = &NoHyphenator;
    }
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

fn layout_with_breaker(
    text: &str,
    max_width: f32,
    spans: Vec<LineBreakSpan>,
    breaker: Box<dyn LineBreaker>,
    hyphenator: &'static dyn Hyphenator,
) -> tiqian::core::layout_model::LayoutResult {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.line_breaker = breaker;
    engine.hyphenator = hyphenator;
    engine.layout(
        LayoutInput::builder(
            TiqianTextContent::builder(Text::from(text))
                .line_break_spans(spans)
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

fn layout_with_default_grid_breaker(
    text: &str,
    max_width: f32,
    hyphenator: &'static dyn Hyphenator,
) -> tiqian::core::layout_model::LayoutResult {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.line_breaker = Box::new(LookaheadLineBreaker::default());
    engine.hyphenator = hyphenator;
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

struct FourSevenHyphenator;

impl Hyphenator for FourSevenHyphenator {
    fn hyphenate(&self, _: &Text) -> Vec<i32> {
        vec![4, 7]
    }
}

static FOUR_SEVEN_HYPHENATOR: FourSevenHyphenator = FourSevenHyphenator;

struct TwoFourSixHyphenator;

impl Hyphenator for TwoFourSixHyphenator {
    fn hyphenate(&self, _: &Text) -> Vec<i32> {
        vec![2, 4, 6]
    }
}

static TWO_FOUR_SIX_HYPHENATOR: TwoFourSixHyphenator = TwoFourSixHyphenator;

struct TailHyphenator;

impl Hyphenator for TailHyphenator {
    fn hyphenate(&self, word: &Text) -> Vec<i32> {
        vec![word.utf16_len() - 5]
    }
}

static TAIL_HYPHENATOR: TailHyphenator = TailHyphenator;

fn line_text(result: &tiqian::core::layout_model::LayoutResult, index: usize) -> String {
    let line = &result.lines[index];
    result.clusters[line.cluster_range.first() as usize..=line.cluster_range.last() as usize]
        .iter()
        .map(|cluster| cluster.text.as_str())
        .collect()
}

#[test]
fn greedy_breaker_produces_multiple_lines_when_width_overflows() {
    let result = layout_with_default_grid("中文排版引擎测试", 64.0, false);

    assert_eq!(2, result.lines.len());
    assert_eq!(8, result.clusters.len());
    let first = &result.lines[0];
    assert_eq!(0, first.range.start());
    assert_eq!(4, first.range.end());
    assert_eq!(64.0, first.adjusted_width);
    assert_eq!(0.0, first.top);
    assert_eq!(24.0, first.bottom);
    let second = &result.lines[1];
    assert_eq!(4, second.range.start());
    assert_eq!(8, second.range.end());
    assert_eq!(64.0, second.adjusted_width);
    assert_eq!(24.0, second.top);
    assert_eq!(48.0, second.bottom);
    assert_eq!(2, result.debug.line_decisions.len());
    assert!(result.debug.line_decisions.iter().all(|decision| decision.kind == "greedy"));
    assert_eq!(48.0, result.size.height);
}

#[test]
fn camel_case_token_breaks_at_hump_without_synthetic_hyphen() {
    let result = layout("PowerPoint", 128.0, Vec::new(), false);

    assert_eq!(2, result.lines.len());
    assert!(result.lines.iter().all(|line| line.hyphen_advance == 0.0));
    assert!(
        result
            .clusters
            .iter()
            .any(|cluster| cluster.text == "Power")
    );
    assert!(
        result
            .clusters
            .iter()
            .any(|cluster| cluster.text == "Point")
    );
}

#[test]
fn all_caps_abbreviation_is_never_broken() {
    let result = layout_with_default_grid("INTERNATIONALIZATION中", 128.0, false);

    assert!(result.clusters.iter().any(|cluster| cluster.text == "INTERNATIONALIZATION"));
}

#[test]
fn existing_hyphen_break_does_not_add_synthetic_hyphen() {
    let result = layout("out-of-the-way", 128.0, Vec::new(), false);

    assert_eq!(2, result.lines.len());
    assert!(result.lines.iter().all(|line| line.hyphen_advance == 0.0));
    assert!(result.clusters.iter().any(|cluster| cluster.text == "out-"));
    assert!(result.clusters.iter().any(|cluster| cluster.text == "way"));
    assert!(line_text(&result, 0).ends_with('-'));
}

#[test]
fn url_separator_break_keeps_solidus_with_preceding_piece() {
    let result = layout("TeX/LaTeX", 80.0, Vec::new(), false);

    assert!(result.clusters.iter().any(|cluster| cluster.text == "TeX/"));
    assert!(
        result
            .clusters
            .iter()
            .any(|cluster| cluster.text == "LaTeX")
    );
    assert_eq!("TeX/", line_text(&result, 0));
    assert_eq!("LaTeX", line_text(&result, 1));
    assert!(result.lines.iter().all(|line| line.hyphen_advance == 0.0));
}

#[test]
fn overlong_latin_word_hard_breaks_with_a_hanging_hyphen() {
    let result = layout_with_default_grid("中English", 80.0, true);

    assert!(result.clusters.iter().all(|cluster| cluster.text != "English"));
    assert!(result.clusters.iter().any(|cluster| cluster.text == "En"));
    assert!(result.clusters.iter().any(|cluster| cluster.text == "ish"));
    assert_eq!(2, result.lines.len());
    assert!(result.lines[0].hyphen_advance > 0.0);
}

#[test]
fn url_like_latin_token_breaks_at_separators_without_synthetic_hyphen() {
    let url = "https://example.com/path/to/abc123def456ghi789";
    let result = layout_with_default_grid(url, 128.0, true);

    assert!(result.lines.len() > 1);
    assert!(result.lines.iter().all(|line| line.hyphen_advance == 0.0));
    assert!(result.clusters.iter().all(|cluster| cluster.text != url));
    assert!(result.clusters.iter().any(|cluster| cluster.text.ends_with('/')));
    assert!(result.clusters.iter().any(|cluster| cluster.text == "example."));
    assert!(result.debug.line_decisions.iter().all(|decision| {
        decision
            .repair_decision
            .as_ref()
            .is_none_or(|repair| repair.reason_code != "ForbiddenAtLineStart")
    }));
}

#[test]
fn opaque_token_hard_breaks_without_synthetic_hyphen() {
    let token = "abc123def456ghi789";
    let result = layout(token, 96.0, Vec::new(), true);

    assert!(result.lines.len() > 1);
    assert!(result.lines.iter().all(|line| line.hyphen_advance == 0.0));
    assert!(result.clusters.iter().all(|cluster| cluster.text != token));
    assert!(result.lines.iter().all(|line| line.visual_width <= 96.0));
}

#[test]
fn long_all_caps_opaque_token_hard_breaks_without_synthetic_hyphen() {
    let token = "QUJDREVGR0hJSktMTU5PUFFSU1RVVldYWVo";
    let result = layout_with_default_grid(token, 96.0, true);

    assert!(result.lines.len() > 1);
    assert!(result.lines.iter().all(|line| line.hyphen_advance == 0.0));
    assert!(result.clusters.iter().all(|cluster| cluster.text != token));
}

#[test]
fn opaque_latin_token_after_cjk_pulls_prefix_onto_loose_line() {
    let prefix = "为什么历史是 ";
    let result = layout_with_default_grid_breaker(
        &(prefix.to_owned() + "abc123def456ghi789"),
        160.0,
        &NoHyphenator,
    );

    assert!(line_text(&result, 0).len() > prefix.len());
    assert_eq!(0.0, result.lines[0].hyphen_advance);
}

#[test]
fn non_lexical_letter_run_after_cjk_pulls_prefix_onto_loose_line_without_synthetic_hyphen() {
    let prefix = "为什么历史是 ";
    let token = "s".repeat(40) + "herstory";
    let result = layout_with_default_grid_breaker(&(prefix.to_owned() + &token), 160.0, &NoHyphenator);

    assert!(line_text(&result, 0).len() > prefix.len());
    assert!(result.lines.iter().all(|line| line.hyphen_advance == 0.0));
}

#[test]
fn long_letter_blob_stays_opaque_even_when_tail_looks_hyphenatable() {
    let prefix = "为什么历史是 ";
    let token = "s".repeat(40) + "herstory";
    let result = layout_with_default_grid_breaker(&(prefix.to_owned() + &token), 160.0, &TAIL_HYPHENATOR);

    assert!(line_text(&result, 0).len() > prefix.len());
    assert!(result.lines.iter().all(|line| line.hyphen_advance == 0.0));
    assert!(result.clusters.iter().all(|cluster| cluster.text != token));
}

#[test]
fn long_opaque_token_can_break_even_when_it_fits_alone_but_not_after_cjk_prefix() {
    let prefix = "为什么历史是 ";
    let token = "s".repeat(40) + "herstory";
    let result = layout_with_default_grid_breaker(&(prefix.to_owned() + &token), 800.0, &TAIL_HYPHENATOR);

    assert!(line_text(&result, 0).len() > prefix.len());
    assert!(result.clusters.iter().all(|cluster| cluster.text != token));
    assert!(result.lines.iter().all(|line| line.hyphen_advance == 0.0));
}

#[test]
fn progressive_technical_break_uses_emergency_tracking_instead_of_cjk_stretch() {
    let text = "中文abcdefghij";
    let technical_range = TextRange::new(2, text.encode_utf16().count() as i32);
    let technical = LineBreakSpan {
        range: technical_range,
        policy: LineBreakPolicy::ProgressiveTechnical,
    };
    let breakers: Vec<Box<dyn LineBreaker>> = vec![
        Box::new(GreedyLineBreaker::default()),
        Box::new(LookaheadLineBreaker::default()),
        Box::new(ParagraphDpLineBreaker::default()),
    ];

    for breaker in breakers {
        let result = layout_with_breaker(
            text,
            104.0,
            vec![technical.clone()],
            breaker,
            &FOUR_SEVEN_HYPHENATOR,
        );
        assert_eq!(6, result.lines[0].range.end());
        assert_eq!(0.0, result.lines[0].hyphen_advance);
        assert!(result.debug.line_decisions[0]
            .notes
            .iter()
            .any(|note| note == "technical-break:Emergency"));
        let adjustment = result
            .debug
            .justification_decisions
            .iter()
            .find(|decision| decision.line_range == result.lines[0].range)
            .expect("expected justification decision for technical line");
        assert!(adjustment
            .allocations
            .iter()
            .all(|allocation| allocation.kind != "CjkInterChar"));
        assert!(adjustment.allocations.iter().any(|allocation| {
            allocation.kind == "EmergencyGraphemeTracking"
                && allocation.cluster_range.start() >= technical_range.start()
        }));
        assert!(adjustment.deficit_after.abs() < 0.001);
    }
}

#[test]
fn progressive_technical_structural_break_falls_through_to_emergency_before_tracking() {
    let text = "中文ab.cdEfghij";
    let result = layout_with_breaker(
        text,
        124.0,
        vec![LineBreakSpan {
            range: TextRange::new(2, text.encode_utf16().count() as i32),
            policy: LineBreakPolicy::ProgressiveTechnical,
        }],
        Box::new(LookaheadLineBreaker::default()),
        &TWO_FOUR_SIX_HYPHENATOR,
    );

    assert_eq!("中文ab.cd", line_text(&result, 0));
    assert!(result.debug.line_decisions[0]
        .notes
        .iter()
        .any(|note| note == "technical-break:Emergency"));
    assert!(result.lines.iter().all(|line| line.hyphen_advance == 0.0));
    let adjustment = &result.debug.justification_decisions[0];
    assert!(adjustment
        .allocations
        .iter()
        .all(|allocation| allocation.kind != "CjkInterChar"));
    assert!(adjustment.allocations.iter().any(|allocation| {
        allocation.kind == "EmergencyGraphemeTracking"
            && allocation.reason.starts_with("TerminalTechnicalEmergencyTracking")
    }));
}

#[test]
fn progressive_technical_hard_break_overrides_number_run_cohesion() {
    let text = "aaaaa1234567890bbbb";
    let span = LineBreakSpan {
        range: TextRange::new(0, text.encode_utf16().count() as i32),
        policy: LineBreakPolicy::ProgressiveTechnical,
    };
    let breakers: Vec<Box<dyn LineBreaker>> = vec![
        Box::new(GreedyLineBreaker::default()),
        Box::new(LookaheadLineBreaker::default()),
        Box::new(ParagraphDpLineBreaker::default()),
    ];

    for breaker in breakers {
        let result = layout_with_breaker(text, 160.0, vec![span.clone()], breaker, &NoHyphenator);
        assert_eq!("aaaaa12345", line_text(&result, 0));
        assert!(result.debug.line_decisions[0]
            .notes
            .iter()
            .any(|note| note == "technical-break:Emergency"));
    }
}

#[test]
fn progressive_technical_clean_break_may_not_stretch_earlier_opaque_token() {
    let text = "deadbeef1234deadbeef1234 ab.cdEfghijklmnop";
    let technical_range = TextRange::new(25, text.encode_utf16().count() as i32);
    let result = layout_with_breaker(
        text,
        300.0,
        vec![LineBreakSpan {
            range: technical_range,
            policy: LineBreakPolicy::ProgressiveTechnical,
        }],
        Box::new(LookaheadLineBreaker::default()),
        &TWO_FOUR_SIX_HYPHENATOR,
    );

    let affected_line_index = result
        .debug
        .line_decisions
        .iter()
        .position(|decision| decision.notes.iter().any(|note| note.starts_with("technical-break:")))
        .expect("expected technical break decision");
    assert!(result.debug.line_decisions[affected_line_index]
        .notes
        .iter()
        .any(|note| note == "technical-break:Emergency"));
    let affected_range = result.lines[affected_line_index].range;
    let adjustment = result
        .debug
        .justification_decisions
        .iter()
        .find(|decision| decision.line_range == affected_range)
        .expect("expected affected line adjustment");
    let tracking: Vec<_> = adjustment
        .allocations
        .iter()
        .filter(|allocation| allocation.kind == "EmergencyGraphemeTracking")
        .collect();
    assert!(!tracking.is_empty());
    assert!(tracking
        .iter()
        .all(|allocation| allocation.cluster_range.start() >= technical_range.start()));
}

#[test]
fn progressive_technical_break_falls_through_structural_tier_before_overstretching_outside_text() {
    let text = "中 ab/cdefghijk";
    let span = LineBreakSpan {
        range: TextRange::new(2, text.encode_utf16().count() as i32),
        policy: LineBreakPolicy::ProgressiveTechnical,
    };
    let breakers: Vec<Box<dyn LineBreaker>> = vec![
        Box::new(GreedyLineBreaker::default()),
        Box::new(LookaheadLineBreaker::default()),
        Box::new(ParagraphDpLineBreaker::default()),
    ];

    for breaker in breakers {
        let result = layout_with_breaker(text, 100.0, vec![span.clone()], breaker, &TWO_FOUR_SIX_HYPHENATOR);
        assert_eq!(7, result.lines[0].range.end());
        assert_eq!(0.0, result.lines[0].hyphen_advance);
        assert!(result.debug.line_decisions[0]
            .notes
            .iter()
            .any(|note| note == "technical-break:Syllable"));
        let adjustment = result
            .debug
            .justification_decisions
            .iter()
            .find(|decision| decision.line_range == result.lines[0].range)
            .expect("expected first line adjustment");
        assert!(adjustment.allocations.iter().all(|allocation| {
            allocation.cluster_range.end() <= span.range.start()
                || allocation.cluster_range.end() >= span.range.end()
        }));
    }
}

#[test]
fn progressive_technical_emergency_is_exposed_by_current_line_stretch_not_full_measure() {
    let text = "Swift 这边是我最有体感的。JSONDecoder 慢是个老问题，SR-6252[36] 那个 issue 里挖出的根因是底层走 NSJSONSerialization 再桥接回 Objective-C，swift_dynamicCast 吃掉大量时间。";
    let swift_range = TextRange::new(104, 121);
    let result = layout_with_breaker(
        text,
        579.0,
        vec![
            LineBreakSpan {
                range: TextRange::new(16, 27),
                policy: LineBreakPolicy::ProgressiveTechnical,
            },
            LineBreakSpan {
                range: TextRange::new(67, 86),
                policy: LineBreakPolicy::ProgressiveTechnical,
            },
            LineBreakSpan {
                range: swift_range,
                policy: LineBreakPolicy::ProgressiveTechnical,
            },
        ],
        Box::new(LookaheadLineBreaker::default()),
        &NoHyphenator,
    );

    let affected_line_index = (0..result.lines.len())
        .find(|index| line_text(&result, *index).contains("Objective-C"))
        .expect("expected Objective-C line");
    let affected_line = &result.lines[affected_line_index];
    assert_eq!(
        "erialization 再桥接回 Objective-C，swift_dy",
        line_text(&result, affected_line_index)
    );
    assert!(result.debug.line_decisions[affected_line_index]
        .notes
        .iter()
        .any(|note| note == "technical-break:Emergency"));
    let adjustment = result
        .debug
        .justification_decisions
        .iter()
        .find(|decision| decision.line_range == affected_line.range);
    let cjk_stretch = adjustment
        .map(|decision| {
            decision
                .allocations
                .iter()
                .filter(|allocation| allocation.kind == "CjkInterChar")
                .map(|allocation| allocation.delta)
                .fold(0.0_f32, f32::max)
        })
        .unwrap_or(0.0);
    assert!(cjk_stretch <= 0.001, "current line still stretched CJK body: {cjk_stretch}");
    assert!(result.debug.break_opportunity_decisions.iter().any(|decision| {
        decision.range == swift_range
            && decision.tier.as_deref() == Some("Emergency")
            && decision.reason == "CurrentLineTechnicalEmergencyBreak"
    }));
    assert!(result
        .debug
        .emergency_tracking_eligibility_decisions
        .iter()
        .any(|decision| {
            decision.range == swift_range
                && decision.reason.starts_with("CurrentLineTechnicalTierRejection:")
        }));
}

#[test]
fn unbroken_progressive_span_uses_source_space_then_keeps_body_opportunities_available() {
    let text = "甲乙ab cd丙丁戊己";
    let technical_range = TextRange::new(2, 7);
    let result = layout_with_breaker(
        text,
        129.0,
        vec![LineBreakSpan {
            range: technical_range,
            policy: LineBreakPolicy::ProgressiveTechnical,
        }],
        Box::new(GreedyLineBreaker::default()),
        &NoHyphenator,
    );
    let baseline = layout_with_breaker(
        text,
        129.0,
        Vec::new(),
        Box::new(GreedyLineBreaker::default()),
        &NoHyphenator,
    );

    assert!(result.debug.line_decisions[0]
        .notes
        .iter()
        .all(|note| !note.starts_with("technical-break:")));
    let adjustment = &result.debug.justification_decisions[0];
    assert!(!adjustment.allocations.is_empty());
    assert_eq!(
        baseline.lines.iter().map(|line| line.range).collect::<Vec<_>>(),
        result.lines.iter().map(|line| line.range).collect::<Vec<_>>(),
    );
    assert!(adjustment.deficit_after.abs() < 0.001);
    assert!(adjustment.allocations.iter().any(|allocation| {
        allocation.cluster_range == TextRange::new(4, 5)
            && allocation.kind == "ProgressiveTechnical"
            && allocation.reason == "ProgressiveTechnicalWhitespaceStretch"
    }));
    assert!(adjustment.allocations.iter().any(|allocation| {
        allocation.cluster_range.end() <= technical_range.start()
            || allocation.cluster_range.start() >= technical_range.end()
    }));
}
