use tiqian::core::geometry::{LayoutConstraints, TextRange};
use tiqian::core::text::Text;
use tiqian::core::text_model::{
    DecorationKind, DecorationSpan, LayoutInput, ParagraphStyle, TiqianTextContent,
};
use tiqian::core::units::Ic;
use tiqian::layout::paragraph_layout_engine::{
    ExplainableStubParagraphLayoutEngine, ParagraphLayoutEngine,
};

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
                range: TextRange::new(0, 2),
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
        range: TextRange::new(0, 4),
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
