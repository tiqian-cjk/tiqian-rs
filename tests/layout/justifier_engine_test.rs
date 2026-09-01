use tiqian::clreq::clreq_profile::{
    AdjustmentStylePolicy, ClreqProfile, ClreqProfileResolver, LineAdjustmentStrategy,
};
use tiqian::core::geometry::LayoutConstraints;
use tiqian::core::layout_model::{Cluster, Glyph, GlyphRun};
use tiqian::core::text::Text;
use tiqian::core::text_model::{
    LastLineAlignment, LayoutInput, LineLengthGrid, ParagraphStyle, TiqianTextContent,
};
use tiqian::core::units::Ic;
use tiqian::layout::paragraph_layout_engine::{
    ExplainableStubParagraphLayoutEngine, ParagraphLayoutEngine,
};
use tiqian::linebreak::hyphenation::NoHyphenator;
use tiqian::shaping::text_shaper::{ShapingInput, ShapingResult, TextShaper};

struct PushOutOnlyProfile;

impl ClreqProfileResolver for PushOutOnlyProfile {
    fn resolve(&self, _: &tiqian::core::text_model::LayoutProfileId) -> ClreqProfile {
        let mut profile = ClreqProfile::mainland_horizontal();
        profile.adjustment.line_adjustment = LineAdjustmentStrategy::PushOutOnly;
        profile
    }
}

struct FixedSinoWesternGapProfile;

impl ClreqProfileResolver for FixedSinoWesternGapProfile {
    fn resolve(&self, _: &tiqian::core::text_model::LayoutProfileId) -> ClreqProfile {
        let mut profile = ClreqProfile::mainland_horizontal();
        profile.adjustment = AdjustmentStylePolicy::builder()
            .allow_sino_western_gap_adjustment(false)
            .build();
        profile
    }
}

fn engine() -> ExplainableStubParagraphLayoutEngine {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.clreq_profile_resolver = Box::new(PushOutOnlyProfile);
    engine
}

fn layout(
    text: &str,
    max_width: f32,
    style: ParagraphStyle,
) -> tiqian::core::layout_model::LayoutResult {
    engine().layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from(text)),
            LayoutConstraints::with_defaults(max_width),
        )
        .paragraph_style(style)
        .build(),
    )
}

fn exact_measure_style() -> ParagraphStyle {
    ParagraphStyle::builder()
        .first_line_indent(Some(Ic::ZERO))
        .line_length_grid(LineLengthGrid::with_enabled(false))
        .build()
}

struct PositionedPairShaper;

impl TextShaper for PositionedPairShaper {
    fn shape(&self, input: &ShapingInput) -> ShapingResult {
        let text = input.text.slice_text(input.range);
        let advance = if text == "AV" { 10.0 } else { text.utf16_len() as f32 * 16.0 };
        let glyphs = if text == "AV" {
            vec![
                Glyph::builder(1, input.range, 5.0).x(0.0).build(),
                Glyph::builder(2, input.range, 5.0).x(5.0).build(),
            ]
        } else {
            vec![Glyph::builder(3, input.range, advance).x(0.0).build()]
        };
        ShapingResult::new(
            vec![Cluster::with_display_text(
                input.range,
                text,
                input.display_text.clone(),
                input.font_decision.candidate.key.clone(),
                advance,
            )],
            vec![GlyphRun::new(
                input.range,
                input.font_decision.candidate.key.clone(),
                glyphs,
                advance,
            )],
        )
    }
}

#[test]
fn connector_boundaries_remain_closed_during_justification() {
    let result = layout(
        "中～文中Example",
        80.0,
        ParagraphStyle::builder()
            .first_line_indent(Some(Ic::ZERO))
            .build(),
    );

    assert!(result.lines.len() >= 2);
    let decision = &result.debug.justification_decisions[0];
    assert_eq!(0.0, decision.deficit_after);
    let inter_char: Vec<_> = decision
        .allocations
        .iter()
        .filter(|allocation| allocation.kind == "CjkInterChar")
        .collect();
    assert_eq!(
        vec![2],
        inter_char
            .iter()
            .map(|allocation| allocation.cluster_range.start())
            .collect::<Vec<_>>()
    );
    assert_eq!(16.0, inter_char[0].delta);
}

#[test]
fn inseparable_number_and_unit_boundary_remains_closed_during_justification() {
    let text = "中文50℃中文中文中文Example";
    let result = layout(text, 128.0, exact_measure_style());
    let number = result
        .clusters
        .iter()
        .find(|cluster| cluster.range.start() <= 2 && cluster.range.end() >= 4)
        .unwrap();
    let unit = result
        .clusters
        .iter()
        .find(|cluster| cluster.range.start() <= 4 && cluster.range.end() >= 5)
        .unwrap();
    assert_ne!(number.range, unit.range);
    let decision = result
        .debug
        .justification_decisions
        .iter()
        .find(|decision| {
            number.range.start() >= decision.line_range.start()
                && unit.range.end() <= decision.line_range.end()
                && !decision.allocations.is_empty()
        })
        .expect("expected a justified line containing 50℃");

    assert!(decision.allocations.iter().all(|allocation| {
        allocation.cluster_range != number.range
            || (allocation.kind != "CjkLatinSpace" && allocation.kind != "CjkInterChar")
    }));
    assert_eq!(0.0, decision.deficit_after);
}

#[test]
fn last_line_is_never_justified() {
    let result = layout(
        "中文中",
        80.0,
        ParagraphStyle::builder()
            .first_line_indent(Some(Ic::ZERO))
            .build(),
    );

    assert_eq!(1, result.lines.len());
    let line = &result.lines[0];
    assert_eq!(48.0, line.adjusted_width);
    assert_eq!(48.0, line.visual_width);
    assert!(result.debug.justification_decisions.is_empty());
}

#[test]
fn mandatory_and_paragraph_end_lines_take_last_line_alignment() {
    let style = ParagraphStyle::builder()
        .first_line_indent(Some(Ic::ZERO))
        .last_line_alignment(LastLineAlignment::Center)
        .line_length_grid(LineLengthGrid::with_enabled(false))
        .build();
    let result = layout("中文中\n中文中文中文中", 100.0, style);

    assert_eq!(3, result.lines.len());
    assert_eq!(26.0, result.lines[0].indent);
    assert_eq!(0.0, result.lines[1].indent);
    assert_eq!(42.0, result.lines[2].indent);
}

#[test]
fn last_line_alignment_positions_the_last_line_via_indent() {
    let layout_with = |alignment| {
        layout(
            "中文中文中文中文中",
            100.0,
            ParagraphStyle::builder()
                .first_line_indent(Some(Ic::ZERO))
                .last_line_alignment(alignment)
                .line_length_grid(LineLengthGrid::with_enabled(false))
                .build(),
        )
    };

    let start = layout_with(LastLineAlignment::Start);
    assert_eq!(100.0, start.lines[0].visual_width);
    assert_eq!(0.0, start.lines[1].indent);
    let center = layout_with(LastLineAlignment::Center);
    assert_eq!(26.0, center.lines[1].indent);
    assert_eq!(0.0, center.lines[0].indent);
    let end = layout_with(LastLineAlignment::End);
    assert_eq!(52.0, end.lines[1].indent);
}

#[test]
fn sino_western_gap_knob_disables_stretch_and_shrink() {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.clreq_profile_resolver = Box::new(FixedSinoWesternGapProfile);
    let result = engine.layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from("中文Hello文中文中文中文中")),
            LayoutConstraints::with_defaults(160.0),
        )
        .paragraph_style(ParagraphStyle::builder().first_line_indent(Some(Ic::ZERO)).build())
        .build(),
    );

    assert!(!result.debug.justification_decisions.is_empty());
    assert!(result
        .debug
        .justification_decisions
        .iter()
        .flat_map(|decision| &decision.allocations)
        .all(|allocation| allocation.kind != "CjkLatinSpace"));
}

#[test]
fn half_em_word_spaces_do_not_stretch_under_justification() {
    let result = layout(
        "AB CD EF中文中文中",
        160.0,
        ParagraphStyle::builder().first_line_indent(Some(Ic::ZERO)).build(),
    );

    assert!(result.lines.len() >= 2);
    let decision = &result.debug.justification_decisions[0];
    assert_eq!(0.0, decision.deficit_after);
    assert!(decision.allocations.iter().all(|allocation| allocation.kind != "WordSpace"));
    assert!(!decision.allocations.is_empty());
    assert_eq!(160.0, result.lines[0].visual_width);
}

#[test]
fn justify_fills_saturated_line_with_uncapped_even_share() {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.hyphenator = &NoHyphenator;
    let result = engine.layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from("中文中文Network中文")),
            LayoutConstraints::with_defaults(160.0),
        )
        .paragraph_style(ParagraphStyle::builder().first_line_indent(Some(Ic::ZERO)).build())
        .build(),
    );

    let decision = result
        .debug
        .justification_decisions
        .iter()
        .find(|decision| decision.line_range.start() == 0)
        .unwrap();
    assert_eq!(0.0, decision.deficit_after);
    assert_eq!(160.0, result.lines[0].visual_width);
    let deltas: Vec<_> = decision
        .allocations
        .iter()
        .filter(|allocation| allocation.kind == "CjkInterChar")
        .map(|allocation| allocation.delta)
        .collect();
    assert_eq!(3, deltas.len());
    assert!(deltas.iter().all(|delta| (*delta - 32.0).abs() < 0.01));
}

#[test]
fn justifies_non_last_line_using_cjk_inter_char_gaps_as_last_resort() {
    let result = layout(
        "中文中文中文",
        80.0,
        ParagraphStyle::builder().first_line_indent(Some(Ic::ZERO)).build(),
    );

    assert_eq!(2, result.lines.len());
    assert_eq!(80.0, result.lines[0].adjusted_width);
    assert_eq!(80.0, result.lines[0].visual_width);
    assert_eq!(16.0, result.lines[1].adjusted_width);
    assert_eq!(16.0, result.lines[1].visual_width);
    assert!(result.debug.justification_decisions.is_empty());
}

#[test]
fn uses_punctuation_glue_first_when_deficit_matches_compression() {
    let result = layout(
        "中，。文",
        64.0,
        ParagraphStyle::builder().first_line_indent(Some(Ic::ZERO)).build(),
    );

    assert_eq!(1, result.lines.len());
    assert!(result.debug.justification_decisions.is_empty());
}

#[test]
fn justify_distributes_deficit_across_priority_chain() {
    let result = layout(
        "中」。文中文中文中",
        80.0,
        ParagraphStyle::builder().first_line_indent(Some(Ic::ZERO)).build(),
    );

    assert!(result.lines.len() >= 2);
    let decision = &result.debug.justification_decisions[0];
    assert_eq!(8.0, decision.deficit_before);
    assert_eq!(0.0, decision.deficit_after);
    assert_eq!(4, decision.allocations.len());
    assert!(decision.allocations.iter().all(|allocation| allocation.kind == "CjkInterChar"));
    assert!(decision.allocations.iter().all(|allocation| allocation.delta == 2.0));
    let mut targets: Vec<_> = decision
        .allocations
        .iter()
        .map(|allocation| allocation.cluster_range.start())
        .collect();
    targets.sort();
    assert_eq!(vec![0, 1, 2, 3], targets);
    assert_eq!(80.0, result.lines[0].visual_width);
    let geometry = result
        .debug
        .geometry_decisions
        .iter()
        .find(|decision| decision.source_text == "」")
        .expect("expected closing punctuation geometry");
    assert_eq!(8.0, geometry.trailing_glue_consumed);
    assert_eq!(2.0, geometry.justification_delta);
    assert_eq!(10.0, geometry.resolved_advance);
}

#[test]
fn cjk_inter_char_acts_as_last_resort_when_punct_glue_exhausted() {
    let result = layout(
        "中文中文中文中文中文中文",
        100.0,
        exact_measure_style(),
    );

    assert_eq!(2, result.lines.len());
    let decision = result
        .debug
        .justification_decisions
        .first()
        .expect("expected first-line justification decision");
    assert_eq!(4.0, decision.deficit_before);
    assert_eq!(0.0, decision.deficit_after);
    assert!(decision.allocations.iter().all(|allocation| allocation.kind == "CjkInterChar"));
    assert_eq!(5, decision.allocations.len());
    assert!(decision
        .allocations
        .iter()
        .all(|allocation| (allocation.delta - 0.8).abs() < 0.001));
    assert_eq!(100.0, result.lines[0].visual_width);
}

#[test]
fn uniform_tracking_includes_bracket_inner_sides() {
    let result = layout(
        "中（中文）文中文中文中",
        100.0,
        exact_measure_style(),
    );

    assert_eq!(2, result.lines.len());
    let decision = result
        .debug
        .justification_decisions
        .first()
        .expect("expected first-line justification decision");
    assert_eq!(0.0, decision.deficit_after);
    let mut targets: Vec<_> = decision
        .allocations
        .iter()
        .map(|allocation| allocation.cluster_range.start())
        .collect();
    targets.sort();
    assert_eq!(vec![0, 1, 2, 3, 4], targets);
    assert!(decision.allocations.iter().all(|allocation| allocation.kind == "CjkInterChar"));
    assert!(decision
        .allocations
        .iter()
        .all(|allocation| (allocation.delta - 0.8).abs() < 0.01));
}

#[test]
fn bracket_western_interior_stretches_in_tier_three_not_tier_two() {
    let result = layout(
        "中文（Hello）中文中文",
        170.0,
        exact_measure_style(),
    );

    assert_eq!(2, result.lines.len());
    let decision = result
        .debug
        .justification_decisions
        .first()
        .expect("expected first-line justification decision");
    assert!(decision
        .allocations
        .iter()
        .all(|allocation| allocation.kind != "CjkLatinSpace"));
    assert!(decision.allocations.iter().any(|allocation| {
        allocation.kind == "CjkInterChar" && allocation.cluster_range.start() == 2
    }));
    assert!(decision.allocations.iter().any(|allocation| {
        allocation.kind == "CjkInterChar" && allocation.cluster_range.start() == 3
    }));
}

#[test]
fn latin_glyph_positions_survive_autospace_and_justification() {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.text_shaper = Box::new(PositionedPairShaper);
    engine.hyphenator = &NoHyphenator;
    engine.clreq_profile_resolver = Box::new(PushOutOnlyProfile);
    let result = engine.layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from("中AV中文")),
            LayoutConstraints::with_defaults(52.0),
        )
        .paragraph_style(exact_measure_style())
        .build(),
    );

    let latin = result
        .clusters
        .iter()
        .find(|cluster| cluster.text == "AV")
        .expect("expected AV cluster");
    assert!(latin.advance > 10.0, "autospace/justification should widen the cluster as trailing layout space: {latin:?}");
    assert!(result
        .debug
        .justification_decisions
        .iter()
        .flat_map(|decision| &decision.allocations)
        .any(|allocation| allocation.cluster_range == latin.range && allocation.kind == "CjkLatinSpace"));
    let glyphs: Vec<_> = result
        .glyph_runs
        .iter()
        .flat_map(|run| &run.glyphs)
        .filter(|glyph| glyph.cluster_range == latin.range)
        .collect();
    assert_eq!(vec![0.0, 5.0], glyphs.iter().map(|glyph| glyph.x).collect::<Vec<_>>());
    assert_eq!(vec![5.0, 5.0], glyphs.iter().map(|glyph| glyph.advance).collect::<Vec<_>>());
}

#[test]
fn dash_boundaries_do_not_receive_uniform_tracking() {
    let result = layout(
        "在所谓中文语境下——不如说中文",
        180.0,
        exact_measure_style(),
    );

    assert!(result.lines.len() >= 2);
    let dash_index = result
        .clusters
        .iter()
        .position(|cluster| cluster.text == "——")
        .expect("expected dash cluster");
    let dash = &result.clusters[dash_index];
    let before = &result.clusters[dash_index - 1];
    let allocations = &result.debug.justification_decisions[0].allocations;
    assert!(allocations.iter().all(|allocation| {
        allocation.kind != "CjkInterChar" || allocation.cluster_range != before.range
    }));
    assert!(allocations.iter().all(|allocation| {
        allocation.kind != "CjkInterChar" || allocation.cluster_range != dash.range
    }));
}

#[test]
fn typed_sino_western_spaces_stretch_in_tier_two() {
    let result = layout(
        "中文 Hello 中文中文中文",
        180.0,
        ParagraphStyle::builder().first_line_indent(Some(Ic::ZERO)).build(),
    );

    assert!(result.lines.len() >= 2);
    let decision = &result.debug.justification_decisions[0];
    let sino: Vec<_> = decision
        .allocations
        .iter()
        .filter(|allocation| allocation.kind == "CjkLatinSpace")
        .collect();
    assert_eq!(2, sino.len());
    assert!(sino.iter().all(|allocation| {
        result
            .clusters
            .iter()
            .find(|cluster| cluster.range.start() == allocation.cluster_range.start())
            .is_some_and(|cluster| cluster.text == " ")
    }));
    assert!(sino.iter().all(|allocation| allocation.delta == sino[0].delta));
}

#[test]
fn punctuation_to_western_boundary_stretches_in_tier_three() {
    let result = layout(
        "你好「World」你好你好你",
        140.0,
        exact_measure_style(),
    );

    let allocations = &result.debug.justification_decisions[0].allocations;
    assert!(allocations.iter().any(|allocation| {
        allocation.kind == "CjkInterChar"
            && result
                .clusters
                .iter()
                .find(|cluster| cluster.range.start() == allocation.cluster_range.start())
                .is_some_and(|cluster| cluster.text == "「")
    }));
}

#[test]
fn line_edge_sino_western_space_stays_collapsed() {
    let result = layout(
        "中文中文 word 中文中",
        80.0,
        ParagraphStyle::builder().first_line_indent(Some(Ic::ZERO)).build(),
    );

    for line in result.lines.iter().take(result.lines.len().saturating_sub(1)) {
        let edge = result
            .clusters
            .iter()
            .rev()
            .find(|cluster| cluster.range.start() < line.range.end())
            .expect("non-last line has edge cluster");
        if edge.text == " " {
            assert_eq!(0.0, edge.advance, "line-edge sino-western space must stay collapsed");
        }
    }
}
