use tiqian::core::geometry::{text_range, LayoutConstraints, TextRange};
use tiqian::core::text::Text;
use tiqian::core::text_model::{
    InlineBoxSpan, InlineObjectBoundaryAdjustment, InlineObjectSpan, LayoutInput, LineBreakPolicy,
    LineBreakSpan, ParagraphStyle, TiqianTextContent,
};
use tiqian::layout::paragraph_layout_engine::{
    ExplainableStubParagraphLayoutEngine, ParagraphLayoutEngine,
};

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

fn expect_rejection(input: LayoutInput, fragment: &str) {
    let error = std::panic::catch_unwind(|| {
        let mut engine = ExplainableStubParagraphLayoutEngine::default();
        engine.layout(input);
    })
    .expect_err("expected layout input rejection");
    let message = error
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| error.downcast_ref::<&str>().copied())
        .expect("panic message");
    assert!(message.contains(fragment), "{message}");
}

#[test]
fn emphasis_dot_gap_em_must_be_finite_and_non_negative() {
    let nan_style = ParagraphStyle::builder().emphasis_dot_gap_em(f32::NAN).build();
    expect_rejection(
        input(nan_style, Vec::new(), Vec::new(), TiqianTextContent::new(Text::from("甲乙"))),
        "emphasisDotGapEm",
    );
    let negative_style = ParagraphStyle::builder().emphasis_dot_gap_em(-0.1).build();
    expect_rejection(
        input(negative_style, Vec::new(), Vec::new(), TiqianTextContent::new(Text::from("甲乙"))),
        "emphasisDotGapEm",
    );
}

#[test]
fn inline_object_minimum_clearance_em_must_be_finite_and_non_negative() {
    let nan_style = ParagraphStyle::builder()
        .inline_object_minimum_clearance_em(f32::NAN)
        .build();
    expect_rejection(
        input(nan_style, Vec::new(), Vec::new(), TiqianTextContent::new(Text::from("甲乙"))),
        "inlineObjectMinimumClearanceEm",
    );
    let negative_style = ParagraphStyle::builder()
        .inline_object_minimum_clearance_em(-1.0)
        .build();
    expect_rejection(
        input(negative_style, Vec::new(), Vec::new(), TiqianTextContent::new(Text::from("甲乙"))),
        "inlineObjectMinimumClearanceEm",
    );
}

#[test]
fn inline_box_span_must_be_a_non_empty_in_bounds_range() {
    expect_rejection(
        input(
            ParagraphStyle::default(),
            vec![InlineBoxSpan::new(text_range(0, 0))],
            Vec::new(),
            TiqianTextContent::new(Text::from("甲乙")),
        ),
        "non-empty source range",
    );
    expect_rejection(
        input(
            ParagraphStyle::default(),
            vec![InlineBoxSpan::new(text_range(1, 9))],
            Vec::new(),
            TiqianTextContent::new(Text::from("甲乙")),
        ),
        "non-empty source range",
    );
}

#[test]
fn inline_box_span_must_have_finite_inline_edges() {
    expect_rejection(
        input(
            ParagraphStyle::default(),
            vec![InlineBoxSpan::with_edges(text_range(0, 1), f32::NAN, 0.0)],
            Vec::new(),
            TiqianTextContent::new(Text::from("甲乙")),
        ),
        "finite inline edges",
    );
    expect_rejection(
        input(
            ParagraphStyle::default(),
            vec![InlineBoxSpan::with_edges(text_range(0, 1), 0.0, f32::INFINITY)],
            Vec::new(),
            TiqianTextContent::new(Text::from("甲乙")),
        ),
        "finite inline edges",
    );
}

#[test]
fn line_break_spans_must_be_non_empty_in_bounds_ranges() {
    let empty = TiqianTextContent::builder(Text::from("甲乙"))
        .line_break_spans(vec![LineBreakSpan {
            range: text_range(0, 0),
            policy: LineBreakPolicy::ProgressiveTechnical,
        }])
        .build();
    expect_rejection(input(ParagraphStyle::default(), Vec::new(), Vec::new(), empty), "LineBreakSpan");
    let out_of_bounds = TiqianTextContent::builder(Text::from("甲乙"))
        .line_break_spans(vec![LineBreakSpan {
            range: text_range(2, 3),
            policy: LineBreakPolicy::ProgressiveTechnical,
        }])
        .build();
    expect_rejection(
        input(ParagraphStyle::default(), Vec::new(), Vec::new(), out_of_bounds),
        "LineBreakSpan",
    );
}

#[test]
fn auto_space_suppressed_ranges_must_be_non_empty_in_bounds() {
    let empty = TiqianTextContent::builder(Text::from("甲乙"))
        .auto_space_suppressed_ranges(vec![text_range(1, 1)])
        .build();
    expect_rejection(
        input(ParagraphStyle::default(), Vec::new(), Vec::new(), empty),
        "Auto-space suppressed range",
    );
    let out_of_bounds = TiqianTextContent::builder(Text::from("甲乙"))
        .auto_space_suppressed_ranges(vec![text_range(0, 8)])
        .build();
    expect_rejection(
        input(ParagraphStyle::default(), Vec::new(), Vec::new(), out_of_bounds),
        "Auto-space suppressed range",
    );
}

#[test]
fn inline_object_ranges_must_be_unique() {
    let object = inline_object(
        text_range(0, 1),
        10.0,
        8.0,
        2.0,
        InlineObjectBoundaryAdjustment::FIXED,
        InlineObjectBoundaryAdjustment::FIXED,
    );
    expect_rejection(
        input(
            ParagraphStyle::default(),
            Vec::new(),
            vec![object.clone(), object],
            TiqianTextContent::new(Text::from("甲乙")),
        ),
        "unique",
    );
}

#[test]
fn inline_object_ranges_must_not_overlap() {
    expect_rejection(
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
                    10.0,
                    8.0,
                    2.0,
                    InlineObjectBoundaryAdjustment::FIXED,
                    InlineObjectBoundaryAdjustment::FIXED,
                ),
            ],
            TiqianTextContent::new(Text::from("甲乙")),
        ),
        "overlap",
    );
}

#[test]
fn inline_object_must_cover_a_non_empty_in_bounds_range() {
    for range in [text_range(1, 1), text_range(0, 9)] {
        expect_rejection(
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
            "non-empty source range",
        );
    }
}

#[test]
fn inline_object_must_have_finite_positive_geometry() {
    for (advance, ascent, descent) in [
        (0.0, 8.0, 2.0),
        (f32::NAN, 8.0, 2.0),
        (10.0, -1.0, 2.0),
        (10.0, f32::NAN, 2.0),
        (10.0, 8.0, f32::NAN),
        (10.0, 8.0, -1.0),
    ] {
        expect_rejection(
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
            "finite positive geometry",
        );
    }
}

#[test]
fn inline_object_leading_boundary_must_be_fixed() {
    let shrink = InlineObjectBoundaryAdjustment::builder().shrink_capacity(0.5).build();
    expect_rejection(
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
        "cannot shrink its leading boundary",
    );
    let discard = InlineObjectBoundaryAdjustment::builder()
        .line_end_discardable_advance(0.5)
        .build();
    expect_rejection(
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
        "cannot discard advance at its leading boundary",
    );
}

#[test]
fn inline_object_trailing_boundary_must_not_exceed_advance() {
    let shrink = InlineObjectBoundaryAdjustment::builder().shrink_capacity(10.5).build();
    expect_rejection(
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
        "trailing shrink capacity",
    );
    let discard = InlineObjectBoundaryAdjustment::builder()
        .line_end_discardable_advance(10.5)
        .build();
    expect_rejection(
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
        "trailing line-end discard",
    );
}
