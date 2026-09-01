use tiqian::common::HashSet;
use tiqian::core::east_asian_spacing::{EastAsianSpacingEdges, EastAsianSpacingValue};
use tiqian::core::geometry::TextRange;
use tiqian::core::layout_model::Cluster;
use tiqian::core::text::Text;
use tiqian::core::text_model::InlineAttachment;
use tiqian::font::font_policy::FontRole;
use tiqian::layout::unicode_punctuation_boundary_resolver::resolve_attached_inline_inter_char_boundaries;

#[test]
fn resolve_attached_inline_inter_char_boundaries_with_sino_western_pair() {
    let text = Text::from("中，中");
    let result = resolve_attached_inline_inter_char_boundaries(
        &text,
        &[
            Cluster::new(TextRange::new(0, 1), Text::from("中"), "cjk".to_owned(), 16.0),
            Cluster::new(TextRange::new(1, 2), Text::from("，"), "cjk".to_owned(), 16.0),
            Cluster::new(TextRange::new(2, 3), Text::from("中"), "cjk".to_owned(), 16.0),
        ],
        &[FontRole::CjkText, FontRole::CjkPunctuation, FontRole::CjkText],
        &[
            EastAsianSpacingEdges {
                leading: EastAsianSpacingValue::Wide,
                trailing: EastAsianSpacingValue::Wide,
                contains_wide: false,
            },
            EastAsianSpacingEdges {
                leading: EastAsianSpacingValue::Wide,
                trailing: EastAsianSpacingValue::Narrow,
                contains_wide: false,
            },
            EastAsianSpacingEdges {
                leading: EastAsianSpacingValue::Narrow,
                trailing: EastAsianSpacingValue::Wide,
                contains_wide: false,
            },
        ],
        &HashSet::new(),
        &[InlineAttachment::None, InlineAttachment::Previous, InlineAttachment::None],
    );
    assert!(!result.virtual_sino_western_boundary_after_clusters.is_empty());
}

#[test]
fn resolve_attached_inline_inter_char_boundaries_with_both_cjk_punctuation() {
    let text = Text::from("、。中");
    let result = resolve_attached_inline_inter_char_boundaries(
        &text,
        &[
            Cluster::new(TextRange::new(0, 1), Text::from("、"), "cjk".to_owned(), 16.0),
            Cluster::new(TextRange::new(1, 2), Text::from("。"), "cjk".to_owned(), 16.0),
            Cluster::new(TextRange::new(2, 3), Text::from("中"), "cjk".to_owned(), 16.0),
        ],
        &[FontRole::CjkPunctuation, FontRole::CjkPunctuation, FontRole::CjkText],
        &[EastAsianSpacingEdges {
            leading: EastAsianSpacingValue::Wide,
            trailing: EastAsianSpacingValue::Wide,
            contains_wide: false,
        }; 3],
        &HashSet::new(),
        &[InlineAttachment::None, InlineAttachment::Previous, InlineAttachment::None],
    );
    assert!(!result.virtual_boundary_after_clusters.is_empty());
}

#[test]
fn resolve_attached_inline_inter_char_boundaries_punctuation_western_leading_not_narrow() {
    let text = Text::from(",中a");
    let result = resolve_attached_inline_inter_char_boundaries(
        &text,
        &[
            Cluster::new(TextRange::new(0, 1), Text::from(","), "cjk".to_owned(), 16.0),
            Cluster::new(TextRange::new(1, 2), Text::from("中"), "cjk".to_owned(), 16.0),
            Cluster::new(TextRange::new(2, 3), Text::from("a"), "latin".to_owned(), 8.0),
        ],
        &[FontRole::CjkPunctuation, FontRole::CjkText, FontRole::LatinText],
        &[EastAsianSpacingEdges {
            leading: EastAsianSpacingValue::Wide,
            trailing: EastAsianSpacingValue::Wide,
            contains_wide: false,
        }; 3],
        &HashSet::new(),
        &[InlineAttachment::None, InlineAttachment::Previous, InlineAttachment::None],
    );
    assert!(result.virtual_boundary_after_clusters.is_empty());
}

#[test]
fn resolve_attached_inline_inter_char_boundaries_all_conditions_false() {
    let text = Text::from("a*b");
    let result = resolve_attached_inline_inter_char_boundaries(
        &text,
        &[
            Cluster::new(TextRange::new(0, 1), Text::from("a"), "latin".to_owned(), 8.0),
            Cluster::new(TextRange::new(1, 2), Text::from("*"), "latin".to_owned(), 8.0),
            Cluster::new(TextRange::new(2, 3), Text::from("b"), "latin".to_owned(), 8.0),
        ],
        &[FontRole::LatinText; 3],
        &[EastAsianSpacingEdges {
            leading: EastAsianSpacingValue::Wide,
            trailing: EastAsianSpacingValue::Wide,
            contains_wide: false,
        }; 3],
        &HashSet::new(),
        &[InlineAttachment::None, InlineAttachment::Previous, InlineAttachment::None],
    );
    assert!(result.virtual_boundary_after_clusters.is_empty());
}

#[test]
fn resolve_attached_inline_inter_char_boundaries_narrow_narrow_pair() {
    let text = Text::from("a*b");
    let result = resolve_attached_inline_inter_char_boundaries(
        &text,
        &[
            Cluster::new(TextRange::new(0, 1), Text::from("a"), "latin".to_owned(), 8.0),
            Cluster::new(TextRange::new(1, 2), Text::from("*"), "latin".to_owned(), 8.0),
            Cluster::new(TextRange::new(2, 3), Text::from("b"), "latin".to_owned(), 8.0),
        ],
        &[FontRole::LatinText; 3],
        &[
            EastAsianSpacingEdges {
                leading: EastAsianSpacingValue::Wide,
                trailing: EastAsianSpacingValue::Narrow,
                contains_wide: false,
            },
            EastAsianSpacingEdges {
                leading: EastAsianSpacingValue::Wide,
                trailing: EastAsianSpacingValue::Wide,
                contains_wide: false,
            },
            EastAsianSpacingEdges {
                leading: EastAsianSpacingValue::Narrow,
                trailing: EastAsianSpacingValue::Wide,
                contains_wide: false,
            },
        ],
        &HashSet::new(),
        &[InlineAttachment::None, InlineAttachment::Previous, InlineAttachment::None],
    );
    assert!(result.virtual_boundary_after_clusters.is_empty());
}

#[test]
fn resolve_attached_inline_inter_char_boundaries_virtual_from_cjk_punctuation_left() {
    let text = Text::from("，x汉");
    let result = resolve_attached_inline_inter_char_boundaries(
        &text,
        &[
            Cluster::new(TextRange::new(0, 1), Text::from("，"), "cjk".to_owned(), 16.0),
            Cluster::new(TextRange::new(1, 2), Text::from("x"), "latin".to_owned(), 8.0),
            Cluster::new(TextRange::new(2, 3), Text::from("汉"), "cjk".to_owned(), 16.0),
        ],
        &[FontRole::CjkPunctuation, FontRole::LatinText, FontRole::CjkText],
        &[
            EastAsianSpacingEdges {
                leading: EastAsianSpacingValue::Wide,
                trailing: EastAsianSpacingValue::Wide,
                contains_wide: false,
            },
            EastAsianSpacingEdges {
                leading: EastAsianSpacingValue::Other,
                trailing: EastAsianSpacingValue::Other,
                contains_wide: false,
            },
            EastAsianSpacingEdges {
                leading: EastAsianSpacingValue::Narrow,
                trailing: EastAsianSpacingValue::Wide,
                contains_wide: false,
            },
        ],
        &HashSet::new(),
        &[InlineAttachment::None, InlineAttachment::Previous, InlineAttachment::None],
    );
    assert_eq!(Some(&0), result.virtual_boundary_after_clusters.get(&1));
}

#[test]
fn resolve_attached_inline_inter_char_boundaries_punctuation_western_leading_narrow_only() {
    let text = Text::from("，xa");
    let result = resolve_attached_inline_inter_char_boundaries(
        &text,
        &[
            Cluster::new(TextRange::new(0, 1), Text::from("，"), "cjk".to_owned(), 16.0),
            Cluster::new(TextRange::new(1, 2), Text::from("x"), "latin".to_owned(), 8.0),
            Cluster::new(TextRange::new(2, 3), Text::from("a"), "latin".to_owned(), 8.0),
        ],
        &[FontRole::CjkPunctuation, FontRole::LatinText, FontRole::LatinText],
        &[
            EastAsianSpacingEdges {
                leading: EastAsianSpacingValue::Wide,
                trailing: EastAsianSpacingValue::Other,
                contains_wide: false,
            },
            EastAsianSpacingEdges {
                leading: EastAsianSpacingValue::Other,
                trailing: EastAsianSpacingValue::Other,
                contains_wide: false,
            },
            EastAsianSpacingEdges {
                leading: EastAsianSpacingValue::Narrow,
                trailing: EastAsianSpacingValue::Other,
                contains_wide: false,
            },
        ],
        &HashSet::new(),
        &[InlineAttachment::None, InlineAttachment::Previous, InlineAttachment::None],
    );
    assert_eq!(Some(&0), result.virtual_boundary_after_clusters.get(&1));
    assert!(result.virtual_sino_western_boundary_after_clusters.is_empty());
}

#[test]
fn resolve_attached_inline_inter_char_boundaries_punctuation_western_trailing_narrow_only() {
    let text = Text::from("xa，");
    let result = resolve_attached_inline_inter_char_boundaries(
        &text,
        &[
            Cluster::new(TextRange::new(0, 1), Text::from("x"), "latin".to_owned(), 8.0),
            Cluster::new(TextRange::new(1, 2), Text::from("a"), "latin".to_owned(), 8.0),
            Cluster::new(TextRange::new(2, 3), Text::from("，"), "cjk".to_owned(), 16.0),
        ],
        &[FontRole::LatinText, FontRole::LatinText, FontRole::CjkPunctuation],
        &[
            EastAsianSpacingEdges {
                leading: EastAsianSpacingValue::Other,
                trailing: EastAsianSpacingValue::Narrow,
                contains_wide: false,
            },
            EastAsianSpacingEdges {
                leading: EastAsianSpacingValue::Other,
                trailing: EastAsianSpacingValue::Other,
                contains_wide: false,
            },
            EastAsianSpacingEdges {
                leading: EastAsianSpacingValue::Other,
                trailing: EastAsianSpacingValue::Other,
                contains_wide: false,
            },
        ],
        &HashSet::new(),
        &[InlineAttachment::None, InlineAttachment::Previous, InlineAttachment::None],
    );
    assert_eq!(Some(&0), result.virtual_boundary_after_clusters.get(&1));
    assert!(result.virtual_sino_western_boundary_after_clusters.is_empty());
}

#[test]
fn resolve_attached_inline_inter_char_boundaries_western_bracket_only() {
    let text = Text::from("(x汉");
    let result = resolve_attached_inline_inter_char_boundaries(
        &text,
        &[
            Cluster::new(TextRange::new(0, 1), Text::from("("), "latin".to_owned(), 8.0),
            Cluster::new(TextRange::new(1, 2), Text::from("x"), "latin".to_owned(), 8.0),
            Cluster::new(TextRange::new(2, 3), Text::from("汉"), "cjk".to_owned(), 16.0),
        ],
        &[FontRole::LatinText, FontRole::LatinText, FontRole::CjkText],
        &[EastAsianSpacingEdges {
            leading: EastAsianSpacingValue::Other,
            trailing: EastAsianSpacingValue::Other,
            contains_wide: false,
        }; 3],
        &HashSet::new(),
        &[InlineAttachment::None, InlineAttachment::Previous, InlineAttachment::None],
    );
    assert_eq!(Some(&0), result.virtual_boundary_after_clusters.get(&1));
    assert!(result.virtual_sino_western_boundary_after_clusters.is_empty());
}
