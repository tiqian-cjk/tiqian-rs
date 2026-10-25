use std::sync::Arc;
mod support;
use tiqian::common::HashMap;
use tiqian::core::font_face::FontFaceId;
use tiqian::core::geometry::text_range;
use tiqian::core::int_range::IntRange;
use tiqian::core::layout_model::{AutoSpaceBoundaryRole, AutoSpaceSide, Cluster, LineEndReason, SyntheticClusterKind};
use tiqian::core::text::Text;
use tiqian::core::text_model::{TextWrapMode, WhiteSpaceMode};
use tiqian::layout::line_breaker::{GreedyLineBreaker, LineBreaker, LineBreakerConfig, LookaheadLineBreaker};
use tiqian::layout::line_measurement::LineMeasurement;
use tiqian::layout::line_optimization::{RepairOption, RepairReasonCode};
use tiqian::layout::line_repair::try_push_in;
use tiqian::layout::paragraph_dp_line_breaker::ParagraphDpLineBreaker;
use tiqian::layout::progressive_break_decisions::{ProgressiveBreakTier, ShrinkChannel, ShrinkOpportunity};
use tiqian::layout::white_space::{scan_white_space, LineWhiteSpaceAction};
use tiqian::layout::punctuation_model::GlueKind;
use tiqian::clreq::clreq_profile::AutoSpaceMode;

fn prepared(text: &str, mode: WhiteSpaceMode, wrap: TextWrapMode, interval: f32, indent: f32) -> (Vec<Cluster>, LineBreakerConfig) {
    let source = Text::from(text);
    let units: HashMap<_, _> = scan_white_space(&source, mode, &HashMap::new()).into_iter().map(|unit| (unit.range, unit)).collect();
    let clusters: Vec<_> = text.chars().enumerate().map(|(i, c)| {
        let range = text_range(i as i32, i as i32 + 1);
        if c == '\t' || c == '\n' {
            Cluster::synthetic(range, Text::from(c.to_string()), Text::from(if c == '\t' { "\t" } else { "" }),
                if c == '\t' { SyntheticClusterKind::WhiteSpace } else { SyntheticClusterKind::MandatoryBreak }, 0.0)
        } else {
            Cluster::new(range, Text::from(c.to_string()), FontFaceId::with_resource_id("test"), 1.0)
        }
    }).collect();
    let mut config = LineBreakerConfig::default();
    config.first_line_indent = indent;
    config.hard_break_after_clusters = text.chars().enumerate().filter_map(|(i, c)| (c == '\n').then_some(i as i32)).collect();
    config.measurement = Some(Arc::new(LineMeasurement::new(&clusters, &clusters, &units, &[], mode, wrap,
        interval, &text.chars().enumerate().filter_map(|(i, c)| (c == '\t').then_some((i as i32, 0.5))).collect(), 0.0, indent)));
    (clusters, config)
}

fn breakers() -> Vec<Box<dyn LineBreaker>> {
    vec![Box::new(GreedyLineBreaker::default()), Box::new(LookaheadLineBreaker::default()), Box::new(ParagraphDpLineBreaker::default())]
}

#[test]
fn phase_three_nowrap_keeps_only_hard_breaks_and_terminal_empty_line() {
    for mode in [WhiteSpaceMode::Collapse, WhiteSpaceMode::PreserveBreaks, WhiteSpaceMode::Preserve, WhiteSpaceMode::BreakSpaces] {
        let (clusters, config) = prepared("a  b\n", mode, TextWrapMode::NoWrap, 8.0, 0.0);
        for breaker in breakers() {
            let solution = breaker.break_lines(&clusters, &clusters, 0.25, &config);
            assert_eq!(2, solution.lines.len(), "{} {mode:?}", breaker.strategy_name());
            assert_eq!(IntRange::new(0, 4), solution.lines[0].cluster_range);
            assert!(solution.lines[1].cluster_range.is_empty());
        }
    }
}

#[test]
fn phase_three_preserve_runs_hang_and_break_spaces_count_every_space() {
    for breaker in breakers() {
        let (clusters, config) = prepared("a   b", WhiteSpaceMode::Preserve, TextWrapMode::Wrap, 8.0, 0.0);
        let solution = breaker.break_lines(&clusters, &clusters, 1.0, &config);
        assert_eq!(IntRange::new(0, 3), solution.lines[0].cluster_range, "{}", breaker.strategy_name());
        assert_eq!(4.0, solution.lines[0].natural_width);
        assert_eq!(1.0, solution.lines[0].adjusted_width);
        assert_eq!(3, solution.lines[0].white_space_adjustments.iter().filter(|a| a.action == LineWhiteSpaceAction::HangEnd).count());
        let (clusters, config) = prepared("   ", WhiteSpaceMode::BreakSpaces, TextWrapMode::Wrap, 8.0, 0.0);
        let solution = breaker.break_lines(&clusters, &clusters, 1.0, &config);
        assert_eq!(3, solution.lines.len(), "{}", breaker.strategy_name());
        assert!(solution.lines.iter().all(|line| line.adjusted_width == 1.0));
    }
}

#[test]
fn phase_three_conditional_hang_and_tab_grid_depend_on_candidate_start() {
    let (clusters, config) = prepared("a  ", WhiteSpaceMode::Preserve, TextWrapMode::Wrap, 8.0, 0.0);
    let solution = GreedyLineBreaker::default().break_lines(&clusters, &clusters, 2.0, &config);
    let line = &solution.lines[0];
    assert_eq!(3.0, line.natural_width);
    assert_eq!(2.0, line.adjusted_width);
    assert_eq!(1.0, line.white_space_adjustments.iter().map(|a| a.hanging_advance).sum::<f32>());
    let (clusters, config) = prepared("a\tb", WhiteSpaceMode::BreakSpaces, TextWrapMode::Wrap, 8.0, 2.0);
    let first = config.build_line(IntRange::new(0, 2), &clusters, &clusters, LineEndReason::ParagraphEnd, 30.0);
    let later = config.build_line(IntRange::new(1, 2), &clusters, &clusters, LineEndReason::ParagraphEnd, 30.0);
    assert_eq!(7.0, first.natural_width);
    assert_eq!(9.0, later.natural_width);
    let (clusters, config) = prepared("a\tb", WhiteSpaceMode::BreakSpaces, TextWrapMode::Wrap, 0.0, 0.0);
    assert_eq!(2.0, config.build_line(IntRange::new(0, 2), &clusters, &clusters, LineEndReason::ParagraphEnd, 30.0).natural_width);
}

fn repair_case(pen: f32, capacity: f32, limit: f32) -> (bool, f32, f32) {
    let (mut clusters, mut config) = prepared("a\tb", WhiteSpaceMode::BreakSpaces, TextWrapMode::Wrap, 8.0, 0.0);
    clusters[0].advance = pen;
    let units = scan_white_space(&Text::from("a\tb"), WhiteSpaceMode::BreakSpaces, &HashMap::new()).into_iter().map(|unit| (unit.range, unit)).collect();
    config.measurement = Some(Arc::new(LineMeasurement::new(&clusters, &clusters, &units, &[], WhiteSpaceMode::BreakSpaces,
        TextWrapMode::Wrap, 8.0, &HashMap::from([(1, 0.5)]), 0.0, 0.0)));
    let previous = config.build_line(IntRange::new(0, 1), &clusters, &clusters, LineEndReason::AutoWrap, limit);
    let current = config.build_line(IntRange::new(2, 2), &clusters, &clusters, LineEndReason::ParagraphEnd, limit);
    let result = try_push_in(&previous, &current, &clusters, &clusters, limit,
        &[ShrinkOpportunity::new(0, 2, capacity, ShrinkChannel::RawAdvance)], 2, None, RepairReasonCode::LineAdjustmentPushIn);
    let shrink = match result.previous.repair { Some(RepairOption::PushIn { total_shrink, .. }) => total_shrink, _ => 0.0 };
    (result.candidate.accepted, result.previous.adjusted_width, shrink)
}

#[test]
fn phase_three_repair_remeasures_absorption_and_stop_jump() {
    assert!(!repair_case(6.0, 1.0, 8.5).0);
    let (accepted, width, shrink) = repair_case(7.75, 0.5, 9.0);
    assert!(accepted);
    assert!(width <= 9.0);
    assert!(shrink <= 0.5);
    assert!(shrink >= 0.25);
}

#[test]
fn phase_three_small_tab_intervals_match_feasibility_samples() {
    for interval in [0.125, 0.25, 0.5, 1.0, 8.0] {
        for pen_step in 1..40 {
            let pen = pen_step as f32 * 0.125;
            let (mut clusters, _) = prepared("a\tb", WhiteSpaceMode::BreakSpaces, TextWrapMode::Wrap, interval, 0.0);
            clusters[0].advance = pen;
            let units = scan_white_space(&Text::from("a\tb"), WhiteSpaceMode::BreakSpaces, &HashMap::new()).into_iter().map(|unit| (unit.range, unit)).collect();
            let measurement = LineMeasurement::new(&clusters, &clusters, &units, &[], WhiteSpaceMode::BreakSpaces,
                TextWrapMode::Wrap, interval, &HashMap::from([(1, 0.5)]), 0.0, 0.0);
            let range = IntRange::new(0, 2);
            let capacity = pen.min(1.0);
            let opportunities = [ShrinkOpportunity::new(0, 2, capacity, ShrinkChannel::RawAdvance)];
            for width_step in 1..12 {
                let limit = width_step as f32 * 0.5;
                let sampled = (0..=256).any(|step| {
                    measurement.measure(range, LineEndReason::ParagraphEnd, limit,
                        &[tiqian::layout::line_optimization::PushInAllocation { cluster_index: 0,
                            shrink: capacity * step as f32 / 256.0, available_capacity: capacity, channel: ShrinkChannel::RawAdvance }]).adjusted <= limit
                });
                let solution = measurement.compression(range, LineEndReason::ParagraphEnd, limit, &opportunities);
                assert!(!sampled || solution.is_some(), "interval={interval} pen={pen} limit={limit}");
                if let Some(allocations) = solution {
                    assert!(measurement.measure(range, LineEndReason::ParagraphEnd, limit, &allocations).adjusted <= limit);
                }
            }
        }
    }
}

#[test]
fn phase_three_collapsed_hidden_sources_do_not_create_extra_lines() {
    for mode in [WhiteSpaceMode::Collapse, WhiteSpaceMode::PreserveBreaks] {
        let source = Text::from("a   b   ");
        let (mut clusters, mut config) = prepared(source.as_str(), mode, TextWrapMode::Wrap, 8.0, 0.0);
        let units: HashMap<_, _> = scan_white_space(&source, mode, &HashMap::new()).into_iter().map(|unit| (unit.range, unit)).collect();
        for cluster in &mut clusters {
            if units.get(&cluster.range).is_some_and(|unit| matches!(unit.action,
                tiqian::layout::white_space::WhiteSpaceAction::Collapse | tiqian::layout::white_space::WhiteSpaceAction::Remove)) {
                cluster.advance = 0.0;
                cluster.display_text = Text::new();
            }
        }
        config.measurement = Some(Arc::new(LineMeasurement::new(&clusters, &clusters, &units, &[], mode, TextWrapMode::Wrap,
            0.0, &HashMap::new(), 0.0, 0.0)));
        for breaker in breakers() {
            let solution = breaker.break_lines(&clusters, &clusters, 1.0, &config);
            assert_eq!(2, solution.lines.len(), "{} {mode:?}", breaker.strategy_name());
            assert_eq!(8, solution.lines.last().unwrap().cluster_range.last() + 1);
            assert!(solution.lines.iter().all(|line| line.adjusted_width == 1.0));
        }
    }
}

#[test]
fn phase_three_progressive_uses_tab_resolved_candidate_width() {
    use tiqian::layout::progressive_break_decisions::{ProgressiveBreakOpportunity, ProgressiveBreakTier};
    let (clusters, mut config) = prepared("a\tbcd", WhiteSpaceMode::BreakSpaces, TextWrapMode::Wrap, 8.0, 0.0);
    config.max_cjk_stretch_per_gap = 1.0;
    config.progressive_break_opportunities = HashMap::from([
        (2, ProgressiveBreakOpportunity::new(ProgressiveBreakTier::Whitespace, text_range(0, 5))),
        (3, ProgressiveBreakOpportunity::new(ProgressiveBreakTier::Structural, text_range(0, 5))),
        (4, ProgressiveBreakOpportunity::new(ProgressiveBreakTier::Emergency, text_range(0, 5))),
    ]);
    for breaker in breakers() {
        let solution = breaker.break_lines(&clusters, &clusters, 8.0, &config);
        assert_eq!(IntRange::new(0, 1), solution.lines[0].cluster_range, "{}", breaker.strategy_name());
        assert_eq!(8.0, solution.lines[0].adjusted_width);
    }
}

#[test]
fn phase_three_autospace_disappears_before_tab_pen_and_is_recorded_once() {
    use tiqian::core::layout_model::AutoSpaceDecisionInfo;
    let (mut clusters, mut config) = prepared("a\tb", WhiteSpaceMode::BreakSpaces, TextWrapMode::NoWrap, 8.0, 0.0);
    clusters[0].advance = 7.75;
    let units = scan_white_space(&Text::from("a\tb"), WhiteSpaceMode::BreakSpaces, &HashMap::new()).into_iter().map(|unit| (unit.range, unit)).collect();
    let decision = AutoSpaceDecisionInfo { cluster_range: text_range(0, 1), side: AutoSpaceSide::Leading,
        boundary_role: AutoSpaceBoundaryRole::EastAsianSpacingWide, mode: AutoSpaceMode::Insert, characters_affected: 0,
        reduction_per_char: 0.0, total_reduction: -0.5, reason: "test" };
    config.measurement = Some(Arc::new(LineMeasurement::new(&clusters, &clusters, &units, &[decision], WhiteSpaceMode::BreakSpaces,
        TextWrapMode::NoWrap, 8.0, &HashMap::from([(1, 0.5)]), 0.0, 0.0)));
    for breaker in breakers() {
        let solution = breaker.break_lines(&clusters, &clusters, 30.0, &config);
        assert_eq!(17.0, solution.lines[0].natural_width);
        assert_eq!(9.0, solution.lines[0].adjusted_width);
        assert_eq!(1, solution.lines[0].white_space_adjustments.iter().filter(|a| a.action == LineWhiteSpaceAction::RemoveAutoSpaceStart).count());
        assert_eq!(0.75, solution.lines[0].white_space_adjustments.iter().find(|a| a.action == LineWhiteSpaceAction::ResolveTab).unwrap().occupied_advance);
    }
}

#[test]
fn phase_three_dp_commits_feasible_tab_compression_and_preserves_source() {
    let (mut clusters, mut config) = prepared("a\tbcd", WhiteSpaceMode::BreakSpaces, TextWrapMode::Wrap, 8.0, 0.0);
    clusters[0].advance = 7.75;
    let units = scan_white_space(&Text::from("a\tbcd"), WhiteSpaceMode::BreakSpaces, &HashMap::new()).into_iter().map(|unit| (unit.range, unit)).collect();
    config.measurement = Some(Arc::new(LineMeasurement::new(&clusters, &clusters, &units, &[], WhiteSpaceMode::BreakSpaces,
        TextWrapMode::Wrap, 8.0, &HashMap::from([(1, 0.5)]), 0.0, 0.0)));
    config.line_adjustment_push_in = true;
    config.shrink_opportunities = vec![ShrinkOpportunity::new(0, 2, 0.5, ShrinkChannel::RawAdvance)];
    config.max_cjk_stretch_per_gap = 1.0;
    let solution = ParagraphDpLineBreaker::default().break_lines(&clusters, &clusters, 9.0, &config);
    assert!(solution.lines.iter().all(|line| line.adjusted_width <= 9.0));
    assert_eq!((0..5).collect::<Vec<_>>(), solution.lines.iter().flat_map(|line| line.cluster_range.into_iter()).collect::<Vec<_>>());
    assert!(solution.lines.iter().any(|line| matches!(&line.repair, Some(RepairOption::PushIn { total_shrink, .. }) if *total_shrink <= 0.5)));
}

#[test]
fn phase_three_compression_keeps_tier_order_and_capacity_proportions() {
    let (clusters, config) = prepared("abcdef", WhiteSpaceMode::BreakSpaces, TextWrapMode::Wrap, 0.0, 0.0);
    let measurement = config.measurement.as_ref().unwrap();
    let allocations = measurement.compression(IntRange::new(0, 5), LineEndReason::ParagraphEnd, 4.0, &[
        ShrinkOpportunity::new(0, 1, 0.5, ShrinkChannel::RawAdvance),
        ShrinkOpportunity::new(1, 2, 1.0, ShrinkChannel::RawAdvance),
        ShrinkOpportunity::new(2, 2, 3.0, ShrinkChannel::RawAdvance),
        ShrinkOpportunity::new(3, 3, 2.0, ShrinkChannel::RawAdvance),
    ]).unwrap();
    assert_eq!(3, allocations.len());
    assert_eq!(0.5, allocations[0].shrink);
    assert_eq!(allocations[1].shrink * 3.0, allocations[2].shrink);
    assert_eq!(6, clusters.len());
}

#[test]
fn phase_three_planning_measures_default_space_and_current_tab_font() {
    use tiqian::clreq::clreq_profile::BuiltInClreqProfileResolver;
    use tiqian::core::geometry::LayoutConstraints;
    use tiqian::core::text_model::{LayoutInput, ParagraphStyle, TabSize, TextStyle, TiqianTextContent};
    use tiqian::font::font_metrics::ScriptAwareFontMetricsNormalizer;
    use tiqian::font::font_policy::CjkFontRoleClassifier;
    use tiqian::layout::default_hyphenator::default_hyphenator;
    use tiqian::layout::justifier::Justifier;
    use tiqian::layout::line_break_planning_stage::{plan_paragraph_lines, LineBreakPlanningRequest};
    use tiqian::layout::punctuation_model::{PunctuationAtomBuilder, PunctuationSpacingCompressor};
    use tiqian::layout::quote_pair_analyzer::QuotePairAnalyzer;
    use tiqian::layout::width_independent_annotation_cache::{build_paragraph_layout_prep, prepare_width_independent_annotation};
    let backend = support::DeterministicStubFontBackend::default();
    for (tab_size, expected) in [(TabSize::default(), 80.0), (TabSize::SpaceAdvanceMultiple(2.0), 48.0),
        (TabSize::LayoutUnits(24.0), 40.0), (TabSize::LayoutUnits(0.0), 32.0)] {
        let input = LayoutInput::builder(TiqianTextContent::new(Text::from("a\tb")), LayoutConstraints::with_defaults(200.0))
            .paragraph_style(ParagraphStyle::builder().white_space(WhiteSpaceMode::BreakSpaces)
                .first_line_indent(Some(tiqian::core::units::Ic::ZERO))
                .text_wrap(TextWrapMode::NoWrap).tab_size(tab_size).build())
            .text_style(TextStyle::builder().font_size(16.0).build()).build();
        let rejected = HashMap::new();
        let annotation = prepare_width_independent_annotation(&input, &rejected, &BuiltInClreqProfileResolver,
            &CjkFontRoleClassifier, &backend, &QuotePairAnalyzer, default_hyphenator());
        let prep = build_paragraph_layout_prep(&input, &annotation, &rejected, &backend, default_hyphenator(),
            &PunctuationAtomBuilder::default(), &PunctuationSpacingCompressor);
        for breaker in breakers() {
            let result = plan_paragraph_lines(LineBreakPlanningRequest::new(&prep, &backend,
                &ScriptAwareFontMetricsNormalizer, &Justifier::default(), breaker.as_ref()));
            assert_eq!(expected, result.line_solution.lines[0].natural_width, "{} {tab_size:?}", breaker.strategy_name());
            assert_eq!(expected, result.line_solution.lines[0].adjusted_width);
        }
    }
}

#[test]
fn phase_three_multiple_tabs_and_mixed_minimums_replay_the_same_width() {
    for interval in [0.25, 1.0, 8.0] {
        let (mut clusters, _) = prepared("a\tb\tc", WhiteSpaceMode::BreakSpaces, TextWrapMode::Wrap, interval, 0.0);
        clusters[0].advance = 7.75;
        clusters[2].advance = 0.75;
        let units = scan_white_space(&Text::from("a\tb\tc"), WhiteSpaceMode::BreakSpaces, &HashMap::new()).into_iter().map(|unit| (unit.range, unit)).collect();
        let measurement = LineMeasurement::new(&clusters, &clusters, &units, &[], WhiteSpaceMode::BreakSpaces,
            TextWrapMode::Wrap, interval, &HashMap::from([(1, 0.5), (3, 1.5)]), 0.0, 0.0);
        let range = IntRange::new(0, 4);
        let opportunities = [ShrinkOpportunity::new(0, 2, 0.5, ShrinkChannel::RawAdvance),
            ShrinkOpportunity::new(2, 2, 0.5, ShrinkChannel::RawAdvance)];
        for step in 0..32 {
            let limit = step as f32 * 0.5;
            let sampled = (0..=128).any(|step| {
                let allocations = opportunities.iter().map(|opp| tiqian::layout::line_optimization::PushInAllocation {
                    cluster_index: opp.cluster_index, shrink: opp.capacity * step as f32 / 128.0,
                    available_capacity: opp.capacity, channel: opp.channel }).collect::<Vec<_>>();
                measurement.measure(range, LineEndReason::ParagraphEnd, limit, &allocations).adjusted <= limit
            });
            let result = measurement.compression(range, LineEndReason::ParagraphEnd, limit, &opportunities);
            assert!(!sampled || result.is_some(), "interval={interval} limit={limit}");
            if let Some(allocations) = result {
                let measured = measurement.measure(range, LineEndReason::ParagraphEnd, limit, &allocations);
                assert!(measured.adjusted <= limit);
                let records = measurement.adjustments(range, LineEndReason::ParagraphEnd, limit, &allocations);
                let occupied: f32 = clusters.iter().map(|cluster| cluster.advance).sum::<f32>()
                    - allocations.iter().map(|allocation| allocation.shrink).sum::<f32>()
                    + records.iter().filter(|record| record.action == LineWhiteSpaceAction::ResolveTab).map(|record| record.occupied_advance).sum::<f32>();
                assert!((occupied - measured.adjusted).abs() < 0.0001);
            }
        }
    }
}

#[test]
fn phase_three_narrow_and_pure_whitespace_lines_keep_every_source_unit() {
    for mode in [WhiteSpaceMode::Preserve, WhiteSpaceMode::BreakSpaces] {
        for text in ["   ", "\t\t", "a \tb\n", "a  "] {
            for width in [0.01, 0.5, 1.0] {
                let (clusters, config) = prepared(text, mode, TextWrapMode::Wrap, 8.0, 0.0);
                for breaker in breakers() {
                    let solution = breaker.break_lines(&clusters, &clusters, width, &config);
                    let indices: Vec<_> = solution.lines.iter().filter(|line| !line.cluster_range.is_empty()).flat_map(|line| line.cluster_range.into_iter()).collect();
                    assert_eq!((0..clusters.len() as i32).collect::<Vec<_>>(), indices, "{} {mode:?} {text:?} {width}", breaker.strategy_name());
                    if mode == WhiteSpaceMode::BreakSpaces {
                        assert!(solution.lines.iter().flat_map(|line| &line.white_space_adjustments)
                            .all(|record| record.hanging_advance == 0.0));
                    }
                }
            }
        }
    }
}

#[test]
fn phase_three_hard_break_tail_hangs_only_overflow_and_stays_on_the_line() {
    for breaker in breakers() {
        let (clusters, config) = prepared("a  \nb", WhiteSpaceMode::Preserve, TextWrapMode::Wrap, 8.0, 0.0);
        let solution = breaker.break_lines(&clusters, &clusters, 2.0, &config);
        assert_eq!(IntRange::new(0, 3), solution.lines[0].cluster_range, "{}", breaker.strategy_name());
        assert_eq!(LineEndReason::MandatoryBreak, solution.lines[0].end_reason);
        assert_eq!(2.0, solution.lines[0].adjusted_width);
        assert_eq!(1.0, solution.lines[0].white_space_adjustments.iter().map(|record| record.hanging_advance).sum::<f32>());
    }
}

#[test]
fn phase_three_push_in_preserves_space_run_boundary_and_records_actual_tail_advance() {
    let (clusters, config) = prepared("ab  c", WhiteSpaceMode::Preserve, TextWrapMode::Wrap, 8.0, 0.0);
    let previous = config.build_line(IntRange::new(0, 0), &clusters, &clusters, LineEndReason::AutoWrap, 2.0);
    let current = config.build_line(IntRange::new(1, 4), &clusters, &clusters, LineEndReason::ParagraphEnd, 2.0);
    let pushed = try_push_in(&previous, &current, &clusters, &clusters, 2.0, &[], 2, Some(2), RepairReasonCode::LineAdjustmentPushIn);
    assert!(pushed.candidate.accepted);
    assert_eq!(IntRange::new(0, 3), pushed.previous.cluster_range);
    assert_eq!(IntRange::new(4, 4), pushed.current.unwrap().cluster_range);
    let measurement = config.measurement.as_ref().unwrap();
    let allocations = [tiqian::layout::line_optimization::PushInAllocation {
        cluster_index: 3, shrink: 0.5, available_capacity: 0.5, channel: ShrinkChannel::RawAdvance,
    }];
    let records = measurement.adjustments(IntRange::new(0, 3), LineEndReason::AutoWrap, 2.0, &allocations);
    assert_eq!(1.5, records.iter().map(|record| record.hanging_advance).sum::<f32>());
    assert_eq!(0.5, records.iter().find(|record| record.cluster_index == 3).unwrap().occupied_advance);
}

#[test]
fn phase_three_tab_threshold_is_strict_and_block_indent_keeps_grid_origin() {
    let (mut clusters, _) = prepared("a\tb", WhiteSpaceMode::BreakSpaces, TextWrapMode::NoWrap, 8.0, 0.0);
    let units = scan_white_space(&Text::from("a\tb"), WhiteSpaceMode::BreakSpaces, &HashMap::new()).into_iter().map(|unit| (unit.range, unit)).collect();
    for (pen, minimum, expected) in [(7.5, 0.5, 9.0), (7.75, 0.5, 17.0), (7.75, 0.0, 9.0)] {
        clusters[0].advance = pen;
        let mut config = LineBreakerConfig::default();
        config.measurement = Some(Arc::new(LineMeasurement::new(&clusters, &clusters, &units, &[],
            WhiteSpaceMode::BreakSpaces, TextWrapMode::NoWrap, 8.0, &HashMap::from([(1, minimum)]), 0.0, 0.0)));
        for breaker in breakers() {
            let solution = breaker.break_lines(&clusters, &clusters, 30.0, &config);
            assert_eq!(expected, solution.lines[0].adjusted_width, "{} pen={pen} minimum={minimum}", breaker.strategy_name());
        }
    }
    clusters[0].advance = 1.0;
    let measurement = LineMeasurement::new(&clusters, &clusters, &units, &[], WhiteSpaceMode::BreakSpaces,
        TextWrapMode::NoWrap, 8.0, &HashMap::from([(1, 0.5)]), 2.0, 3.0);
    assert_eq!(4.0, measurement.measure(IntRange::new(0, 2), LineEndReason::ParagraphEnd, 30.0, &[]).adjusted);
    assert_eq!(7.0, measurement.measure(IntRange::new(1, 2), LineEndReason::ParagraphEnd, 30.0, &[]).adjusted);
}

#[test]
fn phase_three_dp_promotes_only_after_tab_compression_is_feasible() {
    use tiqian::layout::progressive_break_decisions::{ProgressiveBreakOpportunity, ProgressiveBreakTier};
    for (capacity, promoted) in [(0.1, false), (0.5, true)] {
        let (mut clusters, mut config) = prepared("a\tbcd", WhiteSpaceMode::BreakSpaces, TextWrapMode::Wrap, 8.0, 0.0);
        clusters[0].advance = 7.75;
        let units = scan_white_space(&Text::from("a\tbcd"), WhiteSpaceMode::BreakSpaces, &HashMap::new()).into_iter().map(|unit| (unit.range, unit)).collect();
        config.measurement = Some(Arc::new(LineMeasurement::new(&clusters, &clusters, &units, &[], WhiteSpaceMode::BreakSpaces,
            TextWrapMode::Wrap, 8.0, &HashMap::from([(1, 0.5)]), 0.0, 0.0)));
        config.line_adjustment_push_in = true;
        config.shrink_opportunities = vec![ShrinkOpportunity::new(0, 2, capacity, ShrinkChannel::RawAdvance)];
        config.max_cjk_stretch_per_gap = 1.0;
        config.progressive_break_opportunities = HashMap::from([
            (1, ProgressiveBreakOpportunity::new(ProgressiveBreakTier::Emergency, text_range(0, 5))),
            (3, ProgressiveBreakOpportunity::new(ProgressiveBreakTier::Whitespace, text_range(0, 5))),
        ]);
        let solution = ParagraphDpLineBreaker::default().break_lines(&clusters, &clusters, 9.0, &config);
        assert_eq!(promoted, solution.lines[0].cluster_range.last() == 2, "capacity={capacity}");
        assert!(solution.lines.iter().all(|line| line.adjusted_width <= 9.0));
    }
}

#[test]
fn phase_three_conditional_hanging_compression_matches_exhaustive_samples() {
    use tiqian::layout::line_optimization::PushInAllocation;
    for text in ["ab  ", "ab  \n", "a\tb \t", "a\tb \t\n", "   ", "\t\t\n"] {
        for interval in [0.0, 0.25, 1.0, 8.0] {
            let (clusters, config) = prepared(text, WhiteSpaceMode::Preserve, TextWrapMode::Wrap, interval, 0.0);
            let measurement = config.measurement.as_ref().unwrap();
            let range = IntRange::new(0, clusters.len() as i32 - 1);
            for reason in [LineEndReason::AutoWrap, LineEndReason::MandatoryBreak, LineEndReason::ParagraphEnd] {
                let opportunities: Vec<_> = clusters.iter().enumerate().filter(|(_, cluster)| cluster.advance > 0.0)
                    .map(|(index, cluster)| ShrinkOpportunity::new(index as i32, 2, cluster.advance * 0.5, ShrinkChannel::RawAdvance)).collect();
                let effective: Vec<_> = opportunities.iter().copied()
                    .filter(|opp| measurement.shrink_is_effective(range, reason, opp.cluster_index)).collect();
                for step in 1..40 {
                    let limit = step as f32 * 0.125;
                    // 同一 tier 按容量比例穷举，并包含被过滤的尾空白，检查过滤是否损失可行解。
                    let sampled = (0..=256).any(|factor| {
                        let allocations: Vec<_> = opportunities.iter().map(|opp| PushInAllocation {
                            cluster_index: opp.cluster_index, shrink: opp.capacity * factor as f32 / 256.0,
                            available_capacity: opp.capacity, channel: opp.channel,
                        }).collect();
                        measurement.measure(range, reason, limit, &allocations).adjusted <= limit
                    });
                    let solution = measurement.compression(range, reason, limit, &effective);
                    assert!(!sampled || solution.is_some(), "{text:?} interval={interval} reason={reason:?} limit={limit}");
                    if let Some(allocations) = solution {
                        let width = measurement.measure(range, reason, limit, &allocations).adjusted;
                        assert!(width <= limit);
                        let records = measurement.adjustments(range, reason, limit, &allocations);
                        let occupied = clusters.iter().map(|cluster| cluster.advance).sum::<f32>()
                            - allocations.iter().map(|allocation| allocation.shrink).sum::<f32>()
                            + records.iter().filter(|record| record.action == LineWhiteSpaceAction::ResolveTab)
                                .map(|record| record.occupied_advance).sum::<f32>();
                        assert!((occupied - records.iter().map(|record| record.hanging_advance).sum::<f32>() - width).abs() < 0.0001);
                    }
                }
            }
        }
    }
}

struct PhaseThreeAutoSpace(tiqian::clreq::clreq_profile::AutoSpaceMode);

impl tiqian::clreq::clreq_profile::ClreqProfileResolver for PhaseThreeAutoSpace {
    fn resolve(&self, _: &tiqian::core::text_model::LayoutProfileId) -> tiqian::clreq::clreq_profile::ClreqProfile {
        let mut profile = tiqian::clreq::clreq_profile::ClreqProfile::mainland_horizontal();
        profile.auto_space.cjk_latin = self.0;
        profile.auto_space.cjk_digit = self.0;
        profile
    }
}

#[test]
fn phase_three_progressive_engine_insert_and_replace_evidence() {
    use tiqian::api::ParagraphLayoutEngineBuilder;
    use tiqian::clreq::clreq_profile::AutoSpaceMode;
    use tiqian::core::geometry::LayoutConstraints;
    use tiqian::core::text_model::{LayoutInput, LineBreakPolicy, LineBreakSpan, LineLengthGrid, ParagraphStyle, TiqianTextContent};
    use tiqian::core::units::Ic;
    use tiqian::linebreak::hyphenation::{Hyphenator, NoHyphenator};
    struct TwoFourSix;
    impl Hyphenator for TwoFourSix {
        fn hyphenate(&self, _: &Text) -> Vec<i32> { vec![2, 4, 6] }
    }
    static TWO_FOUR_SIX: TwoFourSix = TwoFourSix;
    let long = "Swift 这边是我最有体感的。JSONDecoder 慢是个老问题，SR-6252[36] 那个 issue 里挖出的根因是底层走 NSJSONSerialization 再桥接回 Objective-C，swift_dynamicCast 吃掉大量时间。";
    for mode in [AutoSpaceMode::Insert, AutoSpaceMode::Replace] {
        for (text, width, ranges, hyphenator) in [
            ("中 ab/cdefghijk", 100.0, vec![text_range(2, 14)], &TWO_FOUR_SIX as &dyn Hyphenator),
            (long, 579.0, vec![text_range(16, 27), text_range(67, 86), text_range(104, 121)], &NoHyphenator as &dyn Hyphenator),
        ] {
            for breaker in breakers() {
            let strategy = breaker.strategy_name();
            let result = ParagraphLayoutEngineBuilder::new(Box::new(support::DeterministicStubFontBackend::default()))
                .clreq_profile_resolver(Box::new(PhaseThreeAutoSpace(mode)))
                .line_breaker(breaker).hyphenator(hyphenator).build().layout(
                    LayoutInput::builder(TiqianTextContent::builder(Text::from(text)).line_break_spans(ranges.iter().copied()
                        .map(|range| LineBreakSpan { range, policy: LineBreakPolicy::ProgressiveTechnical }).collect()).build(),
                        LayoutConstraints::with_defaults(width)).paragraph_style(ParagraphStyle::builder()
                            .first_line_indent(Some(Ic::ZERO)).line_length_grid(LineLengthGrid::with_enabled(false)).build()).build());
            let texts: Vec<String> = result.lines.iter().map(|line| result.clusters[line.cluster_range.first() as usize..=line.cluster_range.last() as usize]
                .iter().map(|cluster| cluster.text.as_str()).collect()).collect();
            if width == 100.0 {
                assert_eq!("中 ab/cd", texts[0]);
                assert!(result.debug.line_decisions[0].technical_break
            == Some(ProgressiveBreakTier::Syllable));
                assert_eq!(mode == AutoSpaceMode::Replace, result.debug.justification_decisions.iter()
                    .any(|decision| decision.line_range == result.lines[0].range));
            } else {
                let affected = texts.iter().find(|text| text.contains("Objective-C")).unwrap();
                // 长句的旧断点只约束 lookahead；其他策略同样必须保留源文本并限制技术串外拉伸。
                if strategy == "lookahead" {
                    assert_eq!(if mode == AutoSpaceMode::Replace { "erialization 再桥接回 Objective-C，swift_dy" }
                        else { "Serialization 再桥接回 Objective-C，swift_" }, affected);
                }
                assert_eq!(text, texts.concat());
                let index = texts.iter().position(|text| text == affected).unwrap();
                assert!(result.debug.justification_decisions.iter().filter(|decision| decision.line_range == result.lines[index].range)
                    .flat_map(|decision| &decision.allocations).filter(|allocation| allocation.kind == GlueKind::CjkInterChar)
                    .all(|allocation| allocation.delta <= 0.001));
                assert!(result.debug.break_opportunity_decisions.iter().any(|decision| decision.range == text_range(104, 121)
                    && decision.tier == Some(ProgressiveBreakTier::Emergency) && decision.reason == "CurrentLineTechnicalEmergencyBreak"));
            }
            }
        }
    }
}

#[test]
fn phase_three_conditional_hanging_with_body_trailing_gap_matches_samples() {
    use tiqian::core::layout_model::AutoSpaceDecisionInfo;
    use tiqian::layout::line_optimization::PushInAllocation;
    for text in ["ab  ", "ab  \n", "ab\t "] {
        let (clusters, _) = prepared(text, WhiteSpaceMode::Preserve, TextWrapMode::Wrap, 8.0, 0.0);
        let units = scan_white_space(&Text::from(text), WhiteSpaceMode::Preserve, &HashMap::new())
            .into_iter().map(|unit| (unit.range, unit)).collect();
        let decision = AutoSpaceDecisionInfo { cluster_range: text_range(1, 2), side: AutoSpaceSide::Trailing,
            boundary_role: AutoSpaceBoundaryRole::EastAsianSpacingWide, mode: AutoSpaceMode::Insert, characters_affected: 0,
            reduction_per_char: 0.0, total_reduction: -0.5, reason: "test" };
        let measurement = LineMeasurement::new(&clusters, &clusters, &units, &[decision], WhiteSpaceMode::Preserve,
            TextWrapMode::Wrap, 8.0, &HashMap::from([(2, 0.5)]), 0.0, 0.0);
        let range = IntRange::new(0, clusters.len() as i32 - 1);
        let opportunities = [ShrinkOpportunity::new(0, 2, 0.75, ShrinkChannel::RawAdvance)];
        for reason in [LineEndReason::AutoWrap, LineEndReason::MandatoryBreak, LineEndReason::ParagraphEnd] {
            for step in 1..24 {
                let limit = step as f32 * 0.125;
                let sampled = (0..=256).any(|factor| measurement.measure(range, reason, limit, &[PushInAllocation {
                    cluster_index: 0, shrink: 0.75 * factor as f32 / 256.0,
                    available_capacity: 0.75, channel: ShrinkChannel::RawAdvance,
                }]).adjusted <= limit);
                let solution = measurement.compression(range, reason, limit, &opportunities);
                assert!(!sampled || solution.is_some(), "{text:?} {reason:?} limit={limit}");
                if let Some(allocations) = solution {
                    assert!(measurement.measure(range, reason, limit, &allocations).adjusted <= limit);
                    if limit == 1.5 {
                        assert_eq!(0.5, allocations.iter().map(|allocation| allocation.shrink).sum::<f32>());
                    }
                }
            }
        }
    }
}