use tiqian::clreq::clreq_profile::{
    CjkPunctuationGlyphPolicy, ClreqProfile, ClreqProfileResolver,
};
use tiqian::core::geometry::{LayoutConstraints, Rect};
use tiqian::core::layout_model::{Glyph, GlyphRun};
use tiqian::core::text::Text;
use tiqian::core::text_model::{LayoutInput, ParagraphStyle, TiqianTextContent};
use tiqian::core::units::Ic;
use tiqian::layout::paragraph_layout_engine::{
    ExplainableStubParagraphLayoutEngine, ParagraphLayoutEngine,
};
use tiqian::shaping::text_shaper::{
    ExplainableStubTextShaper, ShapingInput, ShapingResult, TextShaper,
};

struct PreserveInputProfile;

impl ClreqProfileResolver for PreserveInputProfile {
    fn resolve(&self, _: &tiqian::core::text_model::LayoutProfileId) -> ClreqProfile {
        let mut profile = ClreqProfile::mainland_horizontal();
        profile.punctuation_glyph_policy = CjkPunctuationGlyphPolicy::PreserveInput;
        profile
    }
}

struct HaltInkTextShaper;

impl TextShaper for HaltInkTextShaper {
    fn shape(&self, input: &ShapingInput) -> ShapingResult {
        let result = ExplainableStubTextShaper.shape(input);
        let source_text = input.text.slice_text(input.range);
        let is_interpunct = source_text == "·" || source_text == "・";
        let is_ellipsis = source_text == "…";
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
                                    .bounds(Some(if is_ellipsis {
                                        Rect {
                                            left: 2.0,
                                            top: 2.0,
                                            right: 10.0,
                                            bottom: 10.0,
                                        }
                                    } else {
                                        Rect {
                                            left: 4.0,
                                            top: 2.0,
                                            right: 12.0,
                                            bottom: 10.0,
                                        }
                                    }))
                                    .halt_advance(if is_interpunct || is_ellipsis {
                                        Some(8.0)
                                    } else {
                                        glyph.halt_advance
                                    })
                                    .halt_placement_x(if is_interpunct {
                                        Some(-4.0)
                                    } else if is_ellipsis {
                                        Some(0.0)
                                    } else {
                                        glyph.halt_placement_x
                                    })
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

fn layout(text: &str) -> tiqian::core::layout_model::LayoutResult {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.text_shaper = Box::new(HaltInkTextShaper);
    engine.layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from(text)),
            LayoutConstraints::with_defaults(320.0),
        )
        .paragraph_style(ParagraphStyle::builder().first_line_indent(Some(Ic::ZERO)).build())
        .build(),
    )
}

#[test]
fn interpunct_ink_evidence_frees_paired_glue_for_tier_three_shrink() {
    let result = layout("正文·间隔号·后文…结尾");

    let dots: Vec<_> = result
        .debug
        .punctuation_decisions
        .iter()
        .filter(|decision| decision.ch == '·')
        .collect();
    assert_eq!(2, dots.len());
    for dot in dots {
        assert!(dot.leading_glue_natural > 0.0);
        assert!(dot.trailing_glue_natural > 0.0);
        assert_eq!("Center", dot.anchor.as_str());
        assert_eq!("FontHaltFittedBodyCompression", dot.geometry_source);
    }
    let ellipsis = result
        .debug
        .punctuation_decisions
        .iter()
        .find(|decision| decision.punctuation_class == "Ellipsis")
        .unwrap();
    assert_eq!(0.0, ellipsis.leading_glue_natural);
    assert!(ellipsis.trailing_glue_natural > 0.0);
    assert!(!result.lines.is_empty());
}

#[test]
fn preserved_interpunct_codepoint_keeps_interpunct_class_for_tier_three_shrink() {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.clreq_profile_resolver = Box::new(PreserveInputProfile);
    engine.text_shaper = Box::new(HaltInkTextShaper);
    let result = engine.layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from("正文・间隔・后文")),
            LayoutConstraints::with_defaults(320.0),
        )
        .paragraph_style(ParagraphStyle::builder().first_line_indent(Some(Ic::ZERO)).build())
        .build(),
    );

    let interpuncts: Vec<_> = result
        .debug
        .punctuation_decisions
        .iter()
        .filter(|decision| decision.punctuation_class == "Interpunct")
        .collect();
    assert_eq!(vec!['・', '・'], interpuncts.iter().map(|decision| decision.ch).collect::<Vec<_>>());
    for dot in interpuncts {
        assert!(dot.leading_glue_natural > 0.0);
        assert!(dot.trailing_glue_natural > 0.0);
        assert_eq!("Center", dot.anchor.as_str());
    }
    assert!(!result.lines.is_empty());
}
