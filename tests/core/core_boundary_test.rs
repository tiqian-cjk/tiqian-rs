use tiqian::core::geometry::{scalar_offset, text_range, LayoutConstraints, Size};
use tiqian::core::int_range::IntRange;
use tiqian::core::layout_model::{Cluster, LayoutResult, LineBox};
use tiqian::core::layout_queries::get_selection_offset_for_position;
use tiqian::core::source_interaction_boundaries::source_grapheme_boundaries;
use tiqian::core::text::Text;
use tiqian::core::text_model::{LayoutInput, TiqianTextContent};

#[test]
fn source_grapheme_boundaries_returns_single_boundary_for_empty_text() {
    let text = Text::new();
    assert_eq!(vec![scalar_offset(0)], source_grapheme_boundaries(&text, text_range(0, 0)));
}

#[test]
fn get_selection_offset_for_position_returns_start_of_first_cluster() {
    let result = LayoutResult::new(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from("abc")),
            LayoutConstraints::with_defaults(100.0),
        )
        .build(),
        Size { width: 30.0, height: 20.0 },
        vec![
            Cluster::new(text_range(0, 1), Text::from("a"), "latin".to_owned(), 10.0),
            Cluster::new(text_range(1, 2), Text::from("b"), "latin".to_owned(), 10.0),
            Cluster::new(text_range(2, 3), Text::from("c"), "latin".to_owned(), 10.0),
        ],
        Vec::new(),
        vec![LineBox::builder(
            text_range(0, 3),
            IntRange::new(0, 2),
            15.0,
            0.0,
            20.0,
            30.0,
            30.0,
            30.0,
        )
        .build()],
    );

    assert_eq!(scalar_offset(0), get_selection_offset_for_position(&result, 0.0, 10.0));
    assert_eq!(scalar_offset(1), get_selection_offset_for_position(&result, 10.0, 10.0));
    assert_eq!(scalar_offset(2), get_selection_offset_for_position(&result, 20.0, 10.0));
}

#[test]
fn get_selection_offset_for_position_returns_start_of_line_when_empty_clusters() {
    let result = LayoutResult::new(
        LayoutInput::builder(
            TiqianTextContent::new(Text::new()),
            LayoutConstraints::with_defaults(100.0),
        )
        .build(),
        Size { width: 0.0, height: 20.0 },
        Vec::new(),
        Vec::new(),
        vec![LineBox::builder(
            text_range(0, 0),
            IntRange::new(0, -1),
            15.0,
            0.0,
            20.0,
            0.0,
            0.0,
            0.0,
        )
        .build()],
    );

    assert_eq!(scalar_offset(0), get_selection_offset_for_position(&result, 5.0, 10.0));
}