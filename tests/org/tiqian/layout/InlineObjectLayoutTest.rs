use tiqian::org::tiqian::core::Geometry::{LayoutConstraints, TextRange};
use tiqian::org::tiqian::core::Text::Text;
use tiqian::org::tiqian::core::TextModel::{
    INLINE_OBJECT_REPLACEMENT_CHAR, InlineObjectSpan, LayoutInput, LineLengthGrid, ParagraphStyle,
    TextStyle, TiqianTextContent,
};
use tiqian::org::tiqian::core::Units::Ic;
use tiqian::org::tiqian::layout::ParagraphLayoutEngine::{
    ExplainableStubParagraphLayoutEngine, ParagraphLayoutEngine,
};

fn style() -> ParagraphStyle {
    ParagraphStyle::builder()
        .first_line_indent(Some(Ic::ZERO))
        .line_height(Some(24.0))
        .line_length_grid(LineLengthGrid::with_enabled(false))
        .build()
}

fn layout(objects: Vec<InlineObjectSpan>) -> tiqian::org::tiqian::core::LayoutModel::LayoutResult {
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
fn inline_object_reuses_existing_interline_space_without_moving_baseline_grid() {
    let plain = layout(Vec::new());
    let with_object = layout(vec![InlineObjectSpan::with_fixed_boundaries(
        TextRange::new(1, 2),
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
}

#[test]
fn inline_object_expands_only_the_boundary_with_actual_collision() {
    let result = layout(vec![
        InlineObjectSpan::with_fixed_boundaries(TextRange::new(0, 1), 16.0, 14.0, 10.0),
        InlineObjectSpan::with_fixed_boundaries(TextRange::new(1, 2), 16.0, 20.0, 2.0),
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
            TextRange::new(1, 2),
            20.0,
            30.0,
            4.0,
        )])
        .build(),
    );
    let object = result
        .clusters
        .iter()
        .find(|cluster| cluster.range == TextRange::new(1, 2))
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
