use tiqian::clreq::clreq_profile::{ClreqProfile, ClreqProfileResolver, KinsokuLevel, KinsokuMode};
use tiqian::clreq::clreq_profile::HangingPunctuationStyle;
use tiqian::core::geometry::{scalar_offset, text_range, LayoutConstraints};
use tiqian::core::text::Text;
use tiqian::core::text_model::{LayoutInput, LineLengthGrid, ParagraphStyle, TiqianTextContent};
use tiqian::core::units::Ic;
use tiqian::layout::paragraph_layout_engine::{
    ExplainableStubParagraphLayoutEngine, ParagraphLayoutEngine,
};
use tiqian::linebreak::hyphenation::NoHyphenator;

struct FixedBasicProfile;

impl ClreqProfileResolver for FixedBasicProfile {
    fn resolve(&self, _: &tiqian::core::text_model::LayoutProfileId) -> ClreqProfile {
        let mut profile = ClreqProfile::mainland_horizontal();
        profile.kinsoku_mode = KinsokuMode::fixed(KinsokuLevel::Basic);
        profile
    }
}

fn engine() -> ExplainableStubParagraphLayoutEngine {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.clreq_profile_resolver = Box::new(FixedBasicProfile);
    engine.hyphenator = &NoHyphenator;
    engine
}

fn layout(text: &str, max_width: f32, grid: bool) -> tiqian::core::layout_model::LayoutResult {
    engine().layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from(text)),
            LayoutConstraints::with_defaults(max_width),
        )
        .paragraph_style(
            ParagraphStyle::builder()
                .first_line_indent(Some(Ic::ZERO))
                .line_length_grid(LineLengthGrid::with_enabled(grid))
                .build(),
        )
        .build(),
    )
}

fn layout_at_kinsoku(
    text: &str,
    max_width: f32,
    level: KinsokuLevel,
    hanging: HangingPunctuationStyle,
    grid: bool,
) -> tiqian::core::layout_model::LayoutResult {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.hyphenator = &NoHyphenator;
    engine.clreq_profile_resolver = Box::new(FixedKinsokuProfile { level, hanging });
    engine.layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from(text)),
            LayoutConstraints::with_defaults(max_width),
        )
        .paragraph_style(
            ParagraphStyle::builder()
                .first_line_indent(Some(Ic::ZERO))
                .line_length_grid(LineLengthGrid::with_enabled(grid))
                .build(),
        )
        .build(),
    )
}

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

fn line_text(result: &tiqian::core::layout_model::LayoutResult, index: usize) -> String {
    let line = &result.lines[index];
    result.clusters[line.cluster_range.first() as usize..=line.cluster_range.last() as usize]
        .iter()
        .map(|cluster| cluster.text.as_str())
        .collect()
}

#[test]
fn kinsoku_carries_previous_cluster_when_forbidden_punctuation_would_start_line() {
    let result = layout("中文中文。", 64.0, true);

    assert_eq!(2, result.lines.len());
    assert_eq!(scalar_offset(0), result.lines[0].range.start());
    assert_eq!(scalar_offset(3), result.lines[0].range.end());
    assert_eq!(scalar_offset(3), result.lines[1].range.start());
    assert_eq!(scalar_offset(5), result.lines[1].range.end());
    assert_eq!(48.0, result.lines[0].adjusted_width);
    assert_eq!(24.0, result.lines[1].adjusted_width);
    assert_eq!(None, result.debug.line_decisions[0].repair);
    assert_eq!(
        Some("CarryPrevious".to_owned()),
        result.debug.line_decisions[1].repair.clone()
    );
    let repair = result.debug.line_decisions[1]
        .repair_decision
        .as_ref()
        .unwrap();
    assert_eq!("CarryPrevious", repair.kind);
    assert_eq!("ForbiddenAtLineStart", repair.reason_code);
    assert_eq!(Some(3), repair.carried_cluster_index);
}

#[test]
fn kinsoku_pushes_line_start_punctuation_in_when_glue_can_shrink() {
    let result = layout("中文中。", 60.0, false);

    assert_eq!(1, result.lines.len());
    let line = &result.lines[0];
    assert_eq!(scalar_offset(0), line.range.start());
    assert_eq!(scalar_offset(4), line.range.end());
    assert_eq!(64.0, line.natural_width);
    assert_eq!(56.0, line.adjusted_width);
    assert_eq!(
        Some("PushIn".to_owned()),
        result.debug.line_decisions[0].repair.clone()
    );
    let repair = result.debug.line_decisions[0]
        .repair_decision
        .as_ref()
        .unwrap();
    assert_eq!("PushIn", repair.kind);
    assert_eq!("ForbiddenAtLineStart", repair.reason_code);
    assert_eq!(4.0, repair.shrink);
    assert_eq!(8.0, repair.available_capacity);
}

#[test]
fn kinsoku_leaves_greedy_break_alone_when_no_forbidden_punctuation_starts_line() {
    let result = layout("中文中文哈哈", 64.0, true);

    assert_eq!(2, result.lines.len());
    assert_eq!(scalar_offset(0), result.lines[0].range.start());
    assert_eq!(scalar_offset(4), result.lines[0].range.end());
    assert_eq!(scalar_offset(4), result.lines[1].range.start());
    assert_eq!(scalar_offset(6), result.lines[1].range.end());
    assert_eq!(None, result.debug.line_decisions[0].repair);
    assert_eq!(None, result.debug.line_decisions[1].repair);
}

#[test]
fn kinsoku_falls_back_to_leave_ragged_when_previous_line_cannot_spare_cluster() {
    let result = layout("Coffee。", 96.0, true);

    assert!(result.clusters.iter().any(|cluster| cluster.text == "Coffee"));
    assert_eq!(
        Some("LeaveRagged".to_owned()),
        result.debug.line_decisions[1].repair.clone()
    );
    assert_eq!(20, result.debug.line_decisions[1].repair_penalty);
    assert!(result.debug.line_decisions[1]
        .notes
        .iter()
        .any(|note| note.contains("ForbiddenAtLineStart:。") && note.contains("no-room-to-carry")));
}

#[test]
fn long_latin_sentence_wraps_at_word_boundaries() {
    let result = layout("The quick brown fox", 160.0, true);

    assert!(result.lines.len() > 1, "long Latin must wrap at word boundaries");
    for line in &result.lines {
        let line_clusters: Vec<_> = result
            .clusters
            .iter()
            .filter(|cluster| {
                cluster.range.start() >= line.range.start() && cluster.range.end() <= line.range.end()
            })
            .collect();
        let first = line_clusters.first().expect("line must have a cluster");
        let last = line_clusters.last().expect("line must have a cluster");
        if first.text.chars().all(|character| character == ' ') {
            assert_eq!(0.0, first.advance);
        }
        if last.text.chars().all(|character| character == ' ') {
            assert_eq!(0.0, last.advance);
        }
    }
}

#[test]
fn numeric_suffix_symbol_remains_on_one_line() {
    let text = "销量增长了50%呢";
    let result = layout(text, 120.0, false);
    let line_texts: Vec<_> = result
        .lines
        .iter()
        .map(|line| {
            text.chars()
                .skip(line.range.start().value() as usize)
                .take((line.range.end() - line.range.start()) as usize)
                .collect::<String>()
        })
        .collect();

    assert!(
        line_texts.iter().any(|line| line.contains("50%")),
        "50% must stay together: {line_texts:?}"
    );
    assert!(
        line_texts.iter().all(|line| !line.ends_with("50")),
        "no line may end mid-number: {line_texts:?}"
    );
}

#[test]
fn bibliographic_numeric_locator_exposes_structural_breaks() {
    let text = "中文中文中文44(10):21-38.";
    let result = layout(text, 224.0, false);
    let locator_start = text
        .split_once("44")
        .map(|(prefix, _)| prefix.chars().count() as i32)
        .expect("expected locator");
    let decision = result
        .debug
        .break_opportunity_decisions
        .first()
        .expect("expected bibliographic decision");

    assert_eq!(text_range(locator_start, text.chars().count() as i32), decision.range);
    assert_eq!("44(10):21-38.", decision.source_text);
    assert_eq!(
        vec![
            text.split_once('(')
                .map(|(prefix, _)| scalar_offset(prefix.chars().count() as i32))
                .expect("expected opening parenthesis"),
            text.split_once(':')
                .map(|(prefix, _)| scalar_offset(prefix.chars().count() as i32 + 1))
                .expect("expected colon"),
        ],
        decision.break_offsets,
    );
    assert_eq!("BibliographicNumericLocatorBreak", decision.reason);
    let lines: Vec<_> = (0..result.lines.len()).map(|index| line_text(&result, index)).collect();
    assert!(lines[0].ends_with("44(10):"), "locator should fill preceding line: {lines:?}");
    assert_eq!("21-38.", lines.last().expect("expected last line"));
    assert!(lines.iter().all(|line| !line.ends_with('(')), "opening bracket cannot end a line: {lines:?}");
    assert!(lines.iter().all(|line| !line.starts_with(')')), "closing bracket cannot start a line: {lines:?}");
}

#[test]
fn ordinary_numeric_forms_do_not_become_bibliographic_locators() {
    for token in ["3.14", "1,000", "12:34", "2023-08-11"] {
        let result = layout(&("中文".to_owned() + token), 320.0, false);
        assert!(
            result.debug.break_opportunity_decisions.is_empty(),
            "{token} must keep its existing numeric/token policy: {:?}",
            result.debug.break_opportunity_decisions,
        );
    }
}

#[test]
fn kinsoku_level_none_leaves_forbidden_marks_at_line_start() {
    let text = "中文中。中";
    let none = layout_at_kinsoku(
        text,
        48.0,
        KinsokuLevel::None,
        HangingPunctuationStyle::Disabled,
        true,
    );
    assert!(none.debug.line_decisions.iter().all(|decision| decision.repair.is_none()));
    assert!(none.lines.iter().any(|line| line.range.start().value() == 3));

    let basic = layout_at_kinsoku(
        text,
        48.0,
        KinsokuLevel::Basic,
        HangingPunctuationStyle::Disabled,
        true,
    );
    assert!(basic.debug.line_decisions.iter().any(|decision| decision.repair.is_some()));
}

#[test]
fn kinsoku_level_strict_forbids_dash_at_line_start() {
    let text = "中文中——文";
    let basic = layout_at_kinsoku(
        text,
        48.0,
        KinsokuLevel::Basic,
        HangingPunctuationStyle::Disabled,
        true,
    );
    assert!(basic.debug.line_decisions.iter().all(|decision| decision.repair.is_none()));

    let strict = layout_at_kinsoku(
        text,
        48.0,
        KinsokuLevel::Strict,
        HangingPunctuationStyle::Disabled,
        true,
    );
    assert!(strict.debug.line_decisions.iter().any(|decision| decision.repair.is_some()));
}

#[test]
fn line_end_kinsoku_moves_dangling_opener_to_next_line() {
    let result = layout("中中中（中中）中", 64.0, true);

    for line in &result.lines {
        let last = result
            .clusters
            .iter()
            .rev()
            .find(|cluster| cluster.range.end() <= line.range.end())
            .unwrap();
        assert_ne!("（", last.text, "line must not end on an opening bracket");
    }
    assert!(
        result
            .debug
            .line_decisions
            .iter()
            .any(|decision| decision.repair.as_deref() == Some("CarryNext"))
    );
}

#[test]
fn hanging_punctuation_fills_line_to_measure_and_overflows_visual() {
    let text = "中文中文，中文。";
    let hanging = layout_at_kinsoku(
        text,
        64.0,
        KinsokuLevel::Basic,
        HangingPunctuationStyle::PauseStops,
        true,
    );

    assert!(hanging.lines.len() >= 2);
    let line = &hanging.lines[0];
    assert_eq!(scalar_offset(0), line.range.start());
    assert_eq!(scalar_offset(5), line.range.end());
    assert_eq!(64.0, line.adjusted_width);
    assert!(line.visual_width > 64.0, "hung mark must overflow: {}", line.visual_width);
    assert_eq!(
        line.visual_width - line.adjusted_width,
        line.hanging_punctuation_advance
    );
    assert_eq!(Some("Hang".to_owned()), hanging.debug.line_decisions[0].repair.clone());

    let plain = layout_at_kinsoku(
        text,
        64.0,
        KinsokuLevel::Basic,
        HangingPunctuationStyle::Disabled,
        true,
    );
    assert!(plain.lines.iter().all(|line| line.visual_width <= 64.0));
    assert!(plain
        .debug
        .line_decisions
        .iter()
        .all(|decision| decision.repair.as_deref() != Some("Hang")));
}
