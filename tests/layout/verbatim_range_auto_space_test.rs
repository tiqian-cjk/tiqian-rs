use crate::support::DeterministicStubFontBackend;
use tiqian::api::ParagraphLayoutEngineBuilder;
use tiqian::clreq::clreq_profile::{AutoSpaceMode, AutoSpacePolicy, ClreqProfile, ClreqProfileResolver};
use tiqian::core::geometry::{LayoutConstraints, TextRange, text_range};
use tiqian::core::text::Text;
use tiqian::core::text_model::{LayoutInput, LayoutProfileId, ParagraphStyle, TiqianTextContent};
use tiqian::core::units::Ic;

struct AutoSpaceProfile(AutoSpaceMode);

impl ClreqProfileResolver for AutoSpaceProfile {
    fn resolve(&self, _: &LayoutProfileId) -> ClreqProfile {
        let mut profile = ClreqProfile::mainland_horizontal();
        profile.auto_space = AutoSpacePolicy {
            cjk_latin: self.0,
            cjk_digit: self.0,
            ..AutoSpacePolicy::default()
        };
        profile
    }
}

fn layout(
    text: &str,
    suppressed: Vec<TextRange>,
    mode: AutoSpaceMode,
) -> tiqian::core::layout_model::LayoutResult {
    ParagraphLayoutEngineBuilder::new(Box::new(DeterministicStubFontBackend::default()))
        .clreq_profile_resolver(Box::new(AutoSpaceProfile(mode)))
        .build()
        .layout(
            LayoutInput::builder(
                TiqianTextContent::builder(Text::from(text))
                    .auto_space_suppressed_ranges(suppressed)
                    .build(),
                LayoutConstraints::with_defaults(640.0),
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
fn verbatim_internal_boundaries_are_suppressed_while_outer_edges_keep_autospace() {
    let text = "跑print你好print跑";
    let control = layout(text, Vec::new(), AutoSpaceMode::Insert);
    assert_eq!(
        4,
        control
            .debug
            .auto_space_decisions
            .iter()
            .filter(|decision|decision.reason == "TextAutoSpaceInsert:east-asian-spacing-W-N")
            .count()
    );

    let result = layout(text, vec![text_range(1, 13)], AutoSpaceMode::Insert);
    assert_eq!(
        2,
        result
            .debug
            .auto_space_decisions
            .iter()
            .filter(|decision|decision.reason == "TextAutoSpaceInsert:east-asian-spacing-W-N")
            .count()
    );
    assert_eq!(
        2,
        result
            .debug
            .auto_space_decisions
            .iter()
            .filter(|decision|decision.reason == "VerbatimRangeAutoSpace:east-asian-spacing-W-N-suppressed")
            .count()
    );
}

#[test]
fn authored_space_inside_verbatim_range_is_not_normalized() {
    let text = "跑a 你b跑";
    let control = layout(text, Vec::new(), AutoSpaceMode::Replace);
    assert_eq!(
        1,
        control
            .debug
            .auto_space_decisions
            .iter()
            .filter(
                |decision|decision.reason == "TextAutoSpaceReplace:east-asian-spacing-W-space-N"
            )
            .count()
    );

    let result = layout(text, vec![text_range(1, 5)], AutoSpaceMode::Replace);
    assert_eq!(
        0,
        result
            .debug
            .auto_space_decisions
            .iter()
            .filter(
                |decision|decision.reason == "TextAutoSpaceReplace:east-asian-spacing-W-space-N"
            )
            .count()
    );
}
