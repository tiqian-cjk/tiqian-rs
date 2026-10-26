mod support;

use tiqian::api::ParagraphLayoutEngineBuilder;
use tiqian::clreq::clreq_profile::{AutoSpaceMode, ClreqProfile, ClreqProfileResolver};
use tiqian::core::geometry::{LayoutConstraints, ScalarOffset, text_range};
use tiqian::core::layout_queries::{get_cursor_rect, get_text_for_copy};
use tiqian::core::layout_result_replay_index::{cursor_rect, to_replay_index};
use tiqian::core::text::Text;
use tiqian::core::text_model::{LayoutInput, LineLengthGrid, ParagraphStyle, TextSpan, TextStyle, TextWrapMode, TiqianTextContent, WhiteSpaceMode};
use tiqian::core::units::Ic;

struct Profile(AutoSpaceMode);
impl ClreqProfileResolver for Profile {
    fn resolve(&self, _: &tiqian::core::text_model::LayoutProfileId) -> ClreqProfile {
        let mut profile = ClreqProfile::mainland_horizontal();
        profile.auto_space.cjk_latin = self.0;
        profile.auto_space.cjk_digit = self.0;
        profile
    }
}

fn style(mode: WhiteSpaceMode, wrap: TextWrapMode) -> ParagraphStyle {
    ParagraphStyle::builder().white_space(mode).text_wrap(wrap)
        .first_line_indent(Some(Ic::ZERO)).line_length_grid(LineLengthGrid::with_enabled(false)).build()
}

#[test]
fn fragmented_source_and_style_cover_all_autospace_combinations() {
    for whitespace in [WhiteSpaceMode::Collapse, WhiteSpaceMode::PreserveBreaks, WhiteSpaceMode::Preserve, WhiteSpaceMode::BreakSpaces] {
        for mode in [AutoSpaceMode::Disabled, AutoSpaceMode::Insert, AutoSpaceMode::Replace] {
            let mut engine = ParagraphLayoutEngineBuilder::new(Box::new(support::DeterministicStubFontBackend::default()))
                .clreq_profile_resolver(Box::new(Profile(mode))).build();
            for narrow in ["A", "9"] {
                for reverse in [false, true] {
                    for count in 0..=3 {
                        let spaces = " ".repeat(count);
                        let text = if reverse { format!("{narrow}{spaces}中") } else { format!("中{spaces}{narrow}") };
                        let length = text.chars().count() as i32;
                        let content = TiqianTextContent::builder(Text::from(text.as_str()))
                            .source_boundaries((1..length).map(ScalarOffset::new).collect())
                            .spans((0..length).map(|index| TextSpan { range: text_range(index, index + 1),
                                style: TextStyle::builder().font_weight(if index % 2 == 0 { 400 } else { 500 }).build() }).collect()).build();
                        let result = engine.layout(LayoutInput::builder(content, LayoutConstraints::with_defaults(320.0))
                            .paragraph_style(style(whitespace, TextWrapMode::NoWrap)).build());
                        let effective = if matches!(whitespace, WhiteSpaceMode::Collapse | WhiteSpaceMode::PreserveBreaks) { count.min(1) } else { count };
                        let gap = mode != AutoSpaceMode::Disabled && (count == 0 || mode == AutoSpaceMode::Replace && effective == 1);
                        let expected = 32.0 + if gap { 2.0 } else { effective as f32 * 8.0 };
                        assert_eq!(expected, result.lines[0].visual_width, "{text:?} {whitespace:?} {mode:?}");
                        assert_eq!(usize::from(gap), result.debug.auto_space_decisions.len());
                        assert_eq!(text, get_text_for_copy(&result, text_range(0, length)).as_str());
                        for index in 1..=count {
                            assert!(result.clusters.iter().any(|cluster| cluster.range == text_range(index as i32, index as i32 + 1)));
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn mandatory_character_matrix_keeps_source_and_atomic_crlf() {
    for mode in [WhiteSpaceMode::Collapse, WhiteSpaceMode::PreserveBreaks, WhiteSpaceMode::Preserve, WhiteSpaceMode::BreakSpaces] {
        for wrap in [TextWrapMode::Wrap, TextWrapMode::NoWrap] {
            for newline in ["\n", "\r\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}"] {
                let text = format!("a{newline}");
                let length = text.chars().count() as i32;
                let result = ParagraphLayoutEngineBuilder::new(Box::new(support::DeterministicStubFontBackend::default())).build()
                    .layout(LayoutInput::builder(TiqianTextContent::new(Text::from(text.as_str())), LayoutConstraints::with_defaults(320.0))
                        .paragraph_style(style(mode, wrap)).build());
                let collapsed = mode == WhiteSpaceMode::Collapse && matches!(newline, "\n" | "\r\n" | "\r");
                assert_eq!(if collapsed { 1 } else { 2 }, result.lines.len(), "{mode:?} {wrap:?} {newline:?}");
                assert_eq!(text, get_text_for_copy(&result, text_range(0, length)).as_str());
                let replay = to_replay_index(&result);
                for offset in 0..=length {
                    assert_eq!(get_cursor_rect(&result, ScalarOffset::new(offset)), cursor_rect(&replay, &result, ScalarOffset::new(offset)));
                }
                if newline == "\r\n" {
                    assert_eq!(get_cursor_rect(&result, ScalarOffset::new(2)), get_cursor_rect(&result, ScalarOffset::new(3)));
                }
            }
        }
    }
}

#[test]
fn narrow_whitespace_tabs_and_nonbreaking_spaces_keep_queries_and_progress() {
    use tiqian::core::text_model::TabSize;
    for mode in [WhiteSpaceMode::Collapse, WhiteSpaceMode::PreserveBreaks, WhiteSpaceMode::Preserve, WhiteSpaceMode::BreakSpaces] {
        for wrap in [TextWrapMode::Wrap, TextWrapMode::NoWrap] {
            for width in [0.5, 8.0, 320.0] {
                for text in ["   ", "\t\t", " \t \n", "中\u{a0}A", "中\u{3000}9", "a   \r\n"] {
                    for tab_size in [TabSize::LayoutUnits(32.0), TabSize::SpaceAdvanceMultiple(4.0)] {
                        let mut paragraph = style(mode, wrap);
                        paragraph.tab_size = tab_size;
                        let length = text.chars().count() as i32;
                        let result = ParagraphLayoutEngineBuilder::new(Box::new(support::DeterministicStubFontBackend::default())).build()
                            .layout(LayoutInput::builder(TiqianTextContent::new(Text::from(text)), LayoutConstraints::with_defaults(width))
                                .paragraph_style(paragraph).build());
                        assert!(!result.lines.is_empty(), "{text:?} {mode:?} {wrap:?} {width}");
                        assert_eq!(ScalarOffset::new(length), result.lines.last().unwrap().range.end());
                        assert!(result.lines.len() <= length as usize + 1);
                        assert_eq!(text, get_text_for_copy(&result, text_range(0, length)).as_str());
                        let replay = to_replay_index(&result);
                        for offset in 0..=length {
                            assert_eq!(get_cursor_rect(&result, ScalarOffset::new(offset)), cursor_rect(&replay, &result, ScalarOffset::new(offset)));
                        }
                        if text.contains('\u{a0}') || text.contains('\u{3000}') {
                            assert!(result.debug.auto_space_decisions.is_empty());
                            assert!(result.debug.white_space_units.is_empty());
                            let space = result.clusters.iter().find(|cluster| cluster.range == text_range(1, 2)).unwrap();
                            assert!(space.advance > 0.0);
                        }
                    }
                }
            }
        }
    }
}