use tiqian::core::geometry::{text_range, LayoutConstraints};
use tiqian::core::text::Text;
use tiqian::core::text_model::{
    INLINE_OBJECT_REPLACEMENT_CHAR, InlineObjectSpan, LayoutInput, LineBreakPolicy,
    LineBreakSpan, LineLengthGrid, ParagraphStyle, TiqianTextContent,
};
use tiqian::core::units::Ic;
use tiqian::clreq::clreq_profile::{ClreqProfile, ClreqProfileResolver, LineAdjustmentStrategy};
use tiqian::layout::line_breaker::LookaheadLineBreaker;
use tiqian::layout::paragraph_layout_engine::{
    ExplainableStubParagraphLayoutEngine, ParagraphLayoutEngine,
};

struct AdjustmentProfile(LineAdjustmentStrategy);

impl ClreqProfileResolver for AdjustmentProfile {
    fn resolve(&self, _: &tiqian::core::text_model::LayoutProfileId) -> ClreqProfile {
        let mut profile = ClreqProfile::mainland_horizontal();
        profile.adjustment.line_adjustment = self.0;
        profile
    }
}

fn layout(
    text: &str,
    max_width: f32,
    line_height: Option<f32>,
    line_break_spans: Vec<LineBreakSpan>,
    inline_objects: Vec<InlineObjectSpan>,
) -> tiqian::core::layout_model::LayoutResult {
    let style = ParagraphStyle::builder()
        .first_line_indent(Some(Ic::ZERO))
        .line_length_grid(LineLengthGrid::with_enabled(false))
        .line_height(line_height)
        .build();
    ExplainableStubParagraphLayoutEngine::default().layout(
        LayoutInput::builder(
            TiqianTextContent::builder(Text::from(text))
                .line_break_spans(line_break_spans)
                .build(),
            LayoutConstraints::with_defaults(max_width),
        )
        .paragraph_style(style)
        .inline_objects(inline_objects)
        .build(),
    )
}

#[test]
fn explicit_zero_line_height_keeps_the_control_paragraph_at_zero_height() {
    let result = layout("\n", 100.0, Some(0.0), Vec::new(), Vec::new());
    assert_eq!(2, result.lines.len());
    assert_eq!(0.0, result.size.height);
}

#[test]
fn emergency_boundary_eligibility_skips_zero_width_and_mandatory_controls() {
    let zwsp_text = "ab\u{200b}cd";
    let zwsp_length = Text::from(zwsp_text).scalar_len();
    let zwsp = layout(
        &zwsp_text,
        200.0,
        None,
        vec![LineBreakSpan {
            range: text_range(0, zwsp_length.value()),
            policy: LineBreakPolicy::ProgressiveTechnical,
        }],
        Vec::new(),
    );
    assert_eq!(1, zwsp.lines.len());

    let mandatory = layout(
        "aa\nbb",
        200.0,
        None,
        vec![LineBreakSpan {
            range: text_range(0, 5),
            policy: LineBreakPolicy::ProgressiveTechnical,
        }],
        Vec::new(),
    );
    assert_eq!(2, mandatory.lines.len());
}

#[test]
fn emergency_boundary_eligibility_skips_inline_object_boundaries() {
    let text = format!("a{INLINE_OBJECT_REPLACEMENT_CHAR}b");
    let result = layout(
        &text,
        200.0,
        None,
        vec![LineBreakSpan {
            range: text_range(0, 3),
            policy: LineBreakPolicy::ProgressiveTechnical,
        }],
        vec![InlineObjectSpan::with_fixed_boundaries(text_range(1, 2), 16.0, 8.0, 8.0)],
    );
    assert_eq!(1, result.lines.len());
    assert_eq!(0, result.lines[0].cluster_range.first());
}

#[test]
fn dash_and_solidus_boundaries_inside_technical_spans_never_stretch() {
    for text in ["a—b—c", "a/b/c", "a…b"] {
        let result = layout(
            text,
            24.0,
            None,
            vec![LineBreakSpan {
                range: text_range(0, Text::from(text).scalar_len().value()),
                policy: LineBreakPolicy::ProgressiveTechnical,
            }],
            Vec::new(),
        );
        assert!(!result.lines.is_empty(), "{text}: {:?}", result.lines);
        assert!(result.debug.justification_decisions.iter().flat_map(|decision| &decision.allocations).all(|allocation| {
            allocation.kind != "EmergencyGraphemeTracking" || allocation.delta <= 0.0
        }));
    }
}

#[test]
fn overlapping_technical_spans_keep_the_first_boundary_reason() {
    let result = layout(
        "aabbcc",
        200.0,
        None,
        vec![
            LineBreakSpan { range: text_range(0, 4), policy: LineBreakPolicy::ProgressiveTechnical },
            LineBreakSpan { range: text_range(2, 6), policy: LineBreakPolicy::ProgressiveTechnical },
        ],
        Vec::new(),
    );
    assert_eq!(1, result.lines.len());
}

#[test]
fn push_out_first_takes_fewer_fill_push_ins_than_push_in_first() {
    const TEXT: &str = "咖啡（coffee）在十七世纪经威尼斯传入欧洲。最初它被当作药物出售，价格高得吓人，真正让它流行起来的是随后遍地开花的咖啡馆——读报、辩论、下棋、写作——城市生活忽然多出一个公共客厅。意大利人做出了 espresso，维也纳人往杯里加奶油，土耳其人坚持连渣同煮……每座城市都相信自己手里那一杯才是正统。有人说：「先有咖啡馆，后有启蒙运动」。这话说得夸张，但也不算太离谱。";
    let layout_with = |strategy| {
        let mut engine = ExplainableStubParagraphLayoutEngine::default();
        engine.line_breaker = Box::new(LookaheadLineBreaker::default());
        engine.clreq_profile_resolver = Box::new(AdjustmentProfile(strategy));
        engine.layout(
            LayoutInput::builder(
                TiqianTextContent::new(Text::from(TEXT)),
                LayoutConstraints::with_defaults(320.0),
            )
            .build(),
        )
    };
    let fill_push_in_count = |result: &tiqian::core::layout_model::LayoutResult| {
        result
            .debug
            .line_decisions
            .iter()
            .filter(|decision| {
                decision
                    .repair_decision
                    .as_ref()
                    .is_some_and(|repair| repair.reason_code == "LineAdjustmentPushIn")
            })
            .count()
    };

    let push_in_first = layout_with(LineAdjustmentStrategy::PushInFirst);
    let push_out_first = layout_with(LineAdjustmentStrategy::PushOutFirst);
    assert!(fill_push_in_count(&push_in_first) > 0, "{:?}", push_in_first.debug.line_decisions);
    assert!(fill_push_in_count(&push_out_first) <= fill_push_in_count(&push_in_first));
    assert!(push_out_first.lines.len() >= push_in_first.lines.len());
}
