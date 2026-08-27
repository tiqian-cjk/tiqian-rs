use tiqian::org::tiqian::core::Geometry::{LayoutConstraints, TextRange};
use tiqian::org::tiqian::core::TextModel::{
    LayoutInput, LineBreakPolicy, LineBreakSpan, LineLengthGrid, ParagraphStyle, TiqianTextContent,
};
use tiqian::org::tiqian::core::Units::Ic;
use tiqian::org::tiqian::layout::ParagraphLayoutEngine::{
    ExplainableStubParagraphLayoutEngine, ParagraphLayoutEngine,
};
use tiqian::org::tiqian::linebreak::Hyphenation::NoHyphenator;

fn layout(
    text: &str,
    max_width: f32,
    spans: Vec<LineBreakSpan>,
    no_hyphenation: bool,
) -> tiqian::org::tiqian::core::LayoutModel::LayoutResult {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    if no_hyphenation {
        engine.hyphenator = &NoHyphenator;
    }
    engine.layout(
        LayoutInput::builder(
            TiqianTextContent::builder(text.to_owned()).line_break_spans(spans).build(),
            LayoutConstraints::with_defaults(max_width),
        )
        .paragraph_style(
            ParagraphStyle::builder()
                .first_line_indent(Some(Ic::ZERO))
                .line_length_grid(LineLengthGrid::with_enabled(false))
                .build(),
        )
        .build(),
    )
}

fn line_text(result: &tiqian::org::tiqian::core::LayoutModel::LayoutResult, index: usize) -> String {
    let line = &result.lines[index];
    result.clusters[line.cluster_range.first() as usize..=line.cluster_range.last() as usize]
        .iter()
        .map(|cluster| cluster.text.as_str())
        .collect()
}

#[test]
fn camel_case_token_breaks_at_hump_without_synthetic_hyphen() {
    let result = layout("PowerPoint", 128.0, Vec::new(), false);

    assert_eq!(2, result.lines.len());
    assert!(result.lines.iter().all(|line| line.hyphen_advance == 0.0));
    assert!(result.clusters.iter().any(|cluster| cluster.text == "Power"));
    assert!(result.clusters.iter().any(|cluster| cluster.text == "Point"));
}

#[test]
fn existing_hyphen_break_does_not_add_synthetic_hyphen() {
    let result = layout("out-of-the-way", 128.0, Vec::new(), false);

    assert_eq!(2, result.lines.len());
    assert!(result.lines.iter().all(|line| line.hyphen_advance == 0.0));
    assert!(result.clusters.iter().any(|cluster| cluster.text == "out-"));
    assert!(result.clusters.iter().any(|cluster| cluster.text == "way"));
    assert!(line_text(&result, 0).ends_with('-'));
}

#[test]
fn url_separator_break_keeps_solidus_with_preceding_piece() {
    let result = layout("TeX/LaTeX", 80.0, Vec::new(), false);

    assert!(result.clusters.iter().any(|cluster| cluster.text == "TeX/"));
    assert!(result.clusters.iter().any(|cluster| cluster.text == "LaTeX"));
    assert_eq!("TeX/", line_text(&result, 0));
    assert_eq!("LaTeX", line_text(&result, 1));
    assert!(result.lines.iter().all(|line| line.hyphen_advance == 0.0));
}

#[test]
fn opaque_token_hard_breaks_without_synthetic_hyphen() {
    let token = "abc123def456ghi789";
    let result = layout(token, 96.0, Vec::new(), true);

    assert!(result.lines.len() > 1);
    assert!(result.lines.iter().all(|line| line.hyphen_advance == 0.0));
    assert!(result.clusters.iter().all(|cluster| cluster.text != token));
    assert!(result.lines.iter().all(|line| line.visual_width <= 96.0));
}

#[test]
fn progressive_technical_break_uses_emergency_tracking_instead_of_cjk_stretch() {
    let text = "中文abcdefghij";
    let technical_range = TextRange::new(2, text.encode_utf16().count() as i32);
    let technical = LineBreakSpan {
        range: technical_range,
        policy: LineBreakPolicy::ProgressiveTechnical,
    };
    let result = layout(text, 104.0, vec![technical], true);

    assert_eq!(6, result.lines[0].range.end());
    assert_eq!(0.0, result.lines[0].hyphen_advance);
    assert!(result.debug.line_decisions[0].notes.iter().any(|note| note == "technical-break:Emergency"));
    let adjustment = result.debug.justification_decisions.iter()
        .find(|decision| decision.line_range == result.lines[0].range)
        .expect("expected justification decision for technical line");
    assert!(adjustment.allocations.iter().all(|allocation| allocation.kind != "CjkInterChar"));
    assert!(adjustment.allocations.iter().any(|allocation| {
        allocation.kind == "EmergencyGraphemeTracking" && allocation.cluster_range.start() >= technical_range.start()
    }));
    assert!(adjustment.deficit_after.abs() < 0.001);
}
