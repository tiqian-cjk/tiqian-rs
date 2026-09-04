use tiqian::clreq::clreq_profile::{ClreqProfile, ClreqProfileResolver, KinsokuLevel, KinsokuMode};
use tiqian::core::geometry::{scalar_offset, text_range, LayoutConstraints};
use tiqian::core::int_range::IntRange;
use tiqian::core::text::Text;
use tiqian::core::text_model::{
    INLINE_OBJECT_REPLACEMENT_CHAR, InlineObjectBoundaryAdjustment, InlineObjectPreferredStretch,
    InlineObjectPreferredStretchKind, InlineObjectSpan, LayoutInput, LineLengthGrid, ParagraphStyle,
    TextStyle, TiqianTextContent,
};
use tiqian::core::units::Ic;
use tiqian::layout::line_breaker::{GreedyLineBreaker, LookaheadLineBreaker};
use tiqian::layout::paragraph_dp_line_breaker::ParagraphDpLineBreaker;
use tiqian::layout::paragraph_layout_engine::{
    ExplainableStubParagraphLayoutEngine, ParagraphLayoutEngine,
};
use tiqian::layout::line_geometry_stage::resolve_inline_object_line_boundary_extent;
use tiqian::layout::progressive_break_decisions::{
    UnbreakableRanges, adjust_break_for_unbreakables,
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

fn fixed_basic_engine(
    breaker: Box<dyn tiqian::layout::line_breaker::LineBreaker>,
) -> ExplainableStubParagraphLayoutEngine {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.line_breaker = breaker;
    engine.clreq_profile_resolver = Box::new(FixedBasicProfile);
    engine.hyphenator = &NoHyphenator;
    engine
}

fn breaker(strategy: usize) -> Box<dyn tiqian::layout::line_breaker::LineBreaker> {
    match strategy {
        0 => Box::new(GreedyLineBreaker::default()),
        1 => Box::new(LookaheadLineBreaker::default()),
        2 => Box::new(ParagraphDpLineBreaker::default()),
        _ => unreachable!(),
    }
}

fn style() -> ParagraphStyle {
    ParagraphStyle::builder()
        .first_line_indent(Some(Ic::ZERO))
        .line_height(Some(24.0))
        .line_length_grid(LineLengthGrid::with_enabled(false))
        .build()
}

fn layout(objects: Vec<InlineObjectSpan>) -> tiqian::core::layout_model::LayoutResult {
    ExplainableStubParagraphLayoutEngine::default().layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from("甲乙")),
            LayoutConstraints::with_defaults(16.0),
        )
        .text_style(TextStyle::builder().font_size(16.0).build())
        .paragraph_style(style())
        .inline_objects(objects)
        .build(),
    )
}

#[test]
fn line_boundary_closes_one_ulp_gap_without_changing_baseline_distance() {
    assert_eq!(
        84.14,
        resolve_inline_object_line_boundary_extent(80.0, 84.14, 100.0, 15.86001),
    );
}

#[test]
fn inline_object_reuses_existing_interline_space_without_moving_baseline_grid() {
    let plain = layout(Vec::new());
    let with_object = layout(vec![InlineObjectSpan::with_fixed_boundaries(
        text_range(1, 2),
        16.0,
        20.0,
        2.0,
    )]);

    assert_eq!(2, with_object.lines.len());
    assert_eq!(
        plain.lines[1].baseline - plain.lines[0].baseline,
        with_object.lines[1].baseline - with_object.lines[0].baseline
    );
    assert!((with_object.lines[1].baseline - with_object.lines[0].baseline - 24.0).abs() < 0.001);
    assert_eq!(plain.size.height, with_object.size.height);
    let decision = with_object
        .debug
        .inline_object_line_height_decision
        .as_ref()
        .unwrap();
    assert_eq!(1.6, decision.minimum_clearance);
    assert!(decision.line_extras.iter().all(|extra| *extra == 0.0));
    assert!(decision.expanded_line_indices.is_empty());
    assert!(decision.boundary_shifts_after[0] < 0.0);
    assert_eq!("ExistingInterlineSpaceFitsInlineObjects", decision.reason);
    assert!((with_object.lines[1].baseline - 20.0 - (with_object.lines[0].baseline + decision.base_face_descent) >= decision.minimum_clearance - 0.001));
}

#[test]
fn inline_object_expands_only_the_boundary_with_actual_collision() {
    let result = layout(vec![
        InlineObjectSpan::with_fixed_boundaries(text_range(0, 1), 16.0, 14.0, 10.0),
        InlineObjectSpan::with_fixed_boundaries(text_range(1, 2), 16.0, 20.0, 2.0),
    ]);

    assert!((result.lines[1].baseline - result.lines[0].baseline - 31.6).abs() < 0.001);
    let decision = result
        .debug
        .inline_object_line_height_decision
        .as_ref()
        .unwrap();
    assert_eq!(0.0, decision.line_extras[0]);
    assert!((decision.line_extras[1] - 7.6).abs() < 0.001);
    assert_eq!(vec![1], decision.expanded_line_indices);
    assert_eq!("InlineObjectInterlineCollision", decision.reason);
    let without_clearance = ExplainableStubParagraphLayoutEngine::default().layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from("甲乙")),
            LayoutConstraints::with_defaults(16.0),
        )
        .text_style(TextStyle::builder().font_size(16.0).build())
        .paragraph_style(
            ParagraphStyle::builder()
                .first_line_indent(Some(Ic::ZERO))
                .line_height(Some(24.0))
                .line_length_grid(LineLengthGrid::with_enabled(false))
                .inline_object_minimum_clearance_em(0.0)
                .build(),
        )
        .inline_objects(vec![
            InlineObjectSpan::with_fixed_boundaries(text_range(0, 1), 16.0, 14.0, 10.0),
            InlineObjectSpan::with_fixed_boundaries(text_range(1, 2), 16.0, 20.0, 2.0),
        ])
        .build(),
    );
    assert!((without_clearance.lines[1].baseline - without_clearance.lines[0].baseline - 30.0).abs() < 0.001);
    assert_eq!(0.0, without_clearance.debug.inline_object_line_height_decision.unwrap().minimum_clearance);
}

#[test]
fn inline_object_skips_font_shaping_and_owns_its_line_metrics() {
    let text = format!("中{INLINE_OBJECT_REPLACEMENT_CHAR}文");
    let result = ExplainableStubParagraphLayoutEngine::default().layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from(text)),
            LayoutConstraints::with_defaults(120.0),
        )
        .text_style(TextStyle::builder().font_size(16.0).build())
        .paragraph_style(style())
        .inline_objects(vec![InlineObjectSpan::with_fixed_boundaries(
            text_range(1, 2),
            20.0,
            30.0,
            4.0,
        )])
        .build(),
    );
    let object = result
        .clusters
        .iter()
        .find(|cluster| cluster.range == text_range(1, 2))
        .unwrap();

    assert_eq!(20.0, object.advance);
    assert!(
        result
            .glyph_runs
            .iter()
            .flat_map(|run| &run.glyphs)
            .all(|glyph| glyph.cluster_range != object.range)
    );
    let shaping = result
        .debug
        .shaping_decisions
        .iter()
        .find(|decision| decision.range == object.range)
        .unwrap();
    assert_eq!(0, shaping.glyph_count);
    assert_eq!(
        "MeasurableOpaqueInlineObject:no-font-shaping",
        shaping.reason
    );
    assert!(result.lines[0].baseline - result.lines[0].top >= 30.0);
    assert!(result.lines[0].bottom - result.lines[0].baseline >= 4.0);
    assert_eq!(
        "MeasurableOpaqueInlineObject",
        result.debug.inline_object_decisions[0].reason
    );
}

#[test]
fn inline_object_is_one_indivisible_break_cluster() {
    let text = format!("中{INLINE_OBJECT_REPLACEMENT_CHAR}文");
    let result = ExplainableStubParagraphLayoutEngine::default().layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from(text)),
            LayoutConstraints::with_defaults(35.0),
        )
        .text_style(TextStyle::builder().font_size(16.0).build())
        .paragraph_style(style())
        .inline_objects(vec![InlineObjectSpan::with_fixed_boundaries(
            text_range(1, 2), 20.0, 16.0, 4.0,
        )])
        .build(),
    );
    let index = result.clusters.iter().position(|cluster| cluster.range == text_range(1, 2)).unwrap() as i32;
    assert!(result.lines.iter().any(|line| line.cluster_range.first() == index && line.cluster_range.last() == index));
}

#[test]
fn inline_object_keeps_alternate_source_text_while_skipping_its_glyph_shaping() {
    let result = ExplainableStubParagraphLayoutEngine::default().layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from("中图片文")),
            LayoutConstraints::with_defaults(120.0),
        )
        .text_style(TextStyle::builder().font_size(16.0).build())
        .paragraph_style(style())
        .inline_objects(vec![InlineObjectSpan::with_fixed_boundaries(
            text_range(1, 3), 20.0, 16.0, 4.0,
        )])
        .build(),
    );
    let object = result.clusters.iter().find(|cluster| cluster.range == text_range(1, 3)).unwrap();
    assert_eq!("图片", object.text);
    assert_eq!("", object.display_text);
    assert!(result.glyph_runs.iter().flat_map(|run| &run.glyphs).all(|glyph| glyph.cluster_range != object.range));
    let shaping = result.debug.shaping_decisions.iter().find(|decision| decision.range == object.range).unwrap();
    assert_eq!("图片", shaping.source_text);
    assert_eq!("", shaping.display_text);
}

#[test]
fn adjust_break_for_unbreakables_retreats_past_the_whole_contiguous_run() {
    let chain = UnbreakableRanges::new(vec![
        IntRange::new(1, 2),
        IntRange::new(2, 3),
        IntRange::new(3, 4),
    ]);
    assert_eq!(1, adjust_break_for_unbreakables(4, 0, &chain));
    assert_eq!(1, adjust_break_for_unbreakables(3, 0, &chain));
    assert_eq!(1, adjust_break_for_unbreakables(2, 0, &chain));
    assert_eq!(5, adjust_break_for_unbreakables(5, 0, &chain));
    assert_eq!(
        3,
        adjust_break_for_unbreakables(
            5,
            2,
            &UnbreakableRanges::new(vec![IntRange::new(3, 4), IntRange::new(4, 5)]),
        )
    );
    assert_eq!(4, adjust_break_for_unbreakables(4, 1, &chain));
}

#[test]
fn formula_boundary_compression_pushes_attached_comma_into_previous_line() {
    let text = "x+，后";
    let result = fixed_basic_engine(Box::new(GreedyLineBreaker::default())).layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from(text)),
            LayoutConstraints::with_defaults(36.0),
        )
        .text_style(TextStyle::builder().font_size(16.0).build())
        .paragraph_style(style())
        .inline_objects(vec![InlineObjectSpan::with_trailing_boundary(
            text_range(0, 2),
            30.0,
            16.0,
            4.0,
            InlineObjectBoundaryAdjustment::builder()
                .participates_in_uniform_stretch(true)
                .shrink_capacity(4.0)
                .build(),
        )])
        .build(),
    );
    assert!(result.lines.iter().all(|line| !Text::from(text).slice_text(line.range).as_str().starts_with('，')));
    let repair = result.debug.line_decisions[0].repair_decision.as_ref().unwrap();
    assert_eq!("PushIn", repair.kind);
    assert!(repair.push_in_allocations.iter().any(|allocation| allocation.cluster_range == text_range(0, 2) && allocation.shrink > 0.0));
}

#[test]
fn per_atom_formula_chain_never_breaks_mid_run() {
    let closed = InlineObjectBoundaryAdjustment::builder().prevents_line_break(true).build();
    let objects: Vec<_> = (1..=4)
        .map(|index| {
            InlineObjectSpan::with_trailing_boundary(
                text_range(index, index + 1),
                12.0,
                16.0,
                4.0,
                if index < 4 { closed.clone() } else { InlineObjectBoundaryAdjustment::FIXED },
            )
        })
        .collect();
    let result = fixed_basic_engine(Box::new(LookaheadLineBreaker::default())).layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from("中一二三四")),
            LayoutConstraints::with_defaults(60.0),
        )
        .text_style(TextStyle::builder().font_size(16.0).build())
        .paragraph_style(style())
        .inline_objects(objects)
        .build(),
    );
    for line in &result.lines {
        assert!(![scalar_offset(2), scalar_offset(3), scalar_offset(4)].contains(&line.range.end()), "{:?}", result.lines);
    }
}

#[test]
fn punctuation_attached_to_inline_object_never_starts_wrapped_line() {
    for strategy in 0..3 {
        for comma in ['，', ','] {
            let text = format!("x+{comma} 后");
            for width in [24.0, 32.0, 36.0, 48.0, 64.0] {
                let result = fixed_basic_engine(breaker(strategy)).layout(
                    LayoutInput::builder(
                        TiqianTextContent::new(Text::from(text.as_str())),
                        LayoutConstraints::with_defaults(width),
                    )
                    .text_style(TextStyle::builder().font_size(16.0).build())
                    .paragraph_style(style())
                    .inline_objects(vec![InlineObjectSpan::with_fixed_boundaries(
                        text_range(0, 2), 30.0, 16.0, 4.0,
                    )])
                    .build(),
                );
                assert!(result.lines.iter().all(|line| !Text::from(text.as_str()).slice_text(line.range).as_str().starts_with(comma)));
                assert!(result.debug.contextual_kinsoku_decisions.iter().any(|decision| decision.source_text == comma.to_string() && decision.reason == "InlineObjectAttachedKinsoku"));
            }
        }
    }
}

#[test]
fn separator_space_before_punctuation_collapses_and_stays_with_inline_object() {
    let text = "前x ，后文";
    for strategy in 0..3 {
        for width in [32.0, 40.0, 48.0, 56.0, 64.0] {
            let adjustment = InlineObjectBoundaryAdjustment::builder().participates_in_uniform_stretch(true).build();
            let result = fixed_basic_engine(breaker(strategy)).layout(
                LayoutInput::builder(
                    TiqianTextContent::new(Text::from(text)),
                    LayoutConstraints::with_defaults(width),
                )
                .text_style(TextStyle::builder().font_size(16.0).build())
                .paragraph_style(style())
                .inline_objects(vec![InlineObjectSpan::new(
                    text_range(1, 2), 24.0, 16.0, 4.0, adjustment.clone(), adjustment,
                )])
                .build(),
            );
            assert_eq!(0.0, result.clusters.iter().find(|cluster| cluster.range == text_range(2, 3)).unwrap().advance);
            let line_texts: Vec<_> = result.lines.iter().map(|line| Text::from(text).slice_text(line.range).to_string()).collect();
            assert!(
                line_texts.iter().all(|line| !line.trim_start().starts_with('，')),
                "strategy={strategy} width={width} lines={line_texts:?}"
            );
            assert!(result.debug.contextual_kinsoku_decisions.iter().any(|decision| decision.source_text == "，" && decision.reason == "InlineObjectAttachedKinsokuAcrossCollapsedSeparatorSpace"));
            let attachment = &result.debug.inline_object_punctuation_attachment_decisions[0];
            assert_eq!(text_range(2, 3), attachment.separator_range);
            assert!(attachment.collapsed_advance > 0.0);
            assert!(result.debug.justification_decisions.iter().flat_map(|decision| &decision.allocations).all(|allocation| allocation.cluster_range != text_range(1, 2) || allocation.kind != "InlineObjectBoundary"));
        }
    }
}

#[test]
fn relation_stretch_moves_both_formula_sides_by_the_same_final_geometry() {
    let natural_gap = 5.0 / 18.0 * 16.0;
    let target_gap = 8.0;
    let relation = |prevents_line_break| {
        InlineObjectBoundaryAdjustment::builder()
            .participates_in_uniform_stretch(true)
            .preferred_stretch(InlineObjectPreferredStretch::new(
                InlineObjectPreferredStretchKind::Relation,
                natural_gap,
                target_gap,
            ))
            .prevents_line_break(prevents_line_break)
            .build()
    };
    let result = fixed_basic_engine(Box::new(GreedyLineBreaker::default())).layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from("a=b中")),
            LayoutConstraints::with_defaults(47.0),
        )
        .text_style(TextStyle::builder().font_size(16.0).build())
        .paragraph_style(style())
        .inline_objects(vec![
            InlineObjectSpan::with_trailing_boundary(text_range(0, 1), 10.0 + natural_gap, 12.0, 4.0, relation(false)),
            InlineObjectSpan::with_trailing_boundary(text_range(1, 2), 10.0 + natural_gap, 12.0, 4.0, relation(true)),
            InlineObjectSpan::with_fixed_boundaries(text_range(2, 3), 10.0, 12.0, 4.0),
        ])
        .build(),
    );
    let positioned = tiqian::core::layout_queries::positioned_clusters(&result);
    let formula: Vec<_> = positioned.iter().filter(|cluster| cluster.range.end() <= scalar_offset(3)).collect();
    assert_eq!(3, formula.len());
    assert!(formula.iter().all(|cluster| cluster.line_index == 0));
    let before = formula[1].draw_x - (formula[0].draw_x + 10.0);
    let after = formula[2].draw_x - (formula[1].draw_x + 10.0);
    assert!((before - after).abs() < 0.001);
    assert!(before >= target_gap);
    let allocations: Vec<_> = result.debug.justification_decisions[0].allocations.iter().filter(|allocation| allocation.kind == "InlineObjectRelation").collect();
    assert_eq!(2, allocations.len());
    assert!((allocations[0].delta - allocations[1].delta).abs() < 0.001);
}

#[test]
fn formula_break_keeps_baseline_operator_on_previous_line() {
    let text = "a+b";
    let operator = InlineObjectSpan::new(
        text_range(1, 2),
        12.0,
        12.0,
        4.0,
        InlineObjectBoundaryAdjustment::builder().prevents_line_break(true).build(),
        InlineObjectBoundaryAdjustment::builder().shrink_capacity(4.0).line_end_discardable_advance(4.0).build(),
    );
    for strategy in 0..3 {
        let objects = vec![
            InlineObjectSpan::with_fixed_boundaries(text_range(0, 1), 12.0, 12.0, 4.0),
            operator.clone(),
            InlineObjectSpan::with_fixed_boundaries(text_range(2, 3), 12.0, 12.0, 4.0),
        ];
        let result = fixed_basic_engine(breaker(strategy)).layout(
            LayoutInput::builder(TiqianTextContent::new(Text::from(text)), LayoutConstraints::with_defaults(24.0))
                .text_style(TextStyle::builder().font_size(16.0).build())
                .paragraph_style(style())
                .inline_objects(objects.clone())
                .build(),
        );
        let lines: Vec<_> = result.lines.iter().map(|line| Text::from(text).slice_text(line.range).to_string()).collect();
        assert!(lines.len() > 1);
        assert!(lines.iter().skip(1).all(|line| !line.starts_with('+')));
        assert!(lines.iter().take(lines.len() - 1).any(|line| line.ends_with('+')));
        assert_eq!(8.0, result.clusters.iter().find(|cluster| cluster.range == text_range(1, 2)).unwrap().advance);
        assert_eq!(20.0, result.lines[0].visual_width);
        assert_eq!(0.0, tiqian::core::layout_queries::positioned_clusters(&result).iter().find(|cluster| cluster.range == text_range(2, 3)).unwrap().draw_x);
        assert!(result.debug.line_edge_trim_decisions.iter().any(|decision| decision.cluster_range == text_range(1, 2) && decision.reason == "InlineObjectLineEndDiscardableGlue" && decision.natural_glue == 4.0));
        let decision = result.debug.inline_object_decisions.iter().find(|decision| decision.range == text_range(1, 2)).unwrap();
        assert!(decision.leading_prevents_line_break);
        assert_eq!(4.0, decision.trailing_line_end_discardable_advance);
        let unbroken = fixed_basic_engine(breaker(strategy)).layout(
            LayoutInput::builder(TiqianTextContent::new(Text::from(text)), LayoutConstraints::with_defaults(60.0))
                .text_style(TextStyle::builder().font_size(16.0).build())
                .paragraph_style(style())
                .inline_objects(objects)
                .build(),
        );
        assert_eq!(1, unbroken.lines.len());
        assert_eq!(12.0, unbroken.clusters.iter().find(|cluster| cluster.range == text_range(1, 2)).unwrap().advance);
        assert!(unbroken.debug.line_edge_trim_decisions.iter().all(|decision| decision.reason != "InlineObjectLineEndDiscardableGlue"));
    }
}
