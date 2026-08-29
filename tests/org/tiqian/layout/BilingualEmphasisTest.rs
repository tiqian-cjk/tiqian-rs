use tiqian::core::Geometry::{LayoutConstraints, TextRange};
use tiqian::core::Text::Text;
use tiqian::core::TextModel::{DecorationKind, DecorationSpan, LayoutInput, TiqianTextContent};
use tiqian::layout::ParagraphLayoutEngine::{
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
            range: TextRange::new(2, 5),
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

    assert!(decisions[&2].applied);
    assert!(decisions[&4].applied);
    assert!(!decisions[&3].applied);
    assert_eq!("no-dot-on-non-han", decisions[&3].reason);
    assert_eq!(0.0, decisions[&3].dot_diameter);
}
