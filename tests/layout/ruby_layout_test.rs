use tiqian::core::geometry::{LayoutConstraints, Rect, TextRange};
use tiqian::core::layout_model::GlyphRun;
use tiqian::core::text::Text;
use tiqian::core::text_model::{
    LayoutInput, ParagraphStyle, RubyLineHeightMode, RubySpan, TiqianTextContent,
};
use tiqian::core::units::Ic;
use tiqian::layout::paragraph_layout_engine::{
    ExplainableStubParagraphLayoutEngine, ParagraphLayoutEngine,
};
use tiqian::shaping::text_shaper::{
    ExplainableStubTextShaper, ShapingInput, ShapingResult, TextShaper,
};

fn layout(
    text: &str,
    max_width: f32,
    style: ParagraphStyle,
    ruby_spans: Vec<RubySpan>,
) -> tiqian::core::layout_model::LayoutResult {
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

struct ContradictoryRubyInkTextShaper;

impl TextShaper for ContradictoryRubyInkTextShaper {
    fn shape(&self, input: &ShapingInput) -> ShapingResult {
        let result = ExplainableStubTextShaper.shape(input);
        let bounds = if input.display_text == "pg" {
            Rect {
                left: 0.0,
                top: -100.0,
                right: 16.0,
                bottom: 100.0,
            }
        } else {
            Rect {
                left: 0.0,
                top: -1.0,
                right: 16.0,
                bottom: 1.0,
            }
        };
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
                                tiqian::core::layout_model::Glyph::builder(
                                    glyph.id,
                                    glyph.cluster_range,
                                    glyph.advance,
                                )
                                .x(glyph.x)
                                .y(glyph.y)
                                .render_font_key(glyph.render_font_key)
                                .bounds(Some(bounds))
                                .halt_advance(glyph.halt_advance)
                                .halt_placement_x(glyph.halt_placement_x)
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

#[test]
fn ruby_on_one_line_keeps_the_whole_baseline_grid_stable() {
    let style = ParagraphStyle::builder().first_line_indent(Some(Ic::ZERO)).build();
    let plain = layout("甲乙丙丁戊己庚辛", 64.0, style.clone(), Vec::new());
    let annotated = layout(
        "甲乙丙丁戊己庚辛",
        64.0,
        style,
        vec![RubySpan::new(TextRange::new(4, 5), Text::from("wù"))],
    );
    assert_eq!(plain.lines.len(), annotated.lines.len());
    assert_eq!(plain.size.height, annotated.size.height);
    for (plain_line, ruby_line) in plain.lines.iter().zip(&annotated.lines) {
        assert_eq!(plain_line.top, ruby_line.top);
        assert_eq!(plain_line.baseline, ruby_line.baseline);
        assert_eq!(plain_line.bottom, ruby_line.bottom);
    }
    assert!((annotated.lines[1].baseline - annotated.lines[0].baseline - 24.0).abs() < 0.001);
}

#[test]
fn ruby_vertical_geometry_uses_metrics_not_reading_ink() {
    let style = ParagraphStyle::builder()
        .first_line_indent(Some(Ic::ZERO))
        .line_height(Some(18.0))
        .build();
    let layout_with_ink = |reading| {
        let mut engine = ExplainableStubParagraphLayoutEngine::default();
        engine.text_shaper = Box::new(ContradictoryRubyInkTextShaper);
        engine.layout(
            LayoutInput::builder(
                TiqianTextContent::new(Text::from("甲乙丙丁")),
                LayoutConstraints::with_defaults(64.0),
            )
            .paragraph_style(style.clone())
            .ruby_spans(vec![RubySpan::new(TextRange::new(0, 1), Text::from(reading))])
            .build(),
        )
    };
    let shallow = layout_with_ink("he");
    let descender = layout_with_ink("pg");
    assert_eq!(shallow.size.height, descender.size.height);
    assert_eq!(shallow.lines[0].top, descender.lines[0].top);
    assert_eq!(shallow.lines[0].baseline, descender.lines[0].baseline);
    assert_eq!(shallow.lines[0].bottom, descender.lines[0].bottom);
    let a = &shallow.debug.ruby_decisions[0];
    let b = &descender.debug.ruby_decisions[0];
    assert_eq!(a.baseline_y, b.baseline_y);
    assert_eq!(a.ascent, b.ascent);
    assert_eq!(a.descent, b.descent);
    assert_eq!(
        shallow
            .debug
            .ruby_line_height_decision
            .as_ref()
            .unwrap()
            .ruby_extent,
        descender
            .debug
            .ruby_line_height_decision
            .as_ref()
            .unwrap()
            .ruby_extent
    );
}

#[test]
fn no_ruby_is_unchanged() {
    let result = layout(
        "中文排版",
        400.0,
        ParagraphStyle::builder().first_line_indent(Some(Ic::ZERO)).build(),
        Vec::new(),
    );
    assert!(result.debug.ruby_decisions.is_empty());
}

#[test]
fn wide_adjacent_readings_spread_but_narrow_do_not() {
    let total_width = |readings: [&str; 4]| {
        layout(
            "中文排版",
            4000.0,
            ParagraphStyle::builder().first_line_indent(Some(Ic::ZERO)).build(),
            readings
                .iter()
                .enumerate()
                .map(|(index, reading)| RubySpan::new(TextRange::new(index as i32, index as i32 + 1), Text::from(*reading)))
                .collect(),
        )
        .clusters
        .iter()
        .map(|cluster| cluster.advance)
        .sum::<f32>()
    };
    let plain = total_width(["", "", "", ""]);
    let narrow = total_width(["yī", "rén", "yī", "rén"]);
    let wide = total_width(["zhuāng", "chuáng", "shuāng", "guāng"]);
    assert!(narrow >= plain, "spread never shrinks the line ({narrow} vs {plain})");
    assert!(wide > narrow, "wider readings spread more ({wide} vs {narrow})");
}
