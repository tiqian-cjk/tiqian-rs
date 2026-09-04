use tiqian::clreq::clreq_profile::{ClreqProfile, ClreqProfileResolver, KinsokuLevel, KinsokuMode};
use tiqian::core::geometry::{scalar_offset, text_range, LayoutConstraints};
use tiqian::core::text::Text;
use tiqian::core::text_model::{
    DecorationKind, DecorationSpan, LayoutInput, LineLengthGrid, ParagraphStyle, TiqianTextContent,
};
use tiqian::core::units::{Ic, IcLiteral};
use tiqian::layout::paragraph_layout_engine::{
    ExplainableStubParagraphLayoutEngine, ParagraphLayoutEngine,
};

struct FixedBasicProfile;

impl ClreqProfileResolver for FixedBasicProfile {
    fn resolve(&self, _: &tiqian::core::text_model::LayoutProfileId) -> ClreqProfile {
        let mut profile = ClreqProfile::mainland_horizontal();
        profile.kinsoku_mode = KinsokuMode::fixed(KinsokuLevel::Basic);
        profile
    }
}

fn layout(
    style: ParagraphStyle,
    text: &str,
    decorations: Vec<DecorationSpan>,
) -> tiqian::core::layout_model::LayoutResult {
    ExplainableStubParagraphLayoutEngine::default().layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from(text)),
            LayoutConstraints::with_defaults(240.0),
        )
        .paragraph_style(style)
        .decorations(decorations)
        .build(),
    )
}

#[test]
fn cjk_line_box_uses_font_declared_ideographic_typo_metrics() {
    let result = layout(
        ParagraphStyle::builder()
            .first_line_indent(Some(Ic::ZERO))
            .build(),
        "提椠",
        Vec::new(),
    );
    let line = &result.lines[0];
    let cjk = result
        .debug
        .metric_decisions
        .iter()
        .find(|decision| decision.role == "CjkText")
        .unwrap();

    assert!((line.baseline - 18.08).abs() < 0.001);
    assert_eq!(24.0, line.bottom);
    assert_eq!(14.08, cjk.layout_ascent);
    assert_eq!(1.92, cjk.layout_descent);
    assert_eq!("IdeographicLow", cjk.baseline_class);
    assert_eq!("IdeographicEmBox", cjk.metric_box);
}

#[test]
fn auto_space_gap_at_line_end_is_trimmed_like_any_line_edge_blank() {
    let result = ExplainableStubParagraphLayoutEngine::default().layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from("中文 AB 中文中文中文")),
            LayoutConstraints::with_defaults(80.0),
        )
        .paragraph_style(ParagraphStyle::builder().first_line_indent(Some(Ic::ZERO)).build())
        .build(),
    );

    assert_eq!(66.0, result.lines[0].adjusted_width);
    let collapse = result
        .debug
        .line_edge_trim_decisions
        .iter()
        .find(|decision| decision.reason == "LineEdgeWordSpaceCollapse")
        .unwrap();
    assert_eq!("trailing", collapse.side);
    assert_eq!(2.0, collapse.trim_amount);
    assert_eq!(text_range(5, 6), collapse.cluster_range);
}

#[test]
fn emphasis_span_produces_dot_anchors_for_han_and_skips_punctuation() {
    let result = ExplainableStubParagraphLayoutEngine::default().layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from("他强调：豆子新鲜最要紧，烘焙其次。")),
            LayoutConstraints::with_defaults(128.0),
        )
        .paragraph_style(ParagraphStyle::builder().first_line_indent(Some(Ic::ZERO)).build())
        .decorations(vec![DecorationSpan {
            range: text_range(4, 16),
            kind: DecorationKind::Emphasis,
        }])
        .build(),
    );

    let decisions = &result.debug.decoration_decisions;
    assert_eq!(12, decisions.len());
    let applied: Vec<_> = decisions.iter().filter(|decision| decision.applied).collect();
    assert_eq!(11, applied.len());
    assert!(applied.iter().all(|decision| decision.reason == "EmphasisDotOnHanText"));
    let comma = decisions.iter().find(|decision| decision.source_text == "，").unwrap();
    assert!(!comma.applied);
    assert_eq!("clreq-no-dot-on-punctuation", comma.reason);
    assert!(decisions.iter().all(|decision| decision.source_text != "。"));
    let first = decisions.iter().find(|decision| decision.source_text == "豆").unwrap();
    assert_eq!(72.0, first.anchor_x);
    assert!((first.dot_diameter - 16.0 * 0.19).abs() < 0.01);
    assert!((first.anchor_y - (result.lines[0].baseline + 16.0 * 0.12 + 16.0 * 0.1 + first.dot_diameter / 2.0)).abs() < 0.01);
}

#[test]
fn block_indent_insets_every_line() {
    let result = ExplainableStubParagraphLayoutEngine::default().layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from("中文中文中文中文中文中文")),
            LayoutConstraints::with_defaults(100.0),
        )
        .paragraph_style(
            ParagraphStyle::builder()
                .block_indent(2.0f32.ic())
                .first_line_indent(Some(Ic::ZERO))
                .line_length_grid(LineLengthGrid::with_enabled(false))
                .build(),
        )
        .build(),
    );

    assert!(result.lines.len() >= 2);
    assert!(result.lines.iter().all(|line| line.indent == 32.0));
}

#[test]
fn hanging_indent_flushes_first_line_and_insets_rest() {
    let result = ExplainableStubParagraphLayoutEngine::default().layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from("中文中文中文中文中文中文")),
            LayoutConstraints::with_defaults(100.0),
        )
        .paragraph_style(
            ParagraphStyle::builder()
                .block_indent(2.0f32.ic())
                .first_line_indent(Some((-2.0f32).ic()))
                .line_length_grid(LineLengthGrid::with_enabled(false))
                .build(),
        )
        .build(),
    );

    assert!(result.lines.len() >= 2);
    assert_eq!(0.0, result.lines[0].indent);
    assert!(result.lines[1..].iter().all(|line| line.indent == 32.0));
}

#[test]
fn first_line_indent_shrinks_first_line_measure_only() {
    let result = ExplainableStubParagraphLayoutEngine::default().layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from("中文中文中文中文中文中文")),
            LayoutConstraints::with_defaults(160.0),
        )
        .paragraph_style(ParagraphStyle::builder().first_line_indent(Some(2.0f32.ic())).build())
        .build(),
    );

    assert_eq!(2, result.lines.len());
    assert_eq!(32.0, result.lines[0].indent);
    assert_eq!(0.0, result.lines[1].indent);
    assert_eq!(8, result.lines[0].range.end() - result.lines[0].range.start());
    assert_eq!(128.0, result.lines[0].visual_width);
    assert_eq!(160.0, result.size.width);
}

#[test]
fn line_length_grid_floors_measure_to_whole_chars_and_offsets_body() {
    let layout_with = |grid| ExplainableStubParagraphLayoutEngine::default().layout(
        LayoutInput::builder(TiqianTextContent::new(Text::from("中文中文中文中文")), LayoutConstraints::with_defaults(104.0))
            .paragraph_style(ParagraphStyle::builder().first_line_indent(Some(Ic::ZERO)).line_length_grid(grid).build())
            .build(),
    );
    let start = layout_with(LineLengthGrid::default());
    let grid = start.debug.line_length_grid_decision.as_ref().unwrap();
    assert!(grid.enabled);
    assert_eq!(6, grid.cells);
    assert_eq!(96.0, grid.measure);
    assert_eq!(8.0, grid.slack);
    assert_eq!(0.0, grid.body_offset);
    assert_eq!(2, start.lines.len());
    assert_eq!(96.0, start.lines[0].visual_width);
    assert_eq!(0.0, start.lines[0].indent);

    let centered = layout_with(LineLengthGrid::with_body_alignment(Some(tiqian::core::text_model::LastLineAlignment::Center)));
    assert_eq!(4.0, centered.debug.line_length_grid_decision.as_ref().unwrap().body_offset);
    assert_eq!(4.0, centered.lines[0].indent);
    assert_eq!(4.0, centered.lines[1].indent);
}

#[test]
fn line_length_grid_can_be_bypassed_for_exact_widths() {
    let result = ExplainableStubParagraphLayoutEngine::default().layout(
        LayoutInput::builder(TiqianTextContent::new(Text::from("中文中文中文中文")), LayoutConstraints::with_defaults(104.0))
            .paragraph_style(ParagraphStyle::builder().first_line_indent(Some(Ic::ZERO)).line_length_grid(LineLengthGrid::with_enabled(false)).build())
            .build(),
    );
    let grid = result.debug.line_length_grid_decision.as_ref().unwrap();
    assert!(!grid.enabled);
    assert_eq!(104.0, grid.measure);
    assert_eq!(0.0, grid.body_offset);
    assert_eq!(104.0, result.lines[0].visual_width);
}

#[test]
fn first_line_indent_adapts_to_measure_and_can_be_overridden() {
    let layout_with = |mut engine: ExplainableStubParagraphLayoutEngine, width, style: Option<ParagraphStyle>| {
        engine.layout(
            LayoutInput::builder(TiqianTextContent::new(Text::from("中文")), LayoutConstraints::with_defaults(width))
                .paragraph_style(style.unwrap_or_default())
                .build(),
        )
    };
    let long = layout_with(ExplainableStubParagraphLayoutEngine::default(), 240.0, None);
    assert_eq!(32.0, long.lines[0].indent);
    assert_eq!("MeasureAdaptiveFirstLineIndent", long.debug.first_line_indent_decision.as_ref().unwrap().source);
    assert_eq!(2.0, long.debug.first_line_indent_decision.as_ref().unwrap().resolved_em);
    let short = layout_with(ExplainableStubParagraphLayoutEngine::default(), 160.0, None);
    assert_eq!(16.0, short.lines[0].indent);
    assert_eq!(1.0, short.debug.first_line_indent_decision.as_ref().unwrap().resolved_em);
    let mut fixed = ExplainableStubParagraphLayoutEngine::default();
    fixed.clreq_profile_resolver = Box::new(FixedBasicProfile);
    assert_eq!(16.0, layout_with(fixed, 160.0, None).lines[0].indent);
    assert_eq!(0.0, layout_with(ExplainableStubParagraphLayoutEngine::default(), 240.0, Some(ParagraphStyle::builder().first_line_indent(Some(Ic::ZERO)).build())).lines[0].indent);
    let pinned = layout_with(ExplainableStubParagraphLayoutEngine::default(), 160.0, Some(ParagraphStyle::builder().first_line_indent(Some(2.0f32.ic())).build()));
    assert_eq!(32.0, pinned.lines[0].indent);
    assert_eq!("Explicit", pinned.debug.first_line_indent_decision.as_ref().unwrap().source);
}

#[test]
fn mourning_span_is_kept_unbroken_and_framed_per_line() {
    let result = ExplainableStubParagraphLayoutEngine::default().layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from("悼念：王小明同志、张大同同志。")),
            LayoutConstraints::with_defaults(72.0),
        )
        .paragraph_style(
            ParagraphStyle::builder()
                .first_line_indent(Some(Ic::ZERO))
                .line_length_grid(LineLengthGrid::with_enabled(false))
                .build(),
        )
        .decorations(vec![
            DecorationSpan { range: text_range(3, 6), kind: DecorationKind::Mourning },
            DecorationSpan { range: text_range(9, 12), kind: DecorationKind::Mourning },
        ])
        .build(),
    );

    assert_eq!(scalar_offset(3), result.lines[0].range.end());
    let segments = &result.debug.decoration_segments;
    assert_eq!(2, segments.len());
    assert!(segments.iter().all(|segment| {
        segment.reason == "MourningSpanKeptUnbroken" && !segment.open_start && !segment.open_end
    }));
    let first = segments.iter().find(|segment| segment.source_range.start().value() == 3).unwrap();
    assert_eq!(0.0, first.left);
    assert!((first.right - 160.0 / 3.0).abs() < 0.01);
    let line = &result.lines[1];
    assert!((first.top - (line.baseline - 14.08)).abs() < 0.01);
    assert!((first.bottom - (line.baseline + 1.92)).abs() < 0.01);
}

#[test]
fn mourning_span_wider_than_measure_splits_with_open_edges() {
    let result = ExplainableStubParagraphLayoutEngine::default().layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from("王小明大同先生")),
            LayoutConstraints::with_defaults(64.0),
        )
        .paragraph_style(ParagraphStyle::builder().first_line_indent(Some(Ic::ZERO)).build())
        .decorations(vec![DecorationSpan { range: text_range(0, 5), kind: DecorationKind::Mourning }])
        .build(),
    );

    let segments = &result.debug.decoration_segments;
    assert_eq!(2, segments.len());
    assert!(segments.iter().all(|segment| segment.reason == "mourning-span-split-across-lines"));
    assert!(!segments[0].open_start);
    assert!(segments[0].open_end);
    assert!(segments[1].open_start);
    assert!(!segments[1].open_end);
}

#[test]
fn justify_stretches_punctuation_latin_boundary_in_tier_three() {
    let result = ExplainableStubParagraphLayoutEngine::default().layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from("中文中文话：The quick brown fox jumps")),
            LayoutConstraints::with_defaults(160.0),
        )
        .paragraph_style(ParagraphStyle::builder().first_line_indent(Some(Ic::ZERO)).build())
        .build(),
    );

    assert!(result.lines.len() > 1);
    let line0 = result.debug.justification_decisions.iter().find(|decision| decision.line_range.start().value() == 0).unwrap();
    assert!(line0.allocations.iter().any(|allocation| {
        allocation.cluster_range == text_range(5, 6) && allocation.kind == "CjkInterChar"
    }));
    assert_eq!(0.0, line0.deficit_after);
}

#[test]
fn interlinear_lines_get_per_item_segments_with_adjacent_shortening() {
    let result = ExplainableStubParagraphLayoutEngine::default().layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from("屈原写下离骚，顾炎武王夫之并称。")),
            LayoutConstraints::with_defaults(224.0),
        )
        .paragraph_style(ParagraphStyle::builder().first_line_indent(Some(Ic::ZERO)).build())
        .decorations(vec![
            DecorationSpan { range: text_range(0, 2), kind: DecorationKind::ProperNoun },
            DecorationSpan { range: text_range(4, 6), kind: DecorationKind::BookTitle },
            DecorationSpan { range: text_range(7, 10), kind: DecorationKind::ProperNoun },
            DecorationSpan { range: text_range(10, 13), kind: DecorationKind::ProperNoun },
        ])
        .build(),
    );

    let segments = &result.debug.decoration_segments;
    assert_eq!(4, segments.len());
    let baseline = result.lines[0].baseline;
    let quyuan = segments.iter().find(|segment| segment.source_range.start().value() == 0).unwrap();
    assert_eq!("ProperNoun", quyuan.kind);
    assert_eq!(0.0, quyuan.left);
    assert_eq!(32.0, quyuan.right);
    assert!((quyuan.top - (baseline + 16.0 * 0.18)).abs() < 0.01);
    assert!((quyuan.bottom - (baseline + 16.0 * 0.18)).abs() < 0.01);
    assert_eq!("InterlinearLinePerAnnotatedItem", quyuan.reason);
    let lisao = segments.iter().find(|segment| segment.source_range.start().value() == 4).unwrap();
    assert_eq!("BookTitle", lisao.kind);
    assert_eq!(64.0, lisao.left);
    assert_eq!(96.0, lisao.right);
    assert!((lisao.top - (baseline + 16.0 * 0.24)).abs() < 0.01);
    let guyanwu = segments.iter().find(|segment| segment.source_range.start().value() == 7).unwrap();
    assert_eq!(112.0, guyanwu.left);
    assert_eq!(159.0, guyanwu.right);
    assert!(guyanwu.reason.ends_with("AdjacentInterlinearLineShortening"));
    let wangfuzhi = segments.iter().find(|segment| segment.source_range.start().value() == 10).unwrap();
    assert_eq!(161.0, wangfuzhi.left);
    assert_eq!(208.0, wangfuzhi.right);
    assert_eq!(24.0, result.lines[0].bottom - result.lines[0].top);
}

#[test]
fn interlinear_marks_raise_auto_line_height_to_spacing_floor() {
    let layout_with = |line_height| ExplainableStubParagraphLayoutEngine::default().layout(
        LayoutInput::builder(TiqianTextContent::new(Text::from("豆子新鲜")), LayoutConstraints::with_defaults(240.0))
            .paragraph_style(ParagraphStyle::builder().first_line_indent(Some(Ic::ZERO)).line_height(line_height).build())
            .decorations(vec![DecorationSpan { range: text_range(0, 4), kind: DecorationKind::Emphasis }])
            .build(),
    );
    let auto = layout_with(None);
    assert_eq!(24.0, auto.lines[0].bottom);
    assert!(!auto.debug.line_spacing_decision.as_ref().unwrap().floor_applied);
    let clamped = layout_with(Some(20.0));
    assert_eq!(24.0, clamped.lines[0].bottom);
    assert!(clamped.debug.line_spacing_decision.as_ref().unwrap().floor_applied);
    let generous = layout_with(Some(28.0));
    assert_eq!(28.0, generous.lines[0].bottom);
    assert!(!generous.debug.line_spacing_decision.as_ref().unwrap().floor_applied);
    let plain = ExplainableStubParagraphLayoutEngine::default().layout(
        LayoutInput::builder(TiqianTextContent::new(Text::from("豆子新鲜")), LayoutConstraints::with_defaults(240.0))
            .paragraph_style(ParagraphStyle::builder().first_line_indent(Some(Ic::ZERO)).build())
            .build(),
    );
    assert_eq!(24.0, plain.lines[0].bottom);
    assert_eq!("CjkBodyLineHeightDefault", plain.debug.line_spacing_decision.as_ref().unwrap().reason);
    assert!(!plain.debug.line_spacing_decision.as_ref().unwrap().floor_applied);
}

#[test]
fn emphasis_dot_gap_is_explicit_and_independent_of_line_height() {
    for height in [24.0, 48.0] {
        let result = layout(
            ParagraphStyle::builder()
                .first_line_indent(Some(Ic::ZERO))
                .line_height(Some(height))
                .emphasis_dot_gap_em(0.25)
                .build(),
            "着重",
            vec![DecorationSpan {
                range: text_range(0, 2),
                kind: DecorationKind::Emphasis,
            }],
        );
        let dot = result
            .debug
            .decoration_decisions
            .iter()
            .find(|decision| decision.applied)
            .unwrap();
        assert!(
            (dot.anchor_y
                - (result.lines[0].baseline + 16.0 * 0.12 + 16.0 * 0.25 + dot.dot_diameter / 2.0))
                .abs()
                < 0.01
        );
    }
}

#[test]
fn interlinear_marks_clamp_tight_line_height_to_spacing_floor() {
    let marks = vec![DecorationSpan {
        range: text_range(0, 4),
        kind: DecorationKind::Emphasis,
    }];
    let clamped = layout(
        ParagraphStyle::builder()
            .first_line_indent(Some(Ic::ZERO))
            .line_height(Some(20.0))
            .build(),
        "豆子新鲜",
        marks.clone(),
    );
    assert_eq!(24.0, clamped.lines[0].bottom);
    assert!(
        clamped
            .debug
            .line_spacing_decision
            .as_ref()
            .unwrap()
            .floor_applied
    );

    let generous = layout(
        ParagraphStyle::builder()
            .first_line_indent(Some(Ic::ZERO))
            .line_height(Some(28.0))
            .build(),
        "豆子新鲜",
        marks,
    );
    assert_eq!(28.0, generous.lines[0].bottom);
    assert!(
        !generous
            .debug
            .line_spacing_decision
            .as_ref()
            .unwrap()
            .floor_applied
    );
}
