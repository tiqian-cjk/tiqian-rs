use tiqian::core::geometry::{text_range, LayoutConstraints};
use tiqian::core::int_range::IntRange;
use tiqian::core::text::Text;
use tiqian::core::text_model::{
    INLINE_OBJECT_REPLACEMENT_CHAR, InlineAttachment, InlineObjectBoundaryAdjustment,
    InlineObjectSpan, LayoutInput, LineBreakPolicy, LineBreakSpan, LineLengthGrid,
    ParagraphStyle, TextSpan, TextStyle, TiqianTextContent,
};
use tiqian::core::units::Ic;
use tiqian::clreq::clreq_profile::{ClreqProfile, ClreqProfileResolver};
use tiqian::layout::paragraph_layout_engine::{
    ExplainableStubParagraphLayoutEngine, ParagraphLayoutEngine,
};
use tiqian::linebreak::english_hyphenation::english_hyphenation;
use tiqian::shaping::text_shaper::{ExplainableStubTextShaper, ShapingInput, ShapingResult, TextShaper};

fn layout(text: &str, max_width: f32, hyphenate: bool) -> tiqian::core::layout_model::LayoutResult {
    layout_with_content(text, max_width, hyphenate, Vec::new(), Vec::new())
}

fn layout_with_content(
    text: &str,
    max_width: f32,
    hyphenate: bool,
    spans: Vec<TextSpan>,
    inline_objects: Vec<InlineObjectSpan>,
) -> tiqian::core::layout_model::LayoutResult {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    if hyphenate {
        engine.hyphenator = english_hyphenation::en_us();
    }
    engine.layout(
        LayoutInput::builder(
            TiqianTextContent::builder(Text::from(text)).spans(spans).build(),
            LayoutConstraints::with_defaults(max_width),
        )
        .paragraph_style(
            ParagraphStyle::builder()
                .first_line_indent(Some(Ic::ZERO))
                .line_length_grid(LineLengthGrid::with_enabled(false))
                .build(),
        )
            .inline_objects(inline_objects)
        .build(),
    )
}

fn layout_with_spans(
    text: &str,
    max_width: f32,
    line_break_spans: Vec<LineBreakSpan>,
    text_shaper: Option<Box<dyn TextShaper>>,
) -> tiqian::core::layout_model::LayoutResult {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    if let Some(text_shaper) = text_shaper {
        engine.text_shaper = text_shaper;
    }
    engine.layout(
        LayoutInput::builder(
            TiqianTextContent::builder(Text::from(text))
                .line_break_spans(line_break_spans)
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

#[test]
fn empty_text_yields_zero_height_without_lines() {
    let result = layout("", 100.0, false);
    assert!(result.lines.is_empty(), "{:?}", result.lines);
    assert_eq!(0.0, result.size.height);
    assert_eq!(0.0, result.size.width);
}

#[test]
fn lone_mandatory_break_emits_two_zero_width_lines() {
    let result = layout("\n", 100.0, false);
    assert_eq!(2, result.lines.len(), "{:?}", result.lines);
    assert!(result.lines.iter().all(|line| line.natural_width == 0.0 && line.visual_width == 0.0));
    assert!(result.size.height > 0.0, "{}", result.size.height);
}

#[test]
fn mandatory_break_middle_line_skips_its_justification_plan() {
    let result = layout("中文中文\n中文中文", 80.0, false);
    assert_eq!(2, result.lines.len(), "{:?}", result.lines);
    assert_eq!(IntRange::new(0, 4), result.lines[0].cluster_range);
    assert_eq!(IntRange::new(5, 8), result.lines[1].cluster_range);
    assert!(result.lines.iter().all(|line| line.adjusted_width == line.natural_width), "{:?}", result.lines);
    assert!(result.debug.justification_decisions.is_empty());
}

#[test]
fn blank_middle_line_skips_every_edge_pass() {
    let result = layout("中文\n\n中文", 80.0, false);
    assert_eq!(3, result.lines.len(), "{:?}", result.lines);
    assert_eq!(IntRange::new(3, 3), result.lines[1].cluster_range, "{:?}", result.lines);
    assert_eq!(0.0, result.lines[1].natural_width);
    assert!(result.debug.justification_decisions.iter().all(|decision| decision.line_range != result.lines[1].range));
}

#[test]
fn trailing_mandatory_break_emits_terminal_empty_line_without_hyphen() {
    let result = layout("中文aa internationalization\n", 118.0, true);
    let last = result.lines.last().unwrap();
    assert!(last.cluster_range.is_empty(), "{:?}", result.lines);
    assert_eq!(0.0, last.hyphen_advance);
    let before = &result.lines[result.lines.len() - 2];
    assert_eq!(0.0, before.hyphen_advance, "{:?}", result.lines);
    assert!(before.hyphen_glyphs.is_empty());
}

#[test]
fn hyphen_squeeze_consumes_the_word_space_raw_advance_channel() {
    let result = layout("中文aa internationalization", 118.0, true);
    let space = result.clusters.iter().find(|cluster| cluster.text == " ").unwrap();
    assert_eq!(4.0, space.advance, "{:?}", result.clusters);
    let first = &result.lines[0];
    assert_eq!(16.0, first.hyphen_advance);
    assert!((first.adjusted_width + first.hyphen_advance - 118.0).abs() < 1e-9);
}

#[test]
fn hyphen_squeeze_consumes_opening_and_closing_bracket_glue_channels() {
    let result = layout("（中·文，internationalization", 112.0, true);
    let opening = result.clusters.iter().find(|cluster| cluster.text == "（").unwrap();
    let comma = result.clusters.iter().find(|cluster| cluster.text == "，").unwrap();
    assert_eq!(8.0, opening.advance, "{:?}", result.clusters);
    assert_eq!(8.0, comma.advance, "{:?}", result.clusters);
}

#[test]
fn hyphen_squeeze_consumes_the_interpunct_paired_channel() {
    let result = layout("中文，文internationalization", 112.0, true);
    let comma = result.clusters.iter().find(|cluster| cluster.text == "，").unwrap();
    assert_eq!(14.0, comma.advance, "{:?}", result.clusters);
}

#[test]
fn formula_line_end_discards_the_trailing_boundary_advance() {
    let text = format!("甲{INLINE_OBJECT_REPLACEMENT_CHAR}乙丙丁戊");
    let result = layout_with_content(
        &text,
        48.0,
        false,
        Vec::new(),
        vec![InlineObjectSpan::with_trailing_boundary(
            text_range(1, 2),
            24.0,
            12.0,
            12.0,
            InlineObjectBoundaryAdjustment::builder()
                .line_end_discardable_advance(6.0)
                .build(),
        )],
    );
    assert_eq!(IntRange::new(0, 1), result.lines[0].cluster_range, "{:?}", result.lines);
    let discard = result.debug.line_edge_trim_decisions.iter().find(|decision| {
        decision.reason == "InlineObjectLineEndDiscardableGlue"
    }).unwrap();
    assert_eq!(6.0, discard.trim_amount);
    assert_eq!(0.0, discard.consumed_before);
    assert_eq!("trailing", discard.side);
}

#[test]
fn attached_footnote_trailing_glue_trims_when_the_line_ends_at_the_run() {
    let text = "正文：“内容。”[1]后文";
    let result = layout_with_content(
        text,
        164.0,
        false,
        vec![TextSpan {
            range: text_range(8, 11),
            style: TextStyle::builder()
                .inline_attachment(InlineAttachment::Previous)
                .build(),
        }],
        Vec::new(),
    );
    assert_eq!(IntRange::new(0, 8), result.lines[0].cluster_range, "{:?}", result.lines);
    let trim = result.debug.line_edge_trim_decisions.iter().find(|decision| {
        decision.reason == "AttachedInlineVirtualBoundaryLineEndTrim"
    }).unwrap();
    assert_eq!(text_range(8, 11), trim.cluster_range);
    assert_eq!(8.0, trim.trim_amount);
    assert_eq!("trailing", trim.side);
}

#[test]
fn lone_latin_cluster_merges_both_auto_space_edge_trims_into_one_key() {
    let result = layout("中A中", 24.0, false);
    assert_eq!(IntRange::new(1, 1), result.lines[1].cluster_range, "{:?}", result.lines);
    let trims: Vec<_> = result.debug.line_edge_trim_decisions.iter()
        .filter(|decision| decision.reason == "TextAutoSpaceLineEdgeTrim")
        .collect();
    assert_eq!(vec!["trailing", "leading"], trims.iter().map(|decision| decision.side.as_str()).collect::<Vec<_>>());
    assert!(trims.iter().all(|decision| decision.cluster_range == text_range(1, 2) && decision.trim_amount == 2.0), "{trims:?}");
    assert_eq!(16.0, result.lines[1].adjusted_width);
}

#[test]
fn attached_object_mark_hangs_instead_of_leaving_the_separator_at_an_edge() {
    let text = format!("中{INLINE_OBJECT_REPLACEMENT_CHAR} ，中");
    let result = layout_with_content(
        &text,
        48.0,
        false,
        Vec::new(),
        vec![InlineObjectSpan::with_fixed_boundaries(
            text_range(1, 2),
            100.0,
            12.0,
            12.0,
        )],
    );
    let hung = result
        .lines
        .iter()
        .find(|line| line.hanging_punctuation_advance > 0.0);
    assert!(hung.is_some(), "lines={:?}, trims={:?}", result.lines, result.debug.line_edge_trim_decisions);
    let hung = hung.unwrap();
    assert_eq!(IntRange::new(1, 3), hung.cluster_range, "{:?}", result.lines);
    assert!(result.debug.line_edge_trim_decisions.iter().all(|decision| decision.reason != "LineEdgeWordSpaceCollapse"), "{:?}", result.debug.line_edge_trim_decisions);
}

struct ZeroSpaceTextShaper;

impl TextShaper for ZeroSpaceTextShaper {
    fn shape(&self, input: &ShapingInput) -> ShapingResult {
        let shaped = ExplainableStubTextShaper.shape(input);
        ShapingResult::with_decisions(
            shaped.clusters.into_iter().map(|cluster| {
                (cluster.text.chars().all(|character| character == ' ')).then_some(())
                    .map_or(cluster.clone(), |_| tiqian::core::layout_model::Cluster { advance: 0.0, ..cluster })
            }).collect(),
            shaped.glyph_runs,
            shaped.decisions,
        )
    }
}

#[test]
fn zero_advance_edge_space_is_never_collapsed() {
    let result = layout_with_spans(
        "中中中中 aaa bbb",
        114.0,
        Vec::new(),
        Some(Box::new(ZeroSpaceTextShaper)),
    );
    let first = &result.lines[0];
    let edge = &result.clusters[first.cluster_range.last() as usize];
    assert!(edge.text.chars().all(|character| character == ' '), "{:?}", result.clusters);
    assert_eq!(0.0, edge.advance);
    assert!(result.debug.line_edge_trim_decisions.iter().all(|decision| decision.reason != "LineEdgeWordSpaceCollapse"), "{:?}", result.debug.line_edge_trim_decisions);
}

#[test]
fn hyphen_squeeze_falls_back_to_zero_used_glue_when_the_line_already_fits() {
    let comma = layout("中文，internationalization", 88.0, true);
    assert_eq!(IntRange::new(0, 3), comma.lines[0].cluster_range, "{:?}", comma.lines);
    assert_eq!(16.0, comma.lines[0].hyphen_advance);
    assert_eq!(8.0, comma.clusters[2].advance, "{:?}", comma.clusters);

    let bracket = layout("（中文internationalization", 84.0, true);
    assert_eq!(IntRange::new(0, 3), bracket.lines[0].cluster_range, "{:?}", bracket.lines);
    assert_eq!(16.0, bracket.lines[0].hyphen_advance);
    assert!(bracket.clusters[0].advance <= 16.0, "{:?}", bracket.clusters);
}

#[test]
fn tiny_technical_tracking_stays_below_the_rejection_threshold() {
    let text = "中中中中中中 aaaa";
    let spans = vec![LineBreakSpan { range: text_range(0, Text::from(text).scalar_len().value()), policy: LineBreakPolicy::ProgressiveTechnical }];
    let tiny = layout_with_spans(text, 96.004, spans.clone(), None);
    assert_eq!(IntRange::new(0, 5), tiny.lines[0].cluster_range, "{:?}", tiny.lines);
    let deltas: Vec<_> = tiny.debug.justification_decisions.iter().flat_map(|decision| &decision.allocations)
        .filter(|allocation| allocation.kind == "CjkInterChar")
        .map(|allocation| allocation.delta)
        .collect();
    assert!(!deltas.is_empty(), "{:?}", tiny.debug.justification_decisions);
    assert!(deltas.iter().all(|delta| *delta <= 0.001), "{deltas:?}");
    assert!(tiny.debug.emergency_tracking_eligibility_decisions.iter().all(|decision| !decision.reason.starts_with("CurrentLineTechnicalTierRejection:")), "{:?}", tiny.debug.emergency_tracking_eligibility_decisions);

    let rejected = layout_with_spans(text, 96.4, spans, None);
    assert!(rejected.debug.emergency_tracking_eligibility_decisions.iter().any(|decision| decision.reason == "CurrentLineTechnicalTierRejection:WholeToken"), "{:?}", rejected.debug.emergency_tracking_eligibility_decisions);
}

#[test]
fn formula_object_without_boundary_discards_nothing_at_line_end() {
    let text = format!("甲{INLINE_OBJECT_REPLACEMENT_CHAR}乙丙丁戊");
    let result = layout_with_content(
        &text,
        48.0,
        false,
        Vec::new(),
        vec![InlineObjectSpan::with_fixed_boundaries(
            text_range(1, 2),
            24.0,
            12.0,
            12.0,
        )],
    );
    assert_eq!(IntRange::new(0, 1), result.lines[0].cluster_range, "{:?}", result.lines);
    assert!(result.debug.line_edge_trim_decisions.iter().all(|decision| decision.reason != "InlineObjectLineEndDiscardableGlue"), "{:?}", result.debug.line_edge_trim_decisions);
}

#[test]
fn baseline_shift_span_raises_the_final_cluster_shift() {
    let result = layout_with_content(
        "中文正文",
        200.0,
        false,
        vec![TextSpan {
            range: text_range(0, 2),
            style: TextStyle::builder().baseline_shift(4.0).build(),
        }],
        Vec::new(),
    );
    assert_eq!(4.0, result.clusters[0].baseline_shift);
    assert_eq!(4.0, result.clusters[1].baseline_shift);
    assert_eq!(0.0, result.clusters[2].baseline_shift);
}

#[test]
fn dash_run_without_ink_bounds_keeps_synthetic_glyphs() {
    let result = layout("中——中", 200.0, false);
    assert_eq!(1, result.lines.len(), "{:?}", result.lines);
    assert_eq!(1, result.glyph_runs.len());
    let run = &result.glyph_runs[0];
    assert_eq!(3, run.glyphs.len());
    assert!(run.glyphs.iter().all(|glyph| glyph.bounds.is_none()), "{:?}", run.glyphs);
    assert_eq!(64.0, run.advance);
}

#[test]
fn emergency_selected_break_opens_the_preferred_tracking_span() {
    let text = "deadbeefcafebabefeedfaceabcdefabcdef";
    let result = layout_with_spans(
        text,
        101.0,
        vec![LineBreakSpan {
            range: text_range(0, Text::from(text).scalar_len().value()),
            policy: LineBreakPolicy::ProgressiveTechnical,
        }],
        None,
    );
    assert!(result.lines.len() > 1, "{:?}", result.lines);
    let tracking: Vec<_> = result.debug.justification_decisions.iter()
        .flat_map(|decision| &decision.allocations)
        .filter(|allocation| allocation.kind == "EmergencyGraphemeTracking")
        .collect();
    assert!(!tracking.is_empty(), "{:?}", result.debug.justification_decisions);
}

#[test]
fn technical_line_body_stretch_rejects_the_clean_tier_and_replays() {
    let text = "中文中 aa bb 中文中文中文中文中文中文";
    let result = layout_with_spans(
        text,
        96.0,
        vec![LineBreakSpan {
            range: text_range(0, Text::from(text).scalar_len().value()),
            policy: LineBreakPolicy::ProgressiveTechnical,
        }],
        None,
    );
    assert!(result.lines.len() > 1, "{:?}", result.lines);
    assert!(result.debug.emergency_tracking_eligibility_decisions.iter().any(|decision| decision.reason.starts_with("CurrentLineTechnicalTierRejection:")), "{:?}", result.debug.emergency_tracking_eligibility_decisions);
    assert!(result.debug.break_opportunity_decisions.iter().any(|decision| decision.reason == "CurrentLineTechnicalEmergencyBreak"), "{:?}", result.debug.break_opportunity_decisions);
}

struct TaiwanProfile;

impl ClreqProfileResolver for TaiwanProfile {
    fn resolve(&self, _: &tiqian::core::text_model::LayoutProfileId) -> ClreqProfile {
        ClreqProfile::taiwan_horizontal()
    }
}

fn layout_with_taiwan_profile(text: &str, max_width: f32) -> tiqian::core::layout_model::LayoutResult {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.hyphenator = english_hyphenation::en_us();
    engine.clreq_profile_resolver = Box::new(TaiwanProfile);
    engine.layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from(text)),
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

#[test]
fn hyphen_squeeze_consumes_paired_leading_and_trailing_glue_under_taiwan_profile() {
    let result = layout_with_taiwan_profile("中文，文internationalization", 112.0);
    let comma = result.clusters.iter().find(|cluster| cluster.text == "，").unwrap();
    assert!(comma.advance < 16.0, "{}", comma.advance);
}

struct DashInkBoundsTextShaper {
    left: f32,
    right: f32,
}

impl TextShaper for DashInkBoundsTextShaper {
    fn shape(&self, input: &ShapingInput) -> ShapingResult {
        let shaped = ExplainableStubTextShaper.shape(input);
        ShapingResult::with_decisions(
            shaped.clusters,
            shaped.glyph_runs.into_iter().map(|run| tiqian::core::layout_model::GlyphRun {
                glyphs: run.glyphs.into_iter().map(|glyph| {
                    input.display_text.as_str().contains('⸺').then(|| tiqian::core::layout_model::Glyph {
                        bounds: Some(tiqian::core::geometry::Rect {
                            left: self.left,
                            top: 0.0,
                            right: self.right,
                            bottom: 16.0,
                        }),
                        ..glyph.clone()
                    }).unwrap_or(glyph)
                }).collect(),
                ..run
            }).collect(),
            shaped.decisions,
        )
    }
}

fn layout_with_dash_ink_bounds(left: f32, right: f32) -> tiqian::core::layout_model::LayoutResult {
    layout_with_spans("中——中", 200.0, Vec::new(), Some(Box::new(DashInkBoundsTextShaper { left, right })))
}

#[test]
fn dash_ink_centering_with_shaped_bounds() {
    let result = layout_with_dash_ink_bounds(1.0, 29.0);
    let dash_glyph = result.glyph_runs.iter().flat_map(|run| &run.glyphs).find(|glyph| glyph.bounds.is_some()).unwrap();
    assert_eq!(1.0, dash_glyph.x);
}

#[test]
fn dash_ink_centering_with_wide_bounds_returns_same_glyph() {
    let result = layout_with_dash_ink_bounds(0.0, 31.5);
    let dash_glyph = result.glyph_runs.iter().flat_map(|run| &run.glyphs).find(|glyph| glyph.bounds.is_some()).unwrap();
    assert_eq!(0.0, dash_glyph.x);
}

#[test]
fn inline_object_with_zero_discardable_advance() {
    let text = format!("甲{INLINE_OBJECT_REPLACEMENT_CHAR}乙丙丁戊");
    let result = layout_with_content(
        &text,
        48.0,
        false,
        Vec::new(),
        vec![InlineObjectSpan::with_trailing_boundary(
            text_range(1, 2),
            24.0,
            12.0,
            12.0,
            InlineObjectBoundaryAdjustment::builder()
                .line_end_discardable_advance(0.0)
                .build(),
        )],
    );
    assert_eq!(IntRange::new(0, 1), result.lines[0].cluster_range);
}

#[test]
fn inline_object_separator_space_trim_edge() {
    let text = format!("中{INLINE_OBJECT_REPLACEMENT_CHAR} ，文文");
    let result = layout_with_content(
        &text,
        34.0,
        false,
        Vec::new(),
        vec![InlineObjectSpan::with_fixed_boundaries(
            text_range(1, 2),
            16.0,
            12.0,
            12.0,
        )],
    );
    assert!(result.lines.len() > 1);
}
