use tiqian::core::geometry::{scalar_offset, text_range, LayoutConstraints, TextRange};
use tiqian::core::layout_model::{Cluster, LineEndReason};
use tiqian::core::text::Text;
use tiqian::core::text_model::{
    InlineObjectSpan, LayoutInput, LineBreakPolicy, LineBreakSpan, LineLengthGrid, ParagraphStyle,
    TiqianTextContent,
};
use tiqian::core::units::Ic;
use tiqian::layout::paragraph_layout_engine::{
    ExplainableStubParagraphLayoutEngine, ParagraphLayoutEngine,
};
use tiqian::linebreak::english_hyphenation::english_hyphenation;
use tiqian::shaping::text_shaper::{ShapingInput, ShapingResult, TextShaper};

fn no_indent_style() -> ParagraphStyle {
    ParagraphStyle::builder()
        .first_line_indent(Some(Ic::ZERO))
        .line_length_grid(LineLengthGrid::with_enabled(false))
        .build()
}

fn technical_layout(text: &str, max_width: f32) -> tiqian::core::layout_model::LayoutResult {
    let range = text_range(0, text.chars().count() as i32);
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

    assert!(emergency.contains(&scalar_offset(7)), "{emergency:?}");
    assert!(emergency.contains(&scalar_offset(8)), "{emergency:?}");
    assert!(result.lines.iter().all(|line| line.hyphen_advance == 0.0));
}

struct UniformAdvanceTextShaper;

impl TextShaper for UniformAdvanceTextShaper {
    fn shape(&self, input: &ShapingInput) -> ShapingResult {
        let source = input.text.slice_text(input.range);
        let advance = source.scalar_len().value() as f32 * 10.0;
        ShapingResult::new(
            vec![Cluster::with_display_text(
                input.range,
                source,
                input.display_text.clone(),
                input.font_decision.candidate.key.clone(),
                advance,
            )],
            Vec::new(),
        )
    }
}

#[test]
fn technical_identifier_relabels_loose_letter_digit_boundary_as_emergency() {
    let text = "Machine2Machine";
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.text_shaper = Box::new(UniformAdvanceTextShaper);
    engine.hyphenator = english_hyphenation::en_us();
    let result = engine.layout(
        LayoutInput::builder(
            TiqianTextContent::builder(Text::from(text))
                .line_break_spans(vec![LineBreakSpan {
                    range: text_range(0, text.chars().count() as i32),
                    policy: LineBreakPolicy::ProgressiveTechnical,
                }])
                .build(),
            LayoutConstraints::with_defaults(85.0),
        )
        .paragraph_style(no_indent_style())
        .build(),
    );

    assert_eq!(text_range(0, 8), result.lines[0].range);
    assert!(result.debug.line_decisions[0]
        .notes
        .iter()
        .any(|note| note == "technical-break:Emergency"));
    assert_eq!(0.0, result.lines[0].hyphen_advance);
}

#[test]
fn hash_inside_technical_url_skips_syllable_classification() {
    let hash = "deadbeefcafebabefeedfaceabcdefabcdef";
    let text = format!("https://example.com/commit/{hash}");
    let hash_start = text[..text.find(hash).unwrap()].chars().count() as i32;
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
            .all(|offset| offset.value() <= hash_start || offset.value() >= text.chars().count() as i32),
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
fn repeated_plain_token_gets_narrow_non_lexical_authorization() {
    let text = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
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

    assert!(result
        .debug
        .emergency_tracking_eligibility_decisions
        .iter()
        .any(|decision| {
            decision.range == text_range(0, text.chars().count() as i32)
                && decision.reason == "LongRepeatedLetterRun"
        }));
    for line in result
        .lines
        .iter()
        .filter(|line| line.end_reason == LineEndReason::AutoWrap)
    {
        assert!((line.visual_width - 101.0).abs() < 0.001);
    }
}

#[test]
fn opaque_hard_break_keeps_combining_grapheme_intact() {
    let text = "abc123e\u{0301}def456ghi";
    let combining_mark_offset = scalar_offset(text[..text.find('\u{0301}').unwrap()].chars().count() as i32);
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

#[test]
fn technical_tracking_does_not_open_edges_touching_inline_objects_or_zero_width_controls() {
    let object_text = "aaaaaaaaaaaa\u{fffc}bbbbbbbbbbbb";
    let object_range = text_range(12, 13);
    let mut object_engine = ExplainableStubParagraphLayoutEngine::default();
    let object_result = object_engine.layout(
        LayoutInput::builder(
            TiqianTextContent::builder(Text::from(object_text))
                .line_break_spans(vec![LineBreakSpan {
                    range: text_range(0, object_text.chars().count() as i32),
                    policy: LineBreakPolicy::ProgressiveTechnical,
                }])
                .build(),
            LayoutConstraints::with_defaults(300.0),
        )
        .paragraph_style(no_indent_style())
        .inline_objects(vec![InlineObjectSpan::with_fixed_boundaries(
            object_range,
            16.0,
            12.0,
            4.0,
        )])
        .build(),
    );
    let object_allocations: Vec<_> = object_result
        .debug
        .justification_decisions
        .iter()
        .flat_map(|decision| &decision.allocations)
        .filter(|allocation| allocation.kind == "EmergencyGraphemeTracking")
        .collect();
    assert!(!object_allocations.is_empty());
    assert!(object_allocations.iter().all(|allocation| {
        allocation.cluster_range.end() != object_range.start()
            && allocation.cluster_range != object_range
    }));

    let zero_width_text = "aaaaaaaaaaaa\u{200b}bbbbbbbbbbbb";
    let zero_width_range = text_range(12, 13);
    let mut zero_width_engine = ExplainableStubParagraphLayoutEngine::default();
    let zero_width_result = zero_width_engine.layout(
        LayoutInput::builder(
            TiqianTextContent::builder(Text::from(zero_width_text))
                .line_break_spans(vec![LineBreakSpan {
                    range: text_range(0, zero_width_text.chars().count() as i32),
                    policy: LineBreakPolicy::ProgressiveTechnical,
                }])
                .build(),
            LayoutConstraints::with_defaults(300.0),
        )
        .paragraph_style(no_indent_style())
        .build(),
    );
    let zero_width_allocations: Vec<_> = zero_width_result
        .debug
        .justification_decisions
        .iter()
        .flat_map(|decision| &decision.allocations)
        .filter(|allocation| allocation.kind == "EmergencyGraphemeTracking")
        .collect();
    assert!(!zero_width_allocations.is_empty());
    assert!(zero_width_allocations.iter().all(|allocation| {
        allocation.cluster_range.end() != zero_width_range.start()
            && allocation.cluster_range != zero_width_range
    }));
}

#[test]
fn unannotated_url_does_not_authorize_tracking_across_ordinary_path_components() {
    let identity = "abc123def456ghi789";
    let text = format!("https://example.com/path/to/{identity}");
    let identity_start = scalar_offset(
        text[..text.find(identity).expect("identity start")]
            .chars()
            .count() as i32,
    );
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.hyphenator = english_hyphenation::en_us();
    let result = engine.layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from(text.as_str())),
            LayoutConstraints::with_defaults(160.0),
        )
        .paragraph_style(no_indent_style())
        .build(),
    );

    assert_eq!(
        vec![TextRange::new(identity_start, scalar_offset(text.chars().count() as i32))],
        result
            .debug
            .emergency_tracking_eligibility_decisions
            .iter()
            .map(|decision| decision.range)
            .collect::<Vec<_>>(),
    );
    assert!(result
        .debug
        .justification_decisions
        .iter()
        .flat_map(|decision| &decision.allocations)
        .filter(|allocation| allocation.kind == "EmergencyGraphemeTracking")
        .all(|allocation| allocation.cluster_range.start() >= identity_start));
}

#[test]
fn ordinary_western_prose_is_never_inferred_as_tracking_eligible() {
    let text = "ordinary Western paragraphs keep their natural word spacing";
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.hyphenator = english_hyphenation::en_us();
    let result = engine.layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from(text)),
            LayoutConstraints::with_defaults(137.0),
        )
        .paragraph_style(no_indent_style())
        .build(),
    );

    assert!(result
        .debug
        .emergency_tracking_eligibility_decisions
        .is_empty());
    assert!(result
        .debug
        .justification_decisions
        .iter()
        .flat_map(|decision| &decision.allocations)
        .all(|allocation| allocation.kind != "EmergencyGraphemeTracking"));
    assert!(result
        .debug
        .justification_decisions
        .iter()
        .any(|decision| decision.deficit_after > 0.0));
}
