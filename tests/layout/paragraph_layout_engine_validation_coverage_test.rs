use tiqian::core::geometry::{text_range, LayoutConstraints, TextRange};
use tiqian::api::ParagraphLayoutEngineBuilder;
use tiqian::core::text::Text;
use tiqian::core::text_model::{
    DecorationKind, DecorationSpan, InlineBoxSpan, InlineObjectBoundaryAdjustment,
    InlineObjectSpan, LayoutInput, LineBreakPolicy, LineBreakSpan, ParagraphStyle,
    TiqianTextContent,
};
use crate::support::DeterministicStubFontBackend;

fn input(
    paragraph_style: ParagraphStyle,
    inline_boxes: Vec<InlineBoxSpan>,
    inline_objects: Vec<InlineObjectSpan>,
    content: TiqianTextContent,
) -> LayoutInput {
    LayoutInput::builder(content, LayoutConstraints::with_defaults(100.0))
        .paragraph_style(paragraph_style)
        .inline_boxes(inline_boxes)
        .inline_objects(inline_objects)
        .build()
}

fn inline_object(
    range: TextRange,
    advance: f32,
    ascent: f32,
    descent: f32,
    leading_boundary: InlineObjectBoundaryAdjustment,
    trailing_boundary: InlineObjectBoundaryAdjustment,
) -> InlineObjectSpan {
    InlineObjectSpan::new(
        range,
        advance,
        ascent,
        descent,
        leading_boundary,
        trailing_boundary,
    )
}

fn expect_layout_continues(layout_input: LayoutInput) {
    let mut engine = ParagraphLayoutEngineBuilder::new(Box::new(DeterministicStubFontBackend::default())).build();
    let result = engine.layout(layout_input);
    assert!(result.clusters.iter().all(|cluster| cluster.advance.is_finite()));
    let next = engine.layout(input(
        ParagraphStyle::default(),
        Vec::new(),
        Vec::new(),
        TiqianTextContent::new(Text::from("后续文本")),
    ));
    assert!(!next.clusters.is_empty());
}

#[test]
fn invalid_emphasis_dot_gap_em_uses_local_default() {
    let nan_style = ParagraphStyle::builder().emphasis_dot_gap_em(f32::NAN).build();
    let mut engine = ParagraphLayoutEngineBuilder::new(Box::new(DeterministicStubFontBackend::default())).build();
    let result = engine.layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from("甲乙")),
            LayoutConstraints::with_defaults(100.0),
        )
        .paragraph_style(nan_style)
        .decorations(vec![DecorationSpan {
            range: text_range(0, 2),
            kind: DecorationKind::Emphasis,
        }])
        .build(),
    );
    assert!(result
        .debug
        .decoration_decisions
        .iter()
        .filter(|decision| decision.applied)
        .all(|decision| decision.anchor_y.is_finite()));
    let negative_style = ParagraphStyle::builder().emphasis_dot_gap_em(-0.1).build();
    expect_layout_continues(input(negative_style, Vec::new(), Vec::new(), TiqianTextContent::new(Text::from("甲乙"))));
}

#[test]
fn invalid_inline_object_minimum_clearance_em_uses_local_default() {
    let nan_style = ParagraphStyle::builder()
        .inline_object_minimum_clearance_em(f32::NAN)
        .build();
    let result = ParagraphLayoutEngineBuilder::new(Box::new(DeterministicStubFontBackend::default())).build().layout(
        input(
            nan_style,
            Vec::new(),
            vec![InlineObjectSpan::with_fixed_boundaries(text_range(0, 1), 16.0, 14.0, 10.0)],
            TiqianTextContent::new(Text::from("甲乙")),
        ),
    );
    assert_eq!(
        1.6,
        result
            .debug
            .inline_object_line_height_decision
            .unwrap()
            .minimum_clearance,
    );
    let negative_style = ParagraphStyle::builder()
        .inline_object_minimum_clearance_em(-1.0)
        .build();
    expect_layout_continues(input(negative_style, Vec::new(), Vec::new(), TiqianTextContent::new(Text::from("甲乙"))));
}

#[test]
fn invalid_inline_box_ranges_have_no_effect() {
    expect_layout_continues(
        input(
            ParagraphStyle::default(),
            vec![InlineBoxSpan::new(text_range(0, 0))],
            Vec::new(),
            TiqianTextContent::new(Text::from("甲乙")),
        ),
    );
    expect_layout_continues(
        input(
            ParagraphStyle::default(),
            vec![InlineBoxSpan::new(text_range(1, 9))],
            Vec::new(),
            TiqianTextContent::new(Text::from("甲乙")),
        ),
    );
}

#[test]
fn non_finite_inline_box_edges_use_zero() {
    expect_layout_continues(
        input(
            ParagraphStyle::default(),
            vec![InlineBoxSpan::with_edges(text_range(0, 1), f32::NAN, 0.0)],
            Vec::new(),
            TiqianTextContent::new(Text::from("甲乙")),
        ),
    );
    expect_layout_continues(
        input(
            ParagraphStyle::default(),
            vec![InlineBoxSpan::with_edges(text_range(0, 1), 0.0, f32::INFINITY)],
            Vec::new(),
            TiqianTextContent::new(Text::from("甲乙")),
        ),
    );
}

#[test]
fn invalid_line_break_spans_have_no_effect() {
    let empty = TiqianTextContent::builder(Text::from("甲乙"))
        .line_break_spans(vec![LineBreakSpan {
            range: text_range(0, 0),
            policy: LineBreakPolicy::ProgressiveTechnical,
        }])
        .build();
    expect_layout_continues(input(ParagraphStyle::default(), Vec::new(), Vec::new(), empty));
    let out_of_bounds = TiqianTextContent::builder(Text::from("甲乙"))
        .line_break_spans(vec![LineBreakSpan {
            range: text_range(2, 3),
            policy: LineBreakPolicy::ProgressiveTechnical,
        }])
        .build();
    expect_layout_continues(
        input(ParagraphStyle::default(), Vec::new(), Vec::new(), out_of_bounds),
    );
}

#[test]
fn invalid_auto_space_suppressed_ranges_have_no_effect() {
    let empty = TiqianTextContent::builder(Text::from("甲乙"))
        .auto_space_suppressed_ranges(vec![text_range(1, 1)])
        .build();
    expect_layout_continues(
        input(ParagraphStyle::default(), Vec::new(), Vec::new(), empty),
    );
    let out_of_bounds = TiqianTextContent::builder(Text::from("甲乙"))
        .auto_space_suppressed_ranges(vec![text_range(0, 8)])
        .build();
    expect_layout_continues(
        input(ParagraphStyle::default(), Vec::new(), Vec::new(), out_of_bounds),
    );
}

#[test]
fn duplicate_inline_object_ranges_keep_the_first_object() {
    let first = inline_object(
        text_range(0, 1),
        10.0,
        8.0,
        2.0,
        InlineObjectBoundaryAdjustment::FIXED,
        InlineObjectBoundaryAdjustment::FIXED,
    );
    let result = ParagraphLayoutEngineBuilder::new(Box::new(DeterministicStubFontBackend::default())).build().layout(
        input(
            ParagraphStyle::default(),
            Vec::new(),
            vec![
                first,
                inline_object(
                    text_range(0, 1),
                    99.0,
                    8.0,
                    2.0,
                    InlineObjectBoundaryAdjustment::FIXED,
                    InlineObjectBoundaryAdjustment::FIXED,
                ),
            ],
            TiqianTextContent::new(Text::from("甲乙")),
        ),
    );
    assert_eq!(1, result.debug.inline_object_decisions.len());
    assert_eq!(10.0, result.debug.inline_object_decisions[0].advance);
}

#[test]
fn overlapping_inline_object_ranges_keep_the_earlier_source_start() {
    let result = ParagraphLayoutEngineBuilder::new(Box::new(DeterministicStubFontBackend::default())).build().layout(
        input(
            ParagraphStyle::default(),
            Vec::new(),
            vec![
                inline_object(
                    text_range(0, 2),
                    10.0,
                    8.0,
                    2.0,
                    InlineObjectBoundaryAdjustment::FIXED,
                    InlineObjectBoundaryAdjustment::FIXED,
                ),
                inline_object(
                    text_range(1, 2),
                    99.0,
                    8.0,
                    2.0,
                    InlineObjectBoundaryAdjustment::FIXED,
                    InlineObjectBoundaryAdjustment::FIXED,
                ),
            ],
            TiqianTextContent::new(Text::from("甲乙")),
        ),
    );
    assert_eq!(1, result.debug.inline_object_decisions.len());
    assert_eq!(text_range(0, 2), result.debug.inline_object_decisions[0].range);
    assert_eq!(10.0, result.debug.inline_object_decisions[0].advance);
}

#[test]
fn invalid_inline_object_ranges_use_source_text() {
    for range in [text_range(1, 1), text_range(0, 9)] {
        expect_layout_continues(
            input(
                ParagraphStyle::default(),
                Vec::new(),
                vec![inline_object(
                    range,
                    10.0,
                    8.0,
                    2.0,
                    InlineObjectBoundaryAdjustment::FIXED,
                    InlineObjectBoundaryAdjustment::FIXED,
                )],
                TiqianTextContent::new(Text::from("甲乙")),
            ),
        );
    }
}

#[test]
fn invalid_inline_object_geometry_uses_zero_components() {
    for (advance, ascent, descent) in [
        (0.0, 8.0, 2.0),
        (f32::NAN, 8.0, 2.0),
        (10.0, -1.0, 2.0),
        (10.0, f32::NAN, 2.0),
        (10.0, 8.0, f32::NAN),
        (10.0, 8.0, -1.0),
    ] {
        expect_layout_continues(
            input(
                ParagraphStyle::default(),
                Vec::new(),
                vec![inline_object(
                    text_range(0, 1),
                    advance,
                    ascent,
                    descent,
                    InlineObjectBoundaryAdjustment::FIXED,
                    InlineObjectBoundaryAdjustment::FIXED,
                )],
                TiqianTextContent::new(Text::from("甲乙")),
            ),
        );
    }
}

#[test]
fn invalid_inline_object_leading_boundary_uses_fixed_boundary() {
    let shrink = InlineObjectBoundaryAdjustment::builder().shrink_capacity(0.5).build();
    expect_layout_continues(
        input(
            ParagraphStyle::default(),
            Vec::new(),
            vec![inline_object(
                text_range(0, 1),
                10.0,
                8.0,
                2.0,
                shrink,
                InlineObjectBoundaryAdjustment::FIXED,
            )],
            TiqianTextContent::new(Text::from("甲乙")),
        ),
    );
    let discard = InlineObjectBoundaryAdjustment::builder()
        .line_end_discardable_advance(0.5)
        .build();
    expect_layout_continues(
        input(
            ParagraphStyle::default(),
            Vec::new(),
            vec![inline_object(
                text_range(0, 1),
                10.0,
                8.0,
                2.0,
                discard,
                InlineObjectBoundaryAdjustment::FIXED,
            )],
            TiqianTextContent::new(Text::from("甲乙")),
        ),
    );
}

#[test]
fn oversized_inline_object_trailing_boundary_is_clamped() {
    let shrink = InlineObjectBoundaryAdjustment::builder().shrink_capacity(10.5).build();
    expect_layout_continues(
        input(
            ParagraphStyle::default(),
            Vec::new(),
            vec![inline_object(
                text_range(0, 1),
                10.0,
                8.0,
                2.0,
                InlineObjectBoundaryAdjustment::FIXED,
                shrink,
            )],
            TiqianTextContent::new(Text::from("甲乙")),
        ),
    );
    let discard = InlineObjectBoundaryAdjustment::builder()
        .line_end_discardable_advance(10.5)
        .build();
    expect_layout_continues(
        input(
            ParagraphStyle::default(),
            Vec::new(),
            vec![inline_object(
                text_range(0, 1),
                10.0,
                8.0,
                2.0,
                InlineObjectBoundaryAdjustment::FIXED,
                discard,
            )],
            TiqianTextContent::new(Text::from("甲乙")),
        ),
    );
}
