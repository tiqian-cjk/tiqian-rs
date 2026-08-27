use tiqian::org::tiqian::clreq::ClreqProfile::{ClreqProfile, ClreqProfileResolver};
use tiqian::org::tiqian::core::Geometry::LayoutConstraints;
use tiqian::org::tiqian::core::TextModel::{LayoutInput, LineLengthGrid, ParagraphStyle, TiqianTextContent};
use tiqian::org::tiqian::core::Units::Ic;
use tiqian::org::tiqian::layout::ParagraphLayoutEngine::{
    ExplainableStubParagraphLayoutEngine, ParagraphLayoutEngine,
};

struct TaiwanProfile;

impl ClreqProfileResolver for TaiwanProfile {
    fn resolve(&self, _: &tiqian::org::tiqian::core::TextModel::LayoutProfileId) -> ClreqProfile {
        ClreqProfile::taiwan_horizontal()
    }
}

fn layout(text: &str) -> tiqian::org::tiqian::core::LayoutModel::LayoutResult {
    ExplainableStubParagraphLayoutEngine::default().layout(
        LayoutInput::builder(
            TiqianTextContent::new(text.to_owned()),
            LayoutConstraints::with_defaults(320.0),
        )
        .paragraph_style(ParagraphStyle::builder().first_line_indent(Some(Ic::ZERO)).build())
        .build(),
    )
}

#[test]
fn engine_records_profile_fallback_geometry_and_line_end_ledger() {
    let result = layout("你好。");
    let punctuation = result.debug.punctuation_decisions.iter().find(|decision| decision.ch == '。').unwrap();
    assert_eq!(8.0, punctuation.body_width);
    assert_eq!(0.0, punctuation.leading_glue_natural);
    assert_eq!(8.0, punctuation.trailing_glue_natural);
    assert_eq!("ProfileGlueFallbackWithoutFontGeometry", punctuation.geometry_source);

    let geometry = result.debug.geometry_decisions.iter().find(|decision| decision.source_text == "。").unwrap();
    assert_eq!("PunctuationGeometryLedger", geometry.source);
    assert_eq!(8.0, geometry.trailing_glue_consumed);
    assert_eq!(8.0, geometry.resolved_advance);
    let trim = result.debug.line_edge_trim_decisions.iter().find(|decision| decision.cluster_range == geometry.range).unwrap();
    assert_eq!("trailing", trim.side);
    assert_eq!(8.0, trim.trim_amount);
    assert_eq!("LineEndHalfWidthPunctuation", trim.reason);
}

#[test]
fn taiwan_profile_centres_pause_stop_glue_and_trims_both_sides() {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.clreq_profile_resolver = Box::new(TaiwanProfile);
    let result = engine.layout(
        LayoutInput::builder(
            TiqianTextContent::new("你好。".to_owned()),
            LayoutConstraints::with_defaults(320.0),
        )
        .paragraph_style(ParagraphStyle::builder().first_line_indent(Some(Ic::ZERO)).build())
        .build(),
    );

    let punctuation = result.debug.punctuation_decisions.iter().find(|decision| decision.ch == '。').unwrap();
    assert_eq!(4.0, punctuation.leading_glue_natural);
    assert_eq!(4.0, punctuation.trailing_glue_natural);
    assert_eq!("Center", punctuation.anchor);
    let geometry = result.debug.geometry_decisions.iter().find(|decision| decision.source_text == "。").unwrap();
    assert_eq!(4.0, geometry.leading_glue_consumed);
    assert_eq!(4.0, geometry.trailing_glue_consumed);
    assert_eq!(8.0, geometry.resolved_advance);
    let trim = result.debug.line_edge_trim_decisions.first().unwrap();
    assert_eq!("both", trim.side);
    assert_eq!("LineEndCenteredPunctuationPairedCompression", trim.reason);
}

#[test]
fn adjacent_closing_and_pause_stop_compression_is_reflected_in_drawable_ledger() {
    let result = layout("你好」。");

    assert_eq!(48.0, result.lines[0].visual_width);
    assert_eq!(48.0, result.size.width);
    assert_eq!(8.0, result.clusters.iter().find(|cluster| cluster.text == "。").unwrap().advance);
    let spacing = result.debug.spacing_decisions.first().unwrap();
    assert_eq!('」', spacing.left_char);
    assert_eq!('。', spacing.right_char);
    assert_eq!(8.0, spacing.reduction);
    assert_eq!("collapse-adjacent-punctuation-inner-glue", spacing.reason);
}

#[test]
fn push_in_consumes_punctuation_glue_before_carrying_line_start_stop() {
    let result = ExplainableStubParagraphLayoutEngine::default().layout(
        LayoutInput::builder(
            TiqianTextContent::new("中文中。".to_owned()),
            LayoutConstraints::with_defaults(60.0),
        )
        .paragraph_style(
            ParagraphStyle::builder()
                .first_line_indent(Some(Ic::ZERO))
                .line_length_grid(LineLengthGrid::with_enabled(false))
                .build(),
        )
        .build(),
    );

    assert_eq!(1, result.lines.len());
    assert_eq!(Some("PushIn"), result.debug.line_decisions[0].repair.as_deref());
    let geometry = result.debug.geometry_decisions.iter().find(|decision| decision.source_text == "。").unwrap();
    assert_eq!(8.0, geometry.trailing_glue_consumed);
    assert_eq!(8.0, geometry.resolved_advance);
}
