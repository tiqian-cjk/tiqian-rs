use tiqian::core::geometry::{scalar_offset, text_range, LayoutConstraints};
use tiqian::core::text::Text;
use tiqian::core::text_model::{DecorationKind, DecorationSpan, LayoutInput, TiqianTextContent};
use tiqian::layout::paragraph_layout_engine::{
    ExplainableStubParagraphLayoutEngine, ParagraphLayoutEngine,
};

#[test]
fn emphasis_dots_han_but_not_western_text() {
    let result = ExplainableStubParagraphLayoutEngine::default().layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from("强调中A中")),
            LayoutConstraints::with_defaults(400.0),
        )
        .decorations(vec![DecorationSpan {
            range: text_range(2, 5),
            kind: DecorationKind::Emphasis,
        }])
        .build(),
    );
    let decisions = result
        .debug
        .decoration_decisions
        .iter()
        .map(|decision| (decision.cluster_range.start(), decision))
        .collect::<std::collections::HashMap<_, _>>();

    assert!(decisions[&scalar_offset(2)].applied);
    assert!(decisions[&scalar_offset(4)].applied);
    assert!(!decisions[&scalar_offset(3)].applied);
    assert_eq!("no-dot-on-non-han", decisions[&scalar_offset(3)].reason);
    assert_eq!(0.0, decisions[&scalar_offset(3)].dot_diameter);
}
