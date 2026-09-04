use tiqian::core::geometry::{LayoutConstraints, Rect};
use tiqian::core::layout_model::{Glyph, GlyphRun};
use tiqian::core::text::Text;
use tiqian::core::text_model::{
    InlineAttachment, LayoutInput, RubySpan, TextSpan, TextStyle, TiqianTextContent,
};
use tiqian::layout::paragraph_layout_engine::{
    ExplainableStubParagraphLayoutEngine, ParagraphLayoutEngine,
};
use tiqian::shaping::text_shaper::{
    ExplainableStubTextShaper, ShapingInput, ShapingResult, TextShaper,
};

fn layout(
    text: &str,
    constraints: LayoutConstraints,
    spans: Vec<TextSpan>,
    ruby_spans: Vec<RubySpan>,
    text_shaper: Box<dyn TextShaper>,
) -> tiqian::core::layout_model::LayoutResult {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.text_shaper = text_shaper;
    engine.layout(
        LayoutInput::builder(
            TiqianTextContent::builder(Text::from(text)).spans(spans).build(),
            constraints,
        )
        .ruby_spans(ruby_spans)
        .build(),
    )
}

struct CenteredInkTextShaper;

impl TextShaper for CenteredInkTextShaper {
    fn shape(&self, input: &ShapingInput) -> ShapingResult {
        let result = ExplainableStubTextShaper.shape(input);
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
                                Glyph::builder(glyph.id, glyph.cluster_range, glyph.advance)
                                    .x(glyph.x)
                                    .y(glyph.y)
                                    .render_font_key(glyph.render_font_key)
                                    .bounds(Some(Rect {
                                        left: 4.0,
                                        top: 2.0,
                                        right: 12.0,
                                        bottom: 10.0,
                                    }))
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
fn max_lines_caps_visible_lines_to_one() {
    let text = "中文排版引擎测试文本，用于验证多行截断行为是否正确工作并继续延伸。";
    let unrestricted = layout(
        text,
        LayoutConstraints::with_defaults(64.0),
        Vec::new(),
        Vec::new(),
        Box::new(ExplainableStubTextShaper),
    );
    assert!(unrestricted.lines.len() > 1);
    let capped = layout(
        text,
        LayoutConstraints::with_max_lines(64.0, 1),
        Vec::new(),
        Vec::new(),
        Box::new(ExplainableStubTextShaper),
    );
    assert_eq!(1, capped.lines.len());
}

#[test]
fn empty_text_produces_no_visible_lines() {
    let result = layout(
        "",
        LayoutConstraints::with_defaults(100.0),
        Vec::new(),
        Vec::new(),
        Box::new(ExplainableStubTextShaper),
    );

    assert!(result.lines.is_empty());
}

#[test]
fn pure_latin_paragraph_still_produces_lines() {
    let result = layout(
        "hello justified world",
        LayoutConstraints::with_defaults(96.0),
        Vec::new(),
        Vec::new(),
        Box::new(ExplainableStubTextShaper),
    );

    assert!(!result.lines.is_empty());
    assert!(result.lines[0].natural_width > 0.0);
}

#[test]
fn ruby_base_range_crossing_cluster_boundaries_is_skipped() {
    let result = layout(
        "中文测试",
        LayoutConstraints::with_defaults(320.0),
        Vec::new(),
        vec![
            RubySpan::new(tiqian::core::geometry::text_range(0, 2), Text::from("zhōng")),
            RubySpan::new(tiqian::core::geometry::text_range(1, 3), Text::from("wén")),
        ],
        Box::new(ExplainableStubTextShaper),
    );

    assert_eq!(2, result.debug.ruby_decisions.len());
}

#[test]
fn space_runs_resolve_both_wide_narrow_orders() {
    let cjk_first = layout(
        "中文 abc",
        LayoutConstraints::with_defaults(320.0),
        Vec::new(),
        Vec::new(),
        Box::new(ExplainableStubTextShaper),
    );
    assert_eq!(1, cjk_first.lines.len());
    assert!(cjk_first.lines[0].natural_width > 0.0);

    let latin_first = layout(
        "abc 中文",
        LayoutConstraints::with_defaults(320.0),
        Vec::new(),
        Vec::new(),
        Box::new(ExplainableStubTextShaper),
    );
    assert_eq!(1, latin_first.lines.len());
    assert!(latin_first.lines[0].natural_width > 0.0);
}

#[test]
fn attached_reference_at_source_end_lays_out_without_virtual_boundary() {
    let text = "正文：“内容·[1]";
    let byte_start = text.find("[1]").unwrap();
    let start = text[..byte_start].chars().count() as i32;
    let result = layout(
        text,
        LayoutConstraints::with_defaults(320.0),
        vec![TextSpan {
            range: tiqian::core::geometry::text_range(start, start + 3),
            style: TextStyle::builder()
                .inline_attachment(InlineAttachment::Previous)
                .build(),
        }],
        Vec::new(),
        Box::new(ExplainableStubTextShaper),
    );

    assert!(result
        .debug
        .spacing_decisions
        .iter()
        .all(|decision| !decision.reason.starts_with("AttachedInlineVirtual")));
    let collapse = result
        .debug
        .spacing_decisions
        .iter()
        .find(|decision| decision.left_char == '：' && decision.right_char == '“')
        .unwrap();
    assert!(collapse.reduction > 0.0);
}

#[test]
fn centered_ink_punctuation_keeps_paired_glue() {
    let wide = layout(
        "中·文",
        LayoutConstraints::with_defaults(320.0),
        Vec::new(),
        Vec::new(),
        Box::new(CenteredInkTextShaper),
    );
    assert_eq!(1, wide.lines.len());
    let tight = layout(
        "文·本，内容。",
        LayoutConstraints::with_defaults(60.0),
        Vec::new(),
        Vec::new(),
        Box::new(CenteredInkTextShaper),
    );
    assert!(tight.lines.len() > 1);
}
