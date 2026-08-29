use tiqian::core::geometry::LayoutConstraints;
use tiqian::core::text::Text;
use tiqian::core::text_model::{LayoutInput, ParagraphStyle, TiqianTextContent};
use tiqian::core::units::Ic;
use tiqian::layout::paragraph_layout_engine::{
    ExplainableStubParagraphLayoutEngine, ParagraphLayoutEngine,
};

fn layout(text: &str) -> tiqian::core::layout_model::LayoutResult {
    ExplainableStubParagraphLayoutEngine::default().layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from(text)),
            LayoutConstraints::with_defaults(320.0),
        )
        .paragraph_style(
            ParagraphStyle::builder()
                .first_line_indent(Some(Ic::ZERO))
                .build(),
        )
        .build(),
    )
}

#[test]
fn latin_technical_punctuation_stays_in_latin_run() {
    let result = layout("well-known/path");

    assert_eq!(
        "well-known/path",
        result
            .clusters
            .iter()
            .map(|cluster| cluster.text.as_str())
            .collect::<String>()
    );
    assert!(
        result
            .clusters
            .iter()
            .all(|cluster| cluster.font_key == "latin-primary")
    );
    assert!(
        result
            .clusters
            .iter()
            .any(|cluster| cluster.text == "well-")
    );
}

#[test]
fn ascii_brackets_remain_latin_inside_cjk_text() {
    let result = layout("中文(中文)");

    for bracket in ["(", ")"] {
        let cluster = result
            .clusters
            .iter()
            .find(|cluster| cluster.text == bracket)
            .unwrap();
        assert_eq!("latin-primary", cluster.font_key, "{bracket}");
        let decision = result
            .debug
            .font_decisions
            .iter()
            .find(|decision| decision.range == cluster.range)
            .unwrap();
        assert_eq!("LatinText", decision.role, "{bracket}");
    }
}

#[test]
fn western_quote_pair_reaches_latin_font_pipeline_without_cjk_geometry() {
    let result = layout("“Hello” world");

    assert_eq!(3, result.clusters.len());
    assert_eq!("“Hello”", result.clusters[0].text);
    assert_eq!("latin-primary", result.clusters[0].font_key);
    let overrides = result
        .debug
        .role_overrides
        .iter()
        .filter(|override_info| matches!(override_info.range.start(), 0 | 6))
        .collect::<Vec<_>>();
    assert_eq!(2, overrides.len());
    assert!(
        overrides
            .iter()
            .all(|override_info| override_info.overridden_role == "LatinText")
    );
    assert!(
        result
            .debug
            .punctuation_decisions
            .iter()
            .all(|decision| !matches!(decision.ch, '“' | '”'))
    );
}

#[test]
fn cjk_quote_pair_reaches_punctuation_geometry_with_outer_context_evidence() {
    let result = layout("中“文”中");

    let quote_overrides = result
        .debug
        .role_overrides
        .iter()
        .filter(|override_info| matches!(override_info.range.start(), 1 | 3))
        .collect::<Vec<_>>();
    assert_eq!(2, quote_overrides.len());
    assert!(
        quote_overrides
            .iter()
            .all(|override_info| override_info.overridden_role == "CjkPunctuation")
    );
    assert!(
        quote_overrides
            .iter()
            .all(|override_info| override_info.source == "PairedPunctuationOuterScriptContext")
    );
    assert_eq!(
        vec![1, 3],
        result
            .debug
            .punctuation_decisions
            .iter()
            .filter(|decision| matches!(decision.ch, '“' | '”'))
            .map(|decision| decision.range.start())
            .collect::<Vec<_>>(),
    );
}

#[test]
fn mixed_paragraph_start_quote_uses_paragraph_language_fallback() {
    let result = layout("“Json是谁？”");

    let quote_overrides = result
        .debug
        .role_overrides
        .iter()
        .filter(|override_info| matches!(override_info.range.start(), 0 | 8))
        .collect::<Vec<_>>();
    assert_eq!(2, quote_overrides.len());
    assert!(
        quote_overrides
            .iter()
            .all(|override_info| override_info.overridden_role == "CjkPunctuation")
    );
    assert!(
        quote_overrides
            .iter()
            .all(|override_info| override_info.source == "ParagraphLanguageQuoteContext")
    );
    assert_eq!("“Json是谁？”", result.input.content.text);
}

#[test]
fn contraction_apostrophe_stays_latin_inside_cjk_single_quotes() {
    let result = layout("中‘that’s’中");

    let contraction = result
        .debug
        .font_decisions
        .iter()
        .find(|decision| decision.source_text == "that’s")
        .unwrap();
    assert_eq!("LatinText", contraction.role);
    assert_eq!("latin-primary", contraction.font_key);
    assert!(
        result
            .debug
            .punctuation_decisions
            .iter()
            .all(|decision| decision.range.start() != 6)
    );
}
