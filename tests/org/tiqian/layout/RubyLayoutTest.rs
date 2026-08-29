use tiqian::core::Geometry::{LayoutConstraints, TextRange};
use tiqian::core::Text::Text;
use tiqian::core::TextModel::{
    LayoutInput, ParagraphStyle, RubyLineHeightMode, RubySpan, TiqianTextContent,
};
use tiqian::core::Units::Ic;
use tiqian::layout::ParagraphLayoutEngine::{
    ExplainableStubParagraphLayoutEngine, ParagraphLayoutEngine,
};

fn layout(
    text: &str,
    max_width: f32,
    style: ParagraphStyle,
    ruby_spans: Vec<RubySpan>,
) -> tiqian::core::LayoutModel::LayoutResult {
    ExplainableStubParagraphLayoutEngine::default().layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from(text)),
            LayoutConstraints::with_defaults(max_width),
        )
        .paragraph_style(style)
        .ruby_spans(ruby_spans)
        .build(),
    )
}

#[test]
fn ruby_uses_existing_interline_space_without_changing_line_box() {
    let style = ParagraphStyle::builder()
        .first_line_indent(Some(Ic::ZERO))
        .build();
    let plain = layout("中文排版", 400.0, style.clone(), Vec::new());
    let ruby = layout(
        "中文排版",
        400.0,
        style,
        vec![RubySpan::new(TextRange::new(0, 1), Text::from("zhōng"))],
    );

    assert_eq!(plain.lines[0].top, ruby.lines[0].top);
    assert_eq!(plain.lines[0].baseline, ruby.lines[0].baseline);
    assert_eq!(plain.lines[0].bottom, ruby.lines[0].bottom);
    assert_eq!(plain.size.height, ruby.size.height);
    let decision = ruby.debug.ruby_line_height_decision.as_ref().unwrap();
    assert_eq!("PerLine", decision.mode);
    assert_eq!(0.0, decision.max_extra);
    assert!(decision.line_extras.iter().all(|extra| *extra == 0.0));
    assert_eq!("ExistingInterlineSpaceFitsRuby", decision.reason);
    let annotation = &ruby.debug.ruby_decisions[0];
    assert_eq!("zhōng", annotation.text);
    assert!(annotation.center_x >= 0.0 && annotation.center_x <= ruby.clusters[0].advance);
    assert!(annotation.baseline_y < ruby.lines[0].baseline);
    assert_eq!(500, annotation.font_weight);
}

#[test]
fn per_line_mode_expands_only_the_annotated_line_when_ruby_collides() {
    let style = ParagraphStyle::builder()
        .first_line_indent(Some(Ic::ZERO))
        .line_height(Some(18.0))
        .build();
    let plain = layout("甲乙丙丁戊己庚辛壬癸子丑", 64.0, style.clone(), Vec::new());
    let annotated = layout(
        "甲乙丙丁戊己庚辛壬癸子丑",
        64.0,
        style,
        vec![RubySpan::new(TextRange::new(4, 5), Text::from("wù"))],
    );

    assert_eq!(plain.size.height + 6.0, annotated.size.height);
    assert_eq!(18.0, annotated.lines[0].bottom - annotated.lines[0].top);
    assert_eq!(24.0, annotated.lines[1].bottom - annotated.lines[1].top);
    assert_eq!(18.0, annotated.lines[2].bottom - annotated.lines[2].top);
    let decision = annotated.debug.ruby_line_height_decision.as_ref().unwrap();
    assert_eq!("PerLine", decision.mode);
    assert_eq!(6.0, decision.max_extra);
    assert_eq!(vec![0.0, 6.0, 0.0], decision.line_extras);
    assert_eq!(vec![1], decision.expanded_line_indices);
}

#[test]
fn uniform_paragraph_mode_expands_every_line_by_same_ruby_deficit() {
    let style = ParagraphStyle::builder()
        .first_line_indent(Some(Ic::ZERO))
        .line_height(Some(18.0))
        .ruby_line_height_mode(RubyLineHeightMode::UniformParagraph)
        .build();
    let result = layout(
        "甲乙丙丁戊己庚辛壬癸子丑",
        64.0,
        style,
        vec![RubySpan::new(TextRange::new(4, 5), Text::from("wù"))],
    );

    assert_eq!(3, result.lines.len());
    assert!(
        result
            .lines
            .iter()
            .all(|line| (line.bottom - line.top - 24.0).abs() < 0.001)
    );
    assert_eq!(72.0, result.size.height);
    let decision = result.debug.ruby_line_height_decision.as_ref().unwrap();
    assert_eq!("UniformParagraph", decision.mode);
    assert_eq!(6.0, decision.max_extra);
    assert_eq!(vec![6.0, 6.0, 6.0], decision.line_extras);
    assert_eq!(vec![0, 1, 2], decision.expanded_line_indices);
}
