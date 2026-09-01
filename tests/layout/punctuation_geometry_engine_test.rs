use tiqian::clreq::clreq_profile::{
    AdjustmentStylePolicy, AutoSpacePolicy, ClreqProfile, ClreqProfileResolver, KinsokuLevel,
    KinsokuMode, LineEndPunctuationStyle, PunctuationWidthPolicy,
};
use tiqian::core::geometry::{LayoutConstraints, Rect, TextRange};
use tiqian::core::layout_queries::positioned_clusters;
use tiqian::core::layout_model::{Cluster, Glyph, GlyphRun};
use tiqian::core::text::Text;
use tiqian::core::text_model::{LayoutInput, LineLengthGrid, ParagraphStyle, TiqianTextContent};
use tiqian::core::units::Ic;
use tiqian::layout::paragraph_layout_engine::{
    ExplainableStubParagraphLayoutEngine, ParagraphLayoutEngine,
};
use tiqian::shaping::text_shaper::{
    ExplainableStubTextShaper, ShapingInput, ShapingResult, TextShaper,
};

struct TaiwanProfile;

impl ClreqProfileResolver for TaiwanProfile {
    fn resolve(&self, _: &tiqian::core::text_model::LayoutProfileId) -> ClreqProfile {
        ClreqProfile::taiwan_horizontal()
    }
}

struct LooseLineEndProfile;

impl ClreqProfileResolver for LooseLineEndProfile {
    fn resolve(&self, _: &tiqian::core::text_model::LayoutProfileId) -> ClreqProfile {
        let mut profile = ClreqProfile::mainland_horizontal();
        profile.adjustment = AdjustmentStylePolicy::builder()
            .line_end_punctuation(LineEndPunctuationStyle::AllowFullWidth)
            .build();
        profile
    }
}

struct GbFixedSeparatorProfile;

impl ClreqProfileResolver for GbFixedSeparatorProfile {
    fn resolve(&self, _: &tiqian::core::text_model::LayoutProfileId) -> ClreqProfile {
        let mut profile = ClreqProfile::mainland_horizontal();
        profile.punctuation_width = PunctuationWidthPolicy::with_gb_fixed_separators(true);
        profile
    }
}

struct FixedBasicProfile {
    adjustment: AdjustmentStylePolicy,
    auto_space: AutoSpacePolicy,
}

impl ClreqProfileResolver for FixedBasicProfile {
    fn resolve(&self, _: &tiqian::core::text_model::LayoutProfileId) -> ClreqProfile {
        let mut profile = ClreqProfile::mainland_horizontal();
        profile.kinsoku_mode = KinsokuMode::fixed(KinsokuLevel::Basic);
        profile.adjustment = self.adjustment;
        profile.auto_space = self.auto_space;
        profile
    }
}

fn fixed_basic_layout(
    text: &str,
    max_width: f32,
    adjustment: AdjustmentStylePolicy,
    auto_space: AutoSpacePolicy,
    grid: bool,
) -> tiqian::core::layout_model::LayoutResult {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.clreq_profile_resolver = Box::new(FixedBasicProfile { adjustment, auto_space });
    engine.layout(
        LayoutInput::builder(TiqianTextContent::new(Text::from(text)), LayoutConstraints::with_defaults(max_width))
            .paragraph_style(
                ParagraphStyle::builder()
                    .first_line_indent(Some(Ic::ZERO))
                    .line_length_grid(LineLengthGrid::with_enabled(grid))
                    .build(),
            )
            .build(),
    )
}

fn layout(text: &str) -> tiqian::core::layout_model::LayoutResult {
    ExplainableStubParagraphLayoutEngine::default().layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from(text)),
            LayoutConstraints::with_defaults(320.0),
        )
        .paragraph_style(
            ParagraphStyle::builder()
                .first_line_indent(Some(Ic::ZERO))
                .build(),
        )
        .build(),
    )
}

struct CenteredInkTextShaper;

impl TextShaper for CenteredInkTextShaper {
    fn shape(&self, input: &ShapingInput) -> ShapingResult {
        ShapingResult::new(
            vec![Cluster::with_display_text(
                input.range,
                input.text.slice_text(input.range),
                input.display_text.clone(),
                input.font_decision.candidate.key.clone(),
                16.0,
            )],
            vec![GlyphRun::new(
                input.range,
                input.font_decision.candidate.key.clone(),
                vec![Glyph::builder(7, input.range, 16.0)
                    .bounds(Some(Rect {
                        left: 9.0,
                        top: -2.0,
                        right: 11.0,
                        bottom: 2.0,
                    }))
                    .build()],
                16.0,
            )],
        )
    }
}

struct PushInCenteredCommaTextShaper;

impl TextShaper for PushInCenteredCommaTextShaper {
    fn shape(&self, input: &ShapingInput) -> ShapingResult {
        let clusters: Vec<_> = input
            .display_text
            .chars()
            .scan(input.range.start(), |start, character| {
                let end = *start + character.len_utf16() as i32;
                let range = TextRange::new(*start, end);
                *start = end;
                Some(Cluster::with_display_text(
                    range,
                    input.text.slice_text(range),
                    Text::from(character.to_string()),
                    input.font_decision.candidate.key.clone(),
                    16.0,
                ))
            })
            .collect();
        let glyphs = clusters
            .iter()
            .enumerate()
            .map(|(index, cluster)| {
                Glyph::builder(index as u32 + 1, cluster.range, 16.0)
                    .bounds(Some(if cluster.display_text == "，" {
                        Rect {
                            left: 5.0,
                            top: -2.0,
                            right: 11.0,
                            bottom: 2.0,
                        }
                    } else {
                        Rect {
                            left: 0.0,
                            top: -12.0,
                            right: 16.0,
                            bottom: 4.0,
                        }
                    }))
                    .build()
            })
            .collect();
        ShapingResult::new(
            clusters,
            vec![GlyphRun::new(
                input.range,
                input.font_decision.candidate.key.clone(),
                glyphs,
                input.display_text.chars().count() as f32 * 16.0,
            )],
        )
    }
}

struct HaltStopTextShaper;

impl TextShaper for HaltStopTextShaper {
    fn shape(&self, input: &ShapingInput) -> ShapingResult {
        let result = ExplainableStubTextShaper.shape(input);
        if input.display_text != "。" {
            return result;
        }
        ShapingResult::with_decisions(
            result.clusters,
            result
                .glyph_runs
                .into_iter()
                .map(|run| {
                    GlyphRun::new(
                        run.range,
                        run.font_key,
                        run.glyphs
                            .into_iter()
                            .map(|glyph| {
                                Glyph::builder(glyph.id, glyph.cluster_range, glyph.advance)
                                    .x(glyph.x)
                                    .y(glyph.y)
                                    .render_font_key(glyph.render_font_key)
                                    .bounds(glyph.bounds)
                                    .halt_advance(Some(7.0))
                                    .halt_placement_x(Some(0.0))
                                    .build()
                            })
                            .collect(),
                        run.advance,
                    )
                })
                .collect(),
            result.decisions,
        )
    }
}

#[test]
fn engine_records_profile_fallback_geometry_and_line_end_ledger() {
    let result = layout("你好。");
    let punctuation = result
        .debug
        .punctuation_decisions
        .iter()
        .find(|decision| decision.ch == '。')
        .unwrap();
    assert_eq!(8.0, punctuation.body_width);
    assert_eq!(0.0, punctuation.leading_glue_natural);
    assert_eq!(8.0, punctuation.trailing_glue_natural);
    assert_eq!(
        "ProfileGlueFallbackWithoutFontGeometry",
        punctuation.geometry_source
    );

    let geometry = result
        .debug
        .geometry_decisions
        .iter()
        .find(|decision| decision.source_text == "。")
        .unwrap();
    assert_eq!("PunctuationGeometryLedger", geometry.source);
    assert_eq!(8.0, geometry.trailing_glue_consumed);
    assert_eq!(8.0, geometry.resolved_advance);
    let trim = result
        .debug
        .line_edge_trim_decisions
        .iter()
        .find(|decision| decision.cluster_range == geometry.range)
        .unwrap();
    assert_eq!("trailing", trim.side);
    assert_eq!(8.0, trim.trim_amount);
    assert_eq!("LineEndHalfWidthPunctuation", trim.reason);
}

#[test]
fn records_ink_calibrated_punctuation_geometry_in_layout_debug() {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.text_shaper = Box::new(CenteredInkTextShaper);
    let result = engine.layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from("。")),
            LayoutConstraints::with_defaults(320.0),
        )
        .paragraph_style(ParagraphStyle::builder().first_line_indent(Some(Ic::ZERO)).build())
        .build(),
    );

    let punctuation = result.debug.punctuation_decisions.first().unwrap();
    assert_eq!(Some(Rect { left: 9.0, top: -2.0, right: 11.0, bottom: 2.0 }), punctuation.ink_bounds);
    assert_eq!(8.0, punctuation.body_width);
    assert_eq!(Some(8.0), punctuation.ink_containment_body_floor);
    assert!(!punctuation.ink_containment_applied);
    assert_eq!(4.0, punctuation.leading_glue_natural);
    assert_eq!(4.0, punctuation.trailing_glue_natural);
    assert_eq!("InkBoundsFittedBodyCompression", punctuation.geometry_source);

    let geometry = result.debug.geometry_decisions.first().unwrap();
    assert_eq!("InkBoundsFittedBodyCompression", geometry.reason);
    assert_eq!(8.0, geometry.body_width);
    assert_eq!(4.0, geometry.leading_glue_natural);
    assert_eq!(4.0, geometry.trailing_glue_natural);
    assert_eq!(4.0, geometry.leading_glue_consumed);
    assert_eq!(4.0, geometry.trailing_glue_consumed);
    assert_eq!(8.0, geometry.resolved_advance);

    let edge = result.debug.line_edge_trim_decisions.first().unwrap();
    assert_eq!("both", edge.side);
    assert_eq!(8.0, edge.trim_amount);
    assert_eq!("LineEndCenteredPunctuationPairedCompression", edge.reason);
}

#[test]
fn push_in_keeps_font_centered_punctuation_compression_paired() {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.text_shaper = Box::new(PushInCenteredCommaTextShaper);
    let result = engine.layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from("中文中文，中文")),
            LayoutConstraints::with_defaults(72.0),
        )
        .paragraph_style(
            ParagraphStyle::builder()
                .first_line_indent(Some(Ic::ZERO))
                .line_length_grid(LineLengthGrid::with_enabled(false))
                .build(),
        )
        .build(),
    );

    let comma = result
        .debug
        .geometry_decisions
        .iter()
        .find(|decision| decision.source_text == "，")
        .unwrap();
    assert_eq!(4.0, comma.leading_glue_consumed);
    assert_eq!(4.0, comma.trailing_glue_consumed);
    assert_eq!(8.0, comma.resolved_advance);
    let push_in = result
        .debug
        .line_decisions
        .iter()
        .filter_map(|decision| decision.repair_decision.as_ref())
        .find(|decision| decision.kind == "PushIn")
        .unwrap();
    assert_eq!(8.0, push_in.shrink);
    assert_eq!(TextRange::new(4, 5), push_in.push_in_allocations[0].cluster_range);
}

#[test]
fn records_punctuation_atoms_in_layout_debug() {
    let result = layout("你好，世界。——");

    let comma = result
        .debug
        .punctuation_decisions
        .iter()
        .find(|decision| decision.ch == '，')
        .unwrap();
    assert_eq!(TextRange::new(2, 3), comma.range);
    assert_eq!("PauseOrStop", comma.punctuation_class);
    assert_eq!(16.0, comma.advance);
    assert_eq!(8.0, comma.body_width);
    assert_eq!(0.0, comma.leading_glue_natural);
    assert_eq!(8.0, comma.trailing_glue_natural);
    assert_eq!("Leading", comma.anchor);
    assert_eq!(TextRange::new(5, 6), result.debug.punctuation_decisions.iter().find(|decision| decision.ch == '。').unwrap().range);
    let dash = result.debug.punctuation_decisions.iter().find(|decision| decision.ch == '⸺').unwrap();
    assert_eq!(TextRange::new(6, 8), dash.range);
    assert_eq!("Dash", dash.punctuation_class);
    assert_eq!(32.0, dash.advance);
    assert_eq!(3, result.debug.punctuation_decisions.len());
}

#[test]
fn line_start_lenticular_bracket_consumes_opening_glue() {
    let result = layout("【引用结束】");

    let opening = result.debug.punctuation_decisions.iter().find(|decision| decision.ch == '【').unwrap();
    assert_eq!("Opening", opening.punctuation_class);
    assert_eq!(8.0, opening.leading_glue_natural);
    let geometry = result.debug.geometry_decisions.iter().find(|decision| decision.source_text == "【").unwrap();
    assert_eq!(8.0, geometry.leading_glue_natural);
    assert_eq!(8.0, geometry.leading_glue_consumed);
    assert_eq!(8.0, geometry.resolved_advance);
    let positioned = positioned_clusters(&result);
    assert_eq!(0.0, positioned[0].left);
    assert_eq!(-8.0, positioned[0].draw_x);
}

#[test]
fn compresses_adjacent_cjk_single_quote_comma_sequence() {
    let result = layout("’，‘");

    assert!(result.debug.font_decisions.iter().all(|decision| decision.role == "CjkPunctuation"));
    assert_eq!(3, result.debug.punctuation_decisions.len());
    assert_eq!(2, result.debug.spacing_decisions.len());
    assert!(result.debug.spacing_decisions.iter().all(|decision| {
        decision.reason == "collapse-adjacent-punctuation-inner-glue" && decision.reduction == 8.0
    }));
    assert_eq!(32.0, result.lines[0].visual_width);
    assert_eq!(32.0, result.size.width);
    assert_eq!(vec![0.0, 8.0, 16.0], positioned_clusters(&result).iter().map(|cluster| cluster.draw_x).collect::<Vec<_>>());
}

#[test]
fn compresses_cjk_closing_before_ascii_point_mark_without_reclassifying_ascii() {
    let result = layout("中」,next");

    let closing = result.clusters.iter().find(|cluster| cluster.text == "」").unwrap();
    let comma_font = result.debug.font_decisions.iter().find(|decision| decision.range.start() == 2).unwrap();
    let spacing = result.debug.spacing_decisions.iter().find(|decision| {
        decision.reason == "collapse-cjk-closing-before-ascii-point-mark"
    }).unwrap();
    assert_eq!("LatinText", comma_font.role);
    assert_eq!(8.0, closing.advance);
    assert_eq!('」', spacing.left_char);
    assert_eq!(',', spacing.right_char);
    assert_eq!(8.0, spacing.reduction);
}

#[test]
fn halt_advance_from_shaper_drives_punctuation_body_end_to_end() {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.text_shaper = Box::new(HaltStopTextShaper);
    let result = engine.layout(
        LayoutInput::builder(TiqianTextContent::new(Text::from("中文。")), LayoutConstraints::with_defaults(320.0))
            .paragraph_style(ParagraphStyle::builder().first_line_indent(Some(Ic::ZERO)).build())
            .build(),
    );

    let stop = result.debug.punctuation_decisions.first().unwrap();
    assert_eq!(Some(7.0), stop.halt_advance);
    assert_eq!(7.0, stop.body_width);
    assert_eq!("FontHaltFittedBodyCompression", stop.geometry_source);
    assert_eq!(9.0, stop.trailing_glue_natural);
    assert_eq!(7.0, result.clusters.iter().find(|cluster| cluster.text == "。").unwrap().advance);
}

#[test]
fn loose_line_end_style_keeps_full_width_punctuation() {
    let mut loose_engine = ExplainableStubParagraphLayoutEngine::default();
    loose_engine.clreq_profile_resolver = Box::new(LooseLineEndProfile);
    let loose = loose_engine.layout(
        LayoutInput::builder(TiqianTextContent::new(Text::from("中文中文。")), LayoutConstraints::with_defaults(320.0))
            .paragraph_style(ParagraphStyle::builder().first_line_indent(Some(Ic::ZERO)).build())
            .build(),
    );
    assert_eq!(16.0, loose.clusters.iter().find(|cluster| cluster.text == "。").unwrap().advance);
    assert!(loose.debug.line_edge_trim_decisions.iter().all(|decision| decision.reason != "LineEndHalfWidthPunctuation"));

    let strict = layout("中文中文。");
    assert_eq!(8.0, strict.clusters.iter().find(|cluster| cluster.text == "。").unwrap().advance);
}

#[test]
fn gb_fixed_separators_are_half_width_and_unadjustable() {
    let default = layout("中·中文");
    assert_eq!(16.0, default.clusters.iter().find(|cluster| cluster.text == "·").unwrap().advance);

    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.clreq_profile_resolver = Box::new(GbFixedSeparatorProfile);
    let result = engine.layout(
        LayoutInput::builder(TiqianTextContent::new(Text::from("中文·中文")), LayoutConstraints::with_defaults(320.0))
            .paragraph_style(ParagraphStyle::builder().first_line_indent(Some(Ic::ZERO)).build())
            .build(),
    );
    let mid = result.debug.geometry_decisions.iter().find(|decision| decision.source_text == "·").unwrap();
    assert_eq!(mid.trailing_glue_natural, mid.trailing_glue_consumed);
    assert_eq!(mid.leading_glue_natural, mid.leading_glue_consumed);
    assert_eq!(8.0, mid.resolved_advance);
}

#[test]
fn push_in_drains_bracket_outer_glue_before_inline_comma() {
    let result = ExplainableStubParagraphLayoutEngine::default().layout(
        LayoutInput::builder(TiqianTextContent::new(Text::from("中（文）中，中文中。")), LayoutConstraints::with_defaults(144.0))
            .paragraph_style(ParagraphStyle::builder().first_line_indent(Some(Ic::ZERO)).build())
            .build(),
    );

    assert_eq!(1, result.lines.len());
    let geometry = |text| result.debug.geometry_decisions.iter().find(|decision| decision.source_text == text).unwrap();
    assert_eq!(8.0, geometry("。").trailing_glue_consumed);
    assert_eq!(4.0, geometry("（").leading_glue_consumed);
    assert_eq!(4.0, geometry("）").trailing_glue_consumed);
    assert_eq!(0.0, geometry("，").trailing_glue_consumed);
}

#[test]
fn inline_stop_compression_knob_limits_push_in_capacity() {
    let text = "中中中。中中。";
    let default = fixed_basic_layout(text, 96.0, AdjustmentStylePolicy::default(), AutoSpacePolicy::default(), true);
    assert_eq!(1, default.lines.len());
    assert_eq!(Some("PushIn"), default.debug.line_decisions[0].repair.as_deref());

    let no_inline = fixed_basic_layout(
        text,
        96.0,
        AdjustmentStylePolicy::builder().allow_inline_stop_compression(false).build(),
        AutoSpacePolicy::default(),
        true,
    );
    assert!(no_inline.lines.len() > 1);
    let push_in = no_inline
        .debug
        .line_decisions
        .iter()
        .flat_map(|decision| &decision.repair_candidates)
        .find(|candidate| candidate.kind == "PushIn")
        .unwrap();
    assert_eq!(Some("insufficient-capacity"), push_in.rejection_reason.as_deref());
    assert_eq!(8.0, push_in.available_capacity);
}

#[test]
fn sino_western_gap_shrink_floors_at_eighth_em() {
    let result = fixed_basic_layout(
        "中文 AB 中。",
        88.0,
        AdjustmentStylePolicy::default(),
        AutoSpacePolicy::clreq(),
        false,
    );

    assert_eq!(2, result.lines.len());
    assert_eq!(Some("CarryPrevious"), result.debug.line_decisions[1].repair.as_deref());
    let push_in = result.debug.line_decisions[1]
        .repair_candidates
        .iter()
        .find(|candidate| candidate.kind == "PushIn")
        .unwrap();
    assert!(!push_in.accepted);
    assert_eq!(Some("insufficient-capacity"), push_in.rejection_reason.as_deref());
    assert_eq!(12.0, push_in.available_capacity);
}

#[test]
fn taiwan_profile_centres_pause_stop_glue_and_trims_both_sides() {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.clreq_profile_resolver = Box::new(TaiwanProfile);
    let result = engine.layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from("你好。")),
            LayoutConstraints::with_defaults(320.0),
        )
        .paragraph_style(
            ParagraphStyle::builder()
                .first_line_indent(Some(Ic::ZERO))
                .build(),
        )
        .build(),
    );

    let punctuation = result
        .debug
        .punctuation_decisions
        .iter()
        .find(|decision| decision.ch == '。')
        .unwrap();
    assert_eq!(4.0, punctuation.leading_glue_natural);
    assert_eq!(4.0, punctuation.trailing_glue_natural);
    assert_eq!("Center", punctuation.anchor.as_str());
    let geometry = result
        .debug
        .geometry_decisions
        .iter()
        .find(|decision| decision.source_text == "。")
        .unwrap();
    assert_eq!(4.0, geometry.leading_glue_consumed);
    assert_eq!(4.0, geometry.trailing_glue_consumed);
    assert_eq!(8.0, geometry.resolved_advance);
    let trim = result.debug.line_edge_trim_decisions.first().unwrap();
    assert_eq!("both", trim.side);
    assert_eq!("LineEndCenteredPunctuationPairedCompression", trim.reason);
}

#[test]
fn adjacent_closing_and_pause_stop_compression_is_reflected_in_drawable_ledger() {
    let result = layout("你好」。");

    assert_eq!(48.0, result.lines[0].visual_width);
    assert_eq!(48.0, result.size.width);
    assert_eq!(
        8.0,
        result
            .clusters
            .iter()
            .find(|cluster| cluster.text == "。")
            .unwrap()
            .advance
    );
    let spacing = result.debug.spacing_decisions.first().unwrap();
    assert_eq!('」', spacing.left_char);
    assert_eq!('。', spacing.right_char);
    assert_eq!(8.0, spacing.reduction);
    assert_eq!("collapse-adjacent-punctuation-inner-glue", spacing.reason);
}

#[test]
fn push_in_consumes_punctuation_glue_before_carrying_line_start_stop() {
    let result = ExplainableStubParagraphLayoutEngine::default().layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from("中文中。")),
            LayoutConstraints::with_defaults(60.0),
        )
        .paragraph_style(
            ParagraphStyle::builder()
                .first_line_indent(Some(Ic::ZERO))
                .line_length_grid(LineLengthGrid::with_enabled(false))
                .build(),
        )
        .build(),
    );

    assert_eq!(1, result.lines.len());
    assert_eq!(
        Some("PushIn"),
        result.debug.line_decisions[0].repair.as_deref()
    );
    let geometry = result
        .debug
        .geometry_decisions
        .iter()
        .find(|decision| decision.source_text == "。")
        .unwrap();
    assert_eq!(8.0, geometry.trailing_glue_consumed);
    assert_eq!(8.0, geometry.resolved_advance);
}
