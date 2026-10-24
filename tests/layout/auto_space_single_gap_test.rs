use tiqian::api::ParagraphLayoutEngineBuilder;
use tiqian::clreq::clreq_profile::{
    AutoSpaceMode, AutoSpacePolicy, ClreqProfile, ClreqProfileResolver,
};
use tiqian::core::geometry::{LayoutConstraints, text_range};
use tiqian::core::text::Text;
use tiqian::core::text_model::{
    InlineAttachment, LayoutInput, ParagraphStyle, TextSpan, TextStyle, TiqianTextContent,
};
use tiqian::core::units::Ic;

use crate::support::DeterministicStubFontBackend;

struct LetterOnlyAutoSpace;

impl ClreqProfileResolver for LetterOnlyAutoSpace {
    fn resolve(&self, _: &tiqian::core::text_model::LayoutProfileId) -> ClreqProfile {
        let mut profile = ClreqProfile::mainland_horizontal();
        profile.auto_space = AutoSpacePolicy {
            cjk_latin: AutoSpaceMode::Insert,
            cjk_digit: AutoSpaceMode::Disabled,
            ..AutoSpacePolicy::default()
        };
        profile
    }
}

struct DisabledAutoSpace;

impl ClreqProfileResolver for DisabledAutoSpace {
    fn resolve(&self, _: &tiqian::core::text_model::LayoutProfileId) -> ClreqProfile {
        let mut profile = ClreqProfile::mainland_horizontal();
        profile.auto_space = AutoSpacePolicy::disabled();
        profile
    }
}

fn layout(text: &str) -> tiqian::core::layout_model::LayoutResult {
    ParagraphLayoutEngineBuilder::new(Box::new(DeterministicStubFontBackend::default()))
        .build()
        .layout(
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
fn one_typed_space_becomes_one_autospace_gap() {
    let result = layout("中文 CJK 段落");
    assert!(
        result
            .clusters
            .iter()
            .filter(|cluster| cluster.text == " ")
            .all(|cluster| cluster.advance == 2.0)
    );
    assert_eq!(
        48.0,
        result
            .clusters
            .iter()
            .find(|cluster| cluster.text == "CJK")
            .unwrap()
            .advance
    );
}

#[test]
fn auto_space_replaces_typed_space_at_cjk_latin_boundary() {
    let result = layout("中文 CJK 段落");
    let spaces: Vec<_> = result
        .clusters
        .iter()
        .filter(|cluster| cluster.text == " ")
        .collect();

    assert_eq!(2, spaces.len());
    assert!(spaces.iter().all(|space| space.advance == 2.0));
    assert_eq!(2, result.debug.auto_space_decisions.len());
    assert!(result.debug.auto_space_decisions.iter().all(|decision| {
        decision.mode == "Replace" && decision.side == "gap" && decision.total_reduction == 6.0
    }));
}

#[test]
fn auto_space_does_not_shrink_spaces_between_latin_words() {
    let result = layout("Hello world");

    assert_eq!(3, result.clusters.len());
    assert_eq!(
        8.0,
        result
            .clusters
            .iter()
            .find(|cluster| cluster.text == " ")
            .unwrap()
            .advance
    );
    assert!(result.debug.auto_space_decisions.is_empty());
}

#[test]
fn numbered_labels_have_different_typed_space_widths_before_cjk_and_latin() {
    let mut widths = Vec::new();
    for auto_space_enabled in [true, false] {
        let mut builder =
            ParagraphLayoutEngineBuilder::new(Box::new(DeterministicStubFontBackend::default()));
        if !auto_space_enabled {
            builder = builder.clreq_profile_resolver(Box::new(DisabledAutoSpace));
        }
        let mut engine = builder.build();
        for (text, expected_width) in [
            ("01 基础组件", if auto_space_enabled { 2.0 } else { 8.0 }),
            ("02 Input 输入框", 8.0),
        ] {
            let result = engine.layout(
                LayoutInput::builder(
                    TiqianTextContent::new(Text::from(text)),
                    LayoutConstraints::with_defaults(320.0),
                )
                .text_style(TextStyle::builder().font_size(16.0).build())
                .paragraph_style(
                    ParagraphStyle::builder()
                        .first_line_indent(Some(Ic::ZERO))
                        .build(),
                )
                .build(),
            );
            assert_eq!(1, result.lines.len(), "{text}");
            // 两段原文中数字后的空格均位于 scalar range 2..3。
            let space_range = text_range(2, 3);
            let space = result.clusters.iter().find(|c| c.range == space_range).unwrap();
            assert_eq!(" ", space.text.as_str());
            assert_eq!(expected_width, space.advance, "{text}, auto_space={auto_space_enabled}");
            let positioned = tiqian::core::layout_queries::positioned_clusters(&result);
            let occupied = positioned.iter().find(|c| c.range == space_range).unwrap();
            assert_eq!(expected_width, occupied.width(), "{text}");
            let decision = result.debug.auto_space_decisions.iter()
                .find(|decision| decision.cluster_range == space_range);
            if auto_space_enabled && text == "01 基础组件" {
                let decision = decision.unwrap();
                assert_eq!("Replace", decision.mode);
                assert_eq!("gap", decision.side);
                assert_eq!(6.0, decision.total_reduction);
            } else {
                assert!(decision.is_none(), "{text}");
            }
            println!("{text}: auto_space={auto_space_enabled}, space_width={}", space.advance);
            widths.push(space.advance);
        }
    }
    assert_ne!(widths[0], widths[1]);
    assert_eq!(widths[2], widths[3]);
}

#[test]
fn auto_space_disabled_keeps_typed_spaces_at_half_em() {
    let clreq_profile_resolver = Box::new(DisabledAutoSpace);
    let mut engine =
        ParagraphLayoutEngineBuilder::new(Box::new(DeterministicStubFontBackend::default()))
            .clreq_profile_resolver(clreq_profile_resolver)
            .build();
    let result = engine.layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from("中文 CJK 段落")),
            LayoutConstraints::with_defaults(320.0),
        )
        .paragraph_style(
            ParagraphStyle::builder()
                .first_line_indent(Some(Ic::ZERO))
                .build(),
        )
        .build(),
    );
    let spaces: Vec<_> = result
        .clusters
        .iter()
        .filter(|cluster| cluster.text == " ")
        .collect();

    assert_eq!(2, spaces.len());
    assert!(spaces.iter().all(|space| space.advance == 8.0));
    assert!(result.debug.auto_space_decisions.is_empty());
}

#[test]
fn two_typed_spaces_at_boundary_still_collapse_to_one_gap() {
    let result = layout("中文  CJK 段落");
    assert_eq!(
        2.0,
        result
            .clusters
            .iter()
            .find(|cluster| cluster.text == "  ")
            .unwrap()
            .advance
    );
    assert_eq!(
        2.0,
        result
            .clusters
            .iter()
            .find(|cluster| cluster.text == " ")
            .unwrap()
            .advance
    );
}

#[test]
fn three_typed_spaces_still_one_gap() {
    let result = layout("中文   CJK段落");
    assert_eq!(
        2.0,
        result
            .clusters
            .iter()
            .find(|cluster| cluster.text == "   ")
            .unwrap()
            .advance
    );
    assert_eq!(
        50.0,
        result
            .clusters
            .iter()
            .find(|cluster| cluster.text == "CJK")
            .unwrap()
            .advance
    );
}

fn layout_with_attached_reference(text: &str) -> tiqian::core::layout_model::LayoutResult {
    ParagraphLayoutEngineBuilder::new(Box::new(DeterministicStubFontBackend::default()))
        .build()
        .layout(
            LayoutInput::builder(
                TiqianTextContent::builder(Text::from(text))
                    .spans(vec![TextSpan {
                        range: text_range(2, 3),
                        style: TextStyle::builder()
                            .inline_attachment(InlineAttachment::Previous)
                            .build(),
                    }])
                    .build(),
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
fn attached_reference_between_cjk_text_does_not_invent_an_autospace_gap() {
    assert!(
        layout_with_attached_reference("正文1后文")
            .debug
            .auto_space_decisions
            .is_empty()
    );
}

#[test]
fn attached_reference_before_latin_text_gets_the_virtual_cjk_latin_gap() {
    let decision = &layout_with_attached_reference("正文1ABC")
        .debug
        .auto_space_decisions[0];
    assert_eq!("trailing", decision.side);
    assert_eq!("InlineAttachment.Previous", decision.boundary_role);
    assert_eq!(
        "AttachedInlineVirtualAutoSpace:east-asian-spacing-W-N",
        decision.reason
    );
}

#[test]
fn attached_reference_at_paragraph_end_has_no_autospace_gap() {
    assert!(
        layout_with_attached_reference("正文1")
            .debug
            .auto_space_decisions
            .is_empty()
    );
}

#[test]
fn unicode_east_asian_spacing_covers_narrow_scripts_without_script_whitelists() {
    for sample in ["α", "я", "ա"] {
        let result = layout(&format!("中{sample}文"));
        let narrow = result
            .clusters
            .iter()
            .find(|cluster| cluster.text == sample)
            .unwrap();
        assert_eq!(20.0, narrow.advance, "sample={sample}");
        assert_eq!(
            2,
            result.debug.auto_space_decisions.len(),
            "sample={sample}"
        );
        assert!(
            result
                .debug
                .auto_space_decisions
                .iter()
                .all(|decision| decision.cluster_range == narrow.range)
        );
        assert!(
            result
                .debug
                .auto_space_decisions
                .iter()
                .all(|decision| decision.reason == "TextAutoSpaceInsert:east-asian-spacing-W-N")
        );
    }
}

#[test]
fn conditional_punctuation_follows_chinese_language_resolution() {
    let result = layout("中%文");
    assert_eq!(2, result.debug.auto_space_decisions.len());
    assert!(
        result
            .debug
            .auto_space_decisions
            .iter()
            .all(|decision| decision.boundary_role == "EastAsianSpacing.Wide")
    );
}

#[test]
fn autospace_does_not_fire_between_latin_and_cjk_punctuation() {
    assert!(
        layout("Tiqian ）说明")
            .debug
            .auto_space_decisions
            .is_empty()
    );
}

#[test]
fn autospace_does_not_fire_before_slash_led_latin_technical_run() {
    let result = layout("恐跨/TERFism。如果");
    let technical = result
        .clusters
        .iter()
        .find(|cluster| cluster.text == "/TERFism")
        .unwrap();
    assert!(result.debug.auto_space_decisions.iter().all(|decision| decision.cluster_range != technical.range || decision.side != "leading"));
}

#[test]
fn autospace_still_fires_between_latin_and_cjk_text_even_with_punctuation_nearby() {
    let decisions = &layout("中文 shaping 之后").debug.auto_space_decisions;
    assert_eq!(2, decisions.len());
    assert!(
        decisions
            .iter()
            .all(|decision| decision.boundary_role == "EastAsianSpacing.Wide")
    );
    assert!(decisions.iter().all(|decision| decision.side == "gap"));
}

#[test]
fn absent_authored_space_inserts_one_gap_at_each_cjk_latin_edge() {
    let result = layout("中文CJK段落");
    let latin = result
        .clusters
        .iter()
        .find(|cluster| cluster.text == "CJK")
        .unwrap();

    assert_eq!(52.0, latin.advance);
    assert_eq!(2, result.debug.auto_space_decisions.len());
    assert!(
        result
            .debug
            .auto_space_decisions
            .iter()
            .all(|decision| decision.mode == "Insert")
    );
    assert!(
        result
            .debug
            .auto_space_decisions
            .iter()
            .all(|decision| decision.characters_affected == 0)
    );
    assert!(
        result
            .debug
            .auto_space_decisions
            .iter()
            .all(|decision| decision.total_reduction == -2.0)
    );
}

#[test]
fn letter_and_digit_boundaries_follow_separate_profile_modes() {
    let clreq_profile_resolver = Box::new(LetterOnlyAutoSpace);
    let mut engine =
        ParagraphLayoutEngineBuilder::new(Box::new(DeterministicStubFontBackend::default()))
            .clreq_profile_resolver(clreq_profile_resolver)
            .build();
    let result = engine.layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from("甲A乙9丙")),
            LayoutConstraints::with_defaults(320.0),
        )
        .paragraph_style(
            ParagraphStyle::builder()
                .first_line_indent(Some(Ic::ZERO))
                .build(),
        )
        .build(),
    );
    let a_range = result
        .clusters
        .iter()
        .find(|cluster| cluster.text == "A")
        .unwrap()
        .range;
    let nine_range = result
        .clusters
        .iter()
        .find(|cluster| cluster.text == "9")
        .unwrap()
        .range;

    assert!(!result.debug.auto_space_decisions.is_empty());
    assert!(
        result
            .debug
            .auto_space_decisions
            .iter()
            .all(|decision| decision.cluster_range == a_range)
    );
    assert!(
        result
            .debug
            .auto_space_decisions
            .iter()
            .all(|decision| decision.cluster_range != nine_range)
    );
}

struct PhaseTwoAutoSpace(AutoSpaceMode);

impl ClreqProfileResolver for PhaseTwoAutoSpace {
    fn resolve(&self, _: &tiqian::core::text_model::LayoutProfileId) -> ClreqProfile {
        let mut profile = ClreqProfile::mainland_horizontal();
        profile.auto_space.cjk_latin = self.0;
        profile.auto_space.cjk_digit = self.0;
        profile
    }
}

#[test]
fn phase_two_white_space_and_auto_space_matrix() {
    use tiqian::core::text_model::WhiteSpaceMode;
    for white_space in [WhiteSpaceMode::Collapse, WhiteSpaceMode::PreserveBreaks,
        WhiteSpaceMode::Preserve, WhiteSpaceMode::BreakSpaces] {
        for mode in [AutoSpaceMode::Disabled, AutoSpaceMode::Insert, AutoSpaceMode::Replace] {
            let mut engine = ParagraphLayoutEngineBuilder::new(Box::new(DeterministicStubFontBackend::default()))
                .clreq_profile_resolver(Box::new(PhaseTwoAutoSpace(mode))).build();
            for narrow in ["A", "9"] {
                for reverse in [false, true] {
                    for count in 0..=3 {
                        let spaces = " ".repeat(count);
                        let text = if reverse { format!("{narrow}{spaces}中") }
                            else { format!("中{spaces}{narrow}") };
                        let result = engine.layout(LayoutInput::builder(
                            TiqianTextContent::new(Text::from(text.as_str())),
                            LayoutConstraints::with_defaults(320.))
                            .paragraph_style(ParagraphStyle::builder().white_space(white_space)
                                .first_line_indent(Some(Ic::ZERO)).build()).build());
                        let effective = if matches!(white_space, WhiteSpaceMode::Collapse | WhiteSpaceMode::PreserveBreaks) {
                            count.min(1)
                        } else { count };
                        let expected = if count == 0 {
                            if mode == AutoSpaceMode::Disabled { 32. } else { 34. }
                        } else if mode == AutoSpaceMode::Replace && effective == 1 {
                            34.
                        } else { 32. + effective as f32 * 8. };
                        assert_eq!(expected, result.clusters.iter().map(|cluster| cluster.advance).sum::<f32>(),
                            "{text:?}, {white_space:?}, {mode:?}");
                        assert_eq!(text, result.input.content.text.as_str());
                        if count > 0 {
                            assert_eq!(effective, result.clusters.iter().filter(|cluster| cluster.display_text == " ").count());
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn phase_two_folded_whitespace_keeps_source_and_first_style() {
    use tiqian::core::text_model::WhiteSpaceMode;
    let text = Text::from("中 \t\r\n A");
    let result = ParagraphLayoutEngineBuilder::new(Box::new(DeterministicStubFontBackend::default()))
        .clreq_profile_resolver(Box::new(DisabledAutoSpace)).build().layout(
            LayoutInput::builder(TiqianTextContent::builder(text.clone()).spans(vec![
                TextSpan { range: text_range(1, 2), style: TextStyle::builder().font_size(24.).build() },
                TextSpan { range: text_range(2, 6), style: TextStyle::builder().font_size(32.).build() },
            ]).build(), LayoutConstraints::with_defaults(320.))
                .paragraph_style(ParagraphStyle::builder().white_space(WhiteSpaceMode::Collapse)
                    .first_line_indent(Some(Ic::ZERO)).build()).build());
    let space = result.clusters.iter().find(|cluster| cluster.range == text_range(1, 2)).unwrap();
    assert_eq!(12., space.advance);
    assert_eq!(" ", space.display_text.as_str());
    for range in [text_range(2, 3), text_range(3, 5), text_range(5, 6)] {
        let folded = result.clusters.iter().find(|cluster| cluster.range == range).unwrap();
        assert_eq!(0., folded.advance);
        assert!(folded.display_text.is_empty());
    }
    assert_eq!(text, result.input.content.text);
    assert_eq!(1, result.lines.len());
}

#[test]
fn phase_two_converted_crlf_shapes_one_space_with_original_glyph_range() {
    use tiqian::core::text_model::WhiteSpaceMode;
    let result = ParagraphLayoutEngineBuilder::new(Box::new(DeterministicStubFontBackend::default()))
        .clreq_profile_resolver(Box::new(DisabledAutoSpace)).build().layout(
            LayoutInput::builder(TiqianTextContent::new(Text::from("中\r\nA")), LayoutConstraints::with_defaults(320.))
                .paragraph_style(ParagraphStyle::builder().white_space(WhiteSpaceMode::Collapse)
                    .first_line_indent(Some(Ic::ZERO)).build()).build());
    let range = text_range(1, 3);
    let space = result.clusters.iter().find(|cluster| cluster.range == range).unwrap();
    assert_eq!("\r\n", space.text.as_str());
    assert_eq!(" ", space.display_text.as_str());
    assert_eq!(8., space.advance);
    let glyphs: Vec<_> = result.glyph_runs.iter().flat_map(|run| &run.glyphs)
        .filter(|glyph| glyph.cluster_range == range).collect();
    assert_eq!(1, glyphs.len());
    assert_eq!(8., glyphs[0].advance);
}

#[test]
fn phase_two_hard_breaks_tabs_and_suppression_block_auto_space() {
    use tiqian::core::text_model::WhiteSpaceMode;
    for mode in [WhiteSpaceMode::PreserveBreaks, WhiteSpaceMode::Preserve, WhiteSpaceMode::BreakSpaces] {
        for text in ["中\nA", "A\r\n中", "中\u{2028}A", "中\tA"] {
            let result = ParagraphLayoutEngineBuilder::new(Box::new(DeterministicStubFontBackend::default()))
                .build().layout(LayoutInput::builder(TiqianTextContent::new(Text::from(text)),
                    LayoutConstraints::with_defaults(320.))
                    .paragraph_style(ParagraphStyle::builder().white_space(mode)
                        .first_line_indent(Some(Ic::ZERO)).build()).build());
            assert!(result.debug.auto_space_decisions.is_empty(), "{mode:?}, {text:?}");
        }
    }
    let result = ParagraphLayoutEngineBuilder::new(Box::new(DeterministicStubFontBackend::default()))
        .clreq_profile_resolver(Box::new(PhaseTwoAutoSpace(AutoSpaceMode::Replace))).build().layout(
            LayoutInput::builder(TiqianTextContent::builder(Text::from("中  A"))
                .auto_space_suppressed_ranges(vec![text_range(0, 4)]).build(),
                LayoutConstraints::with_defaults(320.))
                .paragraph_style(ParagraphStyle::builder().first_line_indent(Some(Ic::ZERO)).build()).build());
    assert!(result.debug.auto_space_decisions.is_empty());
    assert_eq!(40., result.clusters.iter().map(|cluster| cluster.advance).sum::<f32>());
}
