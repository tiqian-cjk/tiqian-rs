use tiqian::core::Geometry::{LayoutConstraints, TextRange};
use tiqian::core::LayoutModel::LineEndReason;
use tiqian::core::Text::Text;
use tiqian::core::TextModel::{
    LayoutInput, LineBreakPolicy, LineBreakSpan, LineLengthGrid, ParagraphStyle, TiqianTextContent,
};
use tiqian::core::Units::Ic;
use tiqian::layout::ParagraphLayoutEngine::{
    ExplainableStubParagraphLayoutEngine, ParagraphLayoutEngine,
};
use tiqian::linebreak::EnglishHyphenation::english_hyphenation;

fn no_indent_style() -> ParagraphStyle {
    ParagraphStyle::builder()
        .first_line_indent(Some(Ic::ZERO))
        .line_length_grid(LineLengthGrid::with_enabled(false))
        .build()
}

fn technical_layout(text: &str, max_width: f32) -> tiqian::core::LayoutModel::LayoutResult {
    let range = TextRange::new(0, text.encode_utf16().count() as i32);
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.hyphenator = english_hyphenation::en_us();
    engine.layout(
        LayoutInput::builder(
            TiqianTextContent::builder(Text::from(text))
                .line_break_spans(vec![LineBreakSpan {
                    range,
                    policy: LineBreakPolicy::ProgressiveTechnical,
                }])
                .build(),
            LayoutConstraints::with_defaults(max_width),
        )
        .paragraph_style(no_indent_style())
        .build(),
    )
}

#[test]
fn rejected_letter_digit_structural_offsets_remain_emergency_cuts() {
    let result = technical_layout("Machine2Machine", 120.0);
    let emergency: Vec<_> = result
        .debug
        .break_opportunity_decisions
        .iter()
        .filter(|decision| decision.tier.as_deref() == Some("Emergency"))
        .flat_map(|decision| decision.break_offsets.iter().copied())
        .collect();

    assert!(emergency.contains(&7), "{emergency:?}");
    assert!(emergency.contains(&8), "{emergency:?}");
    assert!(result.lines.iter().all(|line| line.hyphen_advance == 0.0));
}

#[test]
fn hash_inside_technical_url_skips_syllable_classification() {
    let hash = "deadbeefcafebabefeedfaceabcdefabcdef";
    let text = format!("https://example.com/commit/{hash}");
    let hash_start = text.find(hash).unwrap() as i32;
    let result = technical_layout(&text, 192.0);
    let syllable_offsets: Vec<_> = result
        .debug
        .break_opportunity_decisions
        .iter()
        .filter(|decision| decision.tier.as_deref() == Some("Syllable"))
        .flat_map(|decision| decision.break_offsets.iter().copied())
        .collect();

    assert!(
        syllable_offsets
            .iter()
            .all(|offset| *offset <= hash_start || *offset >= text.len() as i32),
        "{syllable_offsets:?}"
    );
    assert!(result.lines.iter().all(|line| line.hyphen_advance == 0.0));
}

#[test]
fn technical_hash_uses_emergency_tracking_to_fill_auto_wrapped_lines() {
    let result = technical_layout("deadbeefcafebabefeedfaceabcdefabcdef", 101.0);
    let auto_lines: Vec<_> = result
        .lines
        .iter()
        .filter(|line| line.end_reason == LineEndReason::AutoWrap)
        .collect();

    assert!(!auto_lines.is_empty());
    assert!(
        auto_lines
            .iter()
            .all(|line| (line.visual_width - 101.0).abs() < 0.001)
    );
    assert!(
        result
            .debug
            .justification_decisions
            .iter()
            .flat_map(|decision| &decision.allocations)
            .any(|allocation| {
                allocation.kind == "EmergencyGraphemeTracking"
                    && allocation.reason
                        == "TerminalTechnicalEmergencyTracking:ProgressiveTechnicalSpan"
            })
    );
}

#[test]
fn long_all_caps_word_is_not_tracking_eligible() {
    let text = "SUPERCALIFRAGILISTICEXPIALIDOCIOUS";
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.hyphenator = english_hyphenation::en_us();
    let result = engine.layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from(text)),
            LayoutConstraints::with_defaults(101.0),
        )
        .paragraph_style(no_indent_style())
        .build(),
    );

    assert!(
        result
            .debug
            .emergency_tracking_eligibility_decisions
            .is_empty()
    );
    assert!(
        result
            .debug
            .justification_decisions
            .iter()
            .flat_map(|decision| &decision.allocations)
            .all(|allocation| allocation.kind != "EmergencyGraphemeTracking")
    );
}

#[test]
fn opaque_hard_break_keeps_combining_grapheme_intact() {
    let text = "abc123e\u{0301}def456ghi";
    let combining_mark_offset = text.find('\u{0301}').unwrap() as i32;
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.hyphenator = english_hyphenation::en_us();
    let result = engine.layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from(text)),
            LayoutConstraints::with_defaults(64.0),
        )
        .paragraph_style(no_indent_style())
        .build(),
    );

    assert!(result.clusters.len() > 1);
    assert!(result.clusters.iter().all(|cluster| {
        cluster.range.start() != combining_mark_offset
            && cluster.range.end() != combining_mark_offset
    }));
}
