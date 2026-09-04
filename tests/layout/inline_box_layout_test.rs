use tiqian::core::geometry::{text_range, LayoutConstraints};
use tiqian::core::layout_queries::positioned_clusters;
use tiqian::core::text::Text;
use tiqian::core::text_model::{
    InlineBoxOuterSpacing, InlineBoxSpan, LayoutInput, LineLengthGrid, ParagraphStyle,
    TiqianTextContent,
};
use tiqian::core::units::Ic;
use tiqian::layout::paragraph_layout_engine::{
    ExplainableStubParagraphLayoutEngine, ParagraphLayoutEngine,
};

fn layout(text: &str, boxes: Vec<InlineBoxSpan>) -> tiqian::core::layout_model::LayoutResult {
    ExplainableStubParagraphLayoutEngine::default().layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from(text)),
            LayoutConstraints::with_defaults(400.0),
        )
        .paragraph_style(
            ParagraphStyle::builder()
                .first_line_indent(Some(Ic::ZERO))
                .line_length_grid(LineLengthGrid::with_enabled(false))
                .build(),
        )
        .inline_boxes(boxes)
        .build(),
    )
}

#[test]
fn inline_edges_reserve_advance_and_move_glyph_origin() {
    let plain = layout("中。", Vec::new());
    let boxed = layout(
        "中。",
        vec![InlineBoxSpan::with_all(
            text_range(1, 2),
            3.0,
            5.0,
            InlineBoxOuterSpacing::Source,
        )],
    );
    let plain_stop = plain
        .clusters
        .iter()
        .find(|cluster| cluster.range == text_range(1, 2))
        .unwrap();
    let boxed_stop = boxed
        .clusters
        .iter()
        .find(|cluster| cluster.range == text_range(1, 2))
        .unwrap();
    let positioned = positioned_clusters(&boxed)
        .into_iter()
        .find(|cluster| cluster.range == text_range(1, 2))
        .unwrap();

    assert!((boxed_stop.advance - plain_stop.advance - 8.0).abs() < 0.001);
    assert_eq!(3.0, boxed_stop.leading_layout_advance);
    assert!((positioned.draw_x - positioned.left - 3.0).abs() < 0.001);
    assert_eq!(
        "InlineBoxBoundaryAdvance",
        boxed.debug.inline_box_decisions[0].reason
    );
}

#[test]
fn narrow_outer_spacing_inserts_gap_but_source_mode_does_not() {
    let narrow = layout(
        "中./中",
        vec![InlineBoxSpan::with_all(
            text_range(1, 3),
            3.0,
            5.0,
            InlineBoxOuterSpacing::Narrow,
        )],
    );
    let reasons = narrow
        .debug
        .auto_space_decisions
        .iter()
        .map(|decision| decision.reason.as_str())
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(
        std::collections::HashSet::from([
            "InlineBoxOuterAutoSpace:leading-W-N",
            "InlineBoxOuterAutoSpace:trailing-N-W",
        ]),
        reasons,
    );
    assert!(
        narrow
            .debug
            .auto_space_decisions
            .iter()
            .all(|decision| decision.boundary_role == "InlineBox.Narrow")
    );
    assert_eq!("Narrow", narrow.debug.inline_box_decisions[0].outer_spacing);

    let source = layout(
        "中./中",
        vec![InlineBoxSpan::with_all(
            text_range(1, 3),
            3.0,
            5.0,
            InlineBoxOuterSpacing::Source,
        )],
    );
    assert!(source.debug.auto_space_decisions.is_empty());
    assert_eq!("Source", source.debug.inline_box_decisions[0].outer_spacing);
}
