use tiqian::common::HashSet;
use tiqian::core::east_asian_spacing::{EastAsianSpacingEdges, EastAsianSpacingValue};
use tiqian::core::geometry::TextRange;
use tiqian::core::layout_model::Cluster;
use tiqian::core::text::Text;
use tiqian::core::text_model::InlineAttachment;
use tiqian::font::font_policy::FontRole;
use tiqian::layout::quote_pair_analyzer::{QuotePair, QuoteType};
use tiqian::layout::unicode_punctuation_boundary_resolver::{
    resolve_attached_inline_inter_char_boundaries, resolve_attached_inline_virtual_boundaries,
    resolve_unicode_punctuation_boundaries,
};

fn clusters(text: &str, font_key: &str, advance: f32) -> Vec<Cluster> {
    let mut offset = 0;
    text.chars()
        .map(|character| {
            let end = offset + character.len_utf16() as i32;
            let cluster = Cluster::new(
                TextRange::new(offset, end),
                Text::from(character.to_string()),
                font_key.to_owned(),
                advance,
            );
            offset = end;
            cluster
        })
        .collect()
}

#[test]
fn resolve_attached_inline_virtual_boundaries_with_multiple_previous() {
    let result = resolve_attached_inline_virtual_boundaries(&[
        InlineAttachment::None,
        InlineAttachment::Previous,
        InlineAttachment::Previous,
        InlineAttachment::None,
    ]);
    assert_eq!(1, result.len());
    assert_eq!(0, result[0].previous_cluster_index);
    assert_eq!((1, 2), result[0].attached_cluster_range);
    assert_eq!(Some(3), result[0].next_cluster_index);
}

#[test]
fn resolve_attached_inline_virtual_boundaries_with_no_previous() {
    let result = resolve_attached_inline_virtual_boundaries(&[
        InlineAttachment::None,
        InlineAttachment::None,
    ]);
    assert!(result.is_empty());
}

#[test]
fn resolve_attached_inline_virtual_boundaries_at_start() {
    let result = resolve_attached_inline_virtual_boundaries(&[
        InlineAttachment::Previous,
        InlineAttachment::None,
    ]);
    assert!(result.is_empty());
}

#[test]
fn resolve_unicode_punctuation_boundaries_with_paired_quotes() {
    let text = Text::from("中文“你好”中文");
    let cluster_roles = vec![FontRole::CjkText; 8];
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &clusters(text.as_str(), "cjk", 16.0),
        &cluster_roles,
        &[QuotePair::new(2, 5, QuoteType::Double)],
    );
    assert!(result.decisions.iter().any(|decision| {
        decision.reason == "Uax14WesternPunctuationBoundary:PairedOpeningQuote"
    }));
    assert!(result.decisions.iter().any(|decision| {
        decision.reason == "Uax14WesternPunctuationBoundary:PairedClosingQuote"
    }));
}

#[test]
fn resolve_unicode_punctuation_boundaries_with_unmatched_closing_punctuation() {
    let text = Text::from("中。");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &clusters(text.as_str(), "cjk", 16.0),
        &[FontRole::CjkText; 2],
        &[],
    );
    assert!(!result.forbidden_line_start_clusters.is_empty());
}

#[test]
fn resolve_unicode_punctuation_boundaries_with_cjk_closing_at_line_start() {
    let text = Text::from("。，");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &clusters(text.as_str(), "cjk", 16.0),
        &[FontRole::CjkText; 2],
        &[QuotePair::new(0, 1, QuoteType::Single)],
    );
    assert!(result
        .decisions
        .iter()
        .any(|decision| decision.forbidden_position == "LineEnd"));
}

#[test]
fn resolve_unicode_punctuation_boundaries_with_exclamation_mark() {
    let text = Text::from("中!中");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &[
            Cluster::new(TextRange::new(0, 1), Text::from("中"), "cjk".to_owned(), 16.0),
            Cluster::new(TextRange::new(1, 2), Text::from("!"), "latin".to_owned(), 16.0),
            Cluster::new(TextRange::new(2, 3), Text::from("中"), "cjk".to_owned(), 16.0),
        ],
        &[FontRole::CjkText; 3],
        &[],
    );
    assert!(result
        .decisions
        .iter()
        .any(|decision| decision.forbidden_position == "LineStart"));
}

#[test]
fn resolve_unicode_punctuation_boundaries_with_initial_quote_forbid_line_end() {
    let text = Text::from("中“中");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &clusters(text.as_str(), "cjk", 16.0),
        &[FontRole::CjkText; 3],
        &[],
    );
    assert!(result
        .decisions
        .iter()
        .any(|decision| decision.forbidden_position == "LineEnd"));
}

#[test]
fn resolve_unicode_punctuation_boundaries_with_unresolved_quote() {
    let text = Text::from("中’中");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &clusters(text.as_str(), "cjk", 16.0),
        &[FontRole::CjkText; 3],
        &[],
    );
    assert!(result
        .decisions
        .iter()
        .any(|decision| decision.forbidden_position == "LineStart"));
}

#[test]
fn resolve_unicode_punctuation_boundaries_with_multiple_clusters() {
    let text = Text::from("中文，中文");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &clusters(text.as_str(), "cjk", 16.0),
        &[FontRole::CjkText; 5],
        &[],
    );
    assert!(result
        .decisions
        .iter()
        .any(|decision| decision.forbidden_position == "LineStart"));
}

#[test]
fn resolve_unicode_punctuation_boundaries_with_empty_clusters() {
    let result = resolve_unicode_punctuation_boundaries(&Text::default(), &[], &[], &[]);
    assert!(result.forbidden_line_start_clusters.is_empty());
}

#[test]
fn resolve_unicode_punctuation_boundaries_with_all_cjk_text() {
    let text = Text::from("中文文文");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &clusters(text.as_str(), "cjk", 16.0),
        &[FontRole::CjkText; 4],
        &[],
    );
    assert!(result.decisions.is_empty());
}

#[test]
fn resolve_unicode_punctuation_boundaries_with_western_closing_forbid_line_start() {
    let text = Text::from("中)中");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &[
            Cluster::new(TextRange::new(0, 1), Text::from("中"), "cjk".to_owned(), 16.0),
            Cluster::new(TextRange::new(1, 2), Text::from(")"), "latin".to_owned(), 16.0),
            Cluster::new(TextRange::new(2, 3), Text::from("中"), "cjk".to_owned(), 16.0),
        ],
        &[FontRole::CjkText, FontRole::CjkText, FontRole::CjkText],
        &[],
    );
    assert!(!result.forbidden_line_start_clusters.is_empty());
}

#[test]
fn resolve_unicode_punctuation_boundaries_with_open_punctuation_forbid_line_end() {
    let text = Text::from("（中文");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &clusters(text.as_str(), "cjk", 16.0),
        &[FontRole::CjkText; 3],
        &[],
    );
    assert!(result
        .decisions
        .iter()
        .any(|decision| decision.forbidden_position == "LineEnd"));
}

#[test]
fn resolve_unicode_punctuation_boundaries_with_rule_for_line_start_infix() {
    let text = Text::from("1,2");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &[
            Cluster::new(TextRange::new(0, 1), Text::from("1"), "latin".to_owned(), 8.0),
            Cluster::new(TextRange::new(1, 2), Text::from(","), "latin".to_owned(), 8.0),
            Cluster::new(TextRange::new(2, 3), Text::from("2"), "latin".to_owned(), 8.0),
        ],
        &[FontRole::LatinText; 3],
        &[],
    );
    let infix = result
        .decisions
        .iter()
        .find(|decision| decision.source_text == ",")
        .unwrap();
    assert_eq!("Uax14WesternPunctuationBoundary:LB15d", infix.reason);
}

#[test]
fn resolve_attached_inline_inter_char_boundaries_sino_western_only() {
    let text = Text::from("汉xa");
    let result = resolve_attached_inline_inter_char_boundaries(
        &text,
        &[
            Cluster::new(TextRange::new(0, 1), Text::from("汉"), "cjk".to_owned(), 16.0),
            Cluster::new(TextRange::new(1, 2), Text::from("x"), "latin".to_owned(), 8.0),
            Cluster::new(TextRange::new(2, 3), Text::from("a"), "latin".to_owned(), 8.0),
        ],
        &[FontRole::CjkText, FontRole::LatinText, FontRole::LatinText],
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
                trailing: EastAsianSpacingValue::Other,
                contains_wide: false,
            },
        ],
        &HashSet::new(),
        &[
            InlineAttachment::None,
            InlineAttachment::Previous,
            InlineAttachment::None,
        ],
    );
    assert_eq!(Some(&0), result.virtual_boundary_after_clusters.get(&1));
    assert_eq!(HashSet::from([1]), result.virtual_sino_western_boundary_after_clusters);
}

#[test]
fn resolve_attached_inline_inter_char_boundaries_with_cjk_both_cjk() {
    let text = Text::from("中文");
    let result = resolve_attached_inline_inter_char_boundaries(
        &text,
        &clusters(text.as_str(), "cjk", 16.0),
        &[FontRole::CjkText, FontRole::CjkText],
        &[
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
        &[InlineAttachment::None, InlineAttachment::None],
    );
    assert!(result.virtual_boundary_after_clusters.is_empty());
}

#[test]
fn resolve_attached_inline_inter_char_boundaries_with_western_bracket() {
    let text = Text::from("(中");
    let result = resolve_attached_inline_inter_char_boundaries(
        &text,
        &[
            Cluster::new(TextRange::new(0, 1), Text::from("("), "latin".to_owned(), 8.0),
            Cluster::new(TextRange::new(1, 2), Text::from("中"), "cjk".to_owned(), 16.0),
        ],
        &[FontRole::LatinText, FontRole::CjkText],
        &[EastAsianSpacingEdges {
            leading: EastAsianSpacingValue::Wide,
            trailing: EastAsianSpacingValue::Wide,
            contains_wide: false,
        }; 2],
        &HashSet::new(),
        &[InlineAttachment::None, InlineAttachment::None],
    );
    assert!(result.virtual_boundary_after_clusters.is_empty());
}

#[test]
fn resolve_attached_inline_inter_char_boundaries_with_cjk_body_western_bracket() {
    let text = Text::from("中)");
    let result = resolve_attached_inline_inter_char_boundaries(
        &text,
        &[
            Cluster::new(TextRange::new(0, 1), Text::from("中"), "cjk".to_owned(), 16.0),
            Cluster::new(TextRange::new(1, 2), Text::from(")"), "latin".to_owned(), 8.0),
        ],
        &[FontRole::CjkText, FontRole::LatinText],
        &[EastAsianSpacingEdges {
            leading: EastAsianSpacingValue::Wide,
            trailing: EastAsianSpacingValue::Wide,
            contains_wide: false,
        }; 2],
        &HashSet::new(),
        &[InlineAttachment::None, InlineAttachment::None],
    );
    assert!(result.ordinary_western_boundary_after_clusters.is_empty());
}

#[test]
#[should_panic(expected = "Clusters, roles and East_Asian_Spacing edges must align.")]
fn resolve_attached_inline_inter_char_boundaries_requires_matching_cluster_role_edge_sizes() {
    let text = Text::from("ab");
    let _ = resolve_attached_inline_inter_char_boundaries(
        &text,
        &[Cluster::new(TextRange::new(0, 1), Text::from("a"), "latin".to_owned(), 8.0)],
        &[FontRole::LatinText, FontRole::LatinText],
        &[EastAsianSpacingEdges {
            leading: EastAsianSpacingValue::Wide,
            trailing: EastAsianSpacingValue::Wide,
            contains_wide: false,
        }],
        &HashSet::new(),
        &[InlineAttachment::None],
    );
}

#[test]
#[should_panic(expected = "Inline attachments must align with clusters.")]
fn resolve_attached_inline_inter_char_boundaries_requires_matching_attachment_size() {
    let text = Text::from("ab");
    let _ = resolve_attached_inline_inter_char_boundaries(
        &text,
        &clusters(text.as_str(), "latin", 8.0),
        &[FontRole::LatinText, FontRole::LatinText],
        &[EastAsianSpacingEdges {
            leading: EastAsianSpacingValue::Wide,
            trailing: EastAsianSpacingValue::Wide,
            contains_wide: false,
        }; 2],
        &HashSet::new(),
        &[InlineAttachment::None],
    );
}

#[test]
fn resolve_unicode_punctuation_boundaries_with_punctuation_and_space() {
    let text = Text::from("中 。");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &[
            Cluster::new(TextRange::new(0, 1), Text::from("中"), "cjk".to_owned(), 16.0),
            Cluster::new(TextRange::new(1, 2), Text::from(" "), "latin".to_owned(), 16.0),
            Cluster::new(TextRange::new(2, 3), Text::from("。"), "cjk".to_owned(), 16.0),
        ],
        &[FontRole::CjkText; 3],
        &[],
    );
    assert!(result
        .decisions
        .iter()
        .any(|decision| decision.forbidden_position == "LineStart"));
}

#[test]
fn resolve_unicode_punctuation_boundaries_with_follows_authored_boundary() {
    let text = Text::from("\n（中文");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &[
            Cluster::new(TextRange::new(0, 1), Text::from("\n"), "latin".to_owned(), 16.0),
            Cluster::new(TextRange::new(1, 2), Text::from("（"), "cjk".to_owned(), 16.0),
            Cluster::new(TextRange::new(2, 3), Text::from("中"), "cjk".to_owned(), 16.0),
            Cluster::new(TextRange::new(3, 4), Text::from("文"), "cjk".to_owned(), 16.0),
        ],
        &[FontRole::CjkText; 4],
        &[],
    );
    assert!(!result.decisions.iter().any(|decision| {
        decision.source_text == "（" && decision.forbidden_position == "LineStart"
    }));
}

#[test]
fn resolve_unicode_punctuation_boundaries_with_close_punctuation_class() {
    let text = Text::from("中。");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &clusters(text.as_str(), "cjk", 16.0),
        &[FontRole::CjkText; 2],
        &[],
    );
    assert!(result
        .decisions
        .iter()
        .any(|decision| decision.forbidden_position == "LineStart"));
}

#[test]
fn resolve_unicode_punctuation_boundaries_with_infix_numeric_separator() {
    let text = Text::from("1，2");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &[
            Cluster::new(TextRange::new(0, 1), Text::from("1"), "latin".to_owned(), 8.0),
            Cluster::new(TextRange::new(1, 2), Text::from("，"), "cjk".to_owned(), 16.0),
            Cluster::new(TextRange::new(2, 3), Text::from("2"), "latin".to_owned(), 8.0),
        ],
        &[FontRole::LatinText, FontRole::CjkPunctuation, FontRole::LatinText],
        &[],
    );
    assert!(result.forbidden_line_start_clusters.is_empty());
}

#[test]
fn resolve_unicode_punctuation_boundaries_with_decimal_mark_after_space() {
    let text = Text::from("1 ，2");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &[
            Cluster::new(TextRange::new(0, 1), Text::from("1"), "latin".to_owned(), 8.0),
            Cluster::new(TextRange::new(1, 2), Text::from(" "), "latin".to_owned(), 8.0),
            Cluster::new(TextRange::new(2, 3), Text::from("，"), "cjk".to_owned(), 16.0),
            Cluster::new(TextRange::new(3, 4), Text::from("2"), "latin".to_owned(), 8.0),
        ],
        &[
            FontRole::LatinText,
            FontRole::LatinText,
            FontRole::CjkPunctuation,
            FontRole::LatinText,
        ],
        &[],
    );
    assert!(result.forbidden_line_start_clusters.is_empty());
}

#[test]
fn resolve_unicode_punctuation_boundaries_previous_content_cluster_returns_null() {
    let text = Text::from("  !");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &clusters(text.as_str(), "test", 8.0),
        &[FontRole::LatinText; 3],
        &[],
    );
    assert!(result.forbidden_line_start_clusters.is_empty());
}

#[test]
fn resolve_unicode_punctuation_boundaries_with_infix_numeric_separator_not_decimal_mark() {
    let text = Text::from("1，");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &[
            Cluster::new(TextRange::new(0, 1), Text::from("1"), "latin".to_owned(), 8.0),
            Cluster::new(TextRange::new(1, 2), Text::from("，"), "cjk".to_owned(), 16.0),
        ],
        &[FontRole::LatinText, FontRole::LatinText],
        &[],
    );
    assert!(!result.forbidden_line_start_clusters.is_empty());
}

#[test]
fn resolve_unicode_punctuation_boundaries_with_decimal_mark_after_non_space() {
    let text = Text::from("1,，2");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &[
            Cluster::new(TextRange::new(0, 1), Text::from("1"), "latin".to_owned(), 8.0),
            Cluster::new(TextRange::new(1, 2), Text::from(","), "latin".to_owned(), 8.0),
            Cluster::new(TextRange::new(2, 3), Text::from("，"), "cjk".to_owned(), 16.0),
            Cluster::new(TextRange::new(3, 4), Text::from("2"), "latin".to_owned(), 8.0),
        ],
        &[FontRole::LatinText, FontRole::LatinText, FontRole::CjkPunctuation, FontRole::LatinText],
        &[],
    );
    assert!(!result.forbidden_line_start_clusters.is_empty());
}

#[test]
fn resolve_unicode_punctuation_boundaries_with_quote_direction_final() {
    let text = Text::from("中”中");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &clusters(text.as_str(), "cjk", 16.0),
        &[FontRole::CjkText; 3],
        &[],
    );
    assert!(result
        .decisions
        .iter()
        .any(|decision| decision.forbidden_position == "LineStart"));
}

#[test]
fn resolve_unicode_punctuation_boundaries_with_quote_direction_initial() {
    let text = Text::from("中“中");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &clusters(text.as_str(), "cjk", 16.0),
        &[FontRole::CjkText; 3],
        &[],
    );
    assert!(result
        .decisions
        .iter()
        .any(|decision| decision.forbidden_position == "LineEnd"));
}

#[test]
fn resolve_unicode_punctuation_boundaries_with_quote_direction_unresolved() {
    let text = Text::from("中«中");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &clusters(text.as_str(), "cjk", 16.0),
        &[FontRole::CjkText; 3],
        &[],
    );
    assert!(result
        .decisions
        .iter()
        .any(|decision| decision.forbidden_position == "LineEnd"));
}

#[test]
fn resolve_unicode_punctuation_boundaries_with_word_apostrophe_2019() {
    let text = Text::from("it’s");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &clusters(text.as_str(), "latin", 8.0),
        &[FontRole::LatinText; 4],
        &[],
    );
    assert!(result.forbidden_line_start_clusters.is_empty());
}

#[test]
fn resolve_unicode_punctuation_boundaries_with_latin_word_code_point() {
    let text = Text::from("café");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &clusters(text.as_str(), "latin", 8.0),
        &[FontRole::LatinText; 4],
        &[],
    );
    assert!(result.decisions.is_empty());
}

#[test]
fn resolve_unicode_punctuation_boundaries_with_first_significant_code_point() {
    let text = Text::from("  “");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &clusters(text.as_str(), "latin", 8.0),
        &[FontRole::LatinText; 3],
        &[],
    );
    assert!(result
        .decisions
        .iter()
        .any(|decision| decision.forbidden_position == "LineEnd"));
}

#[test]
fn resolve_unicode_punctuation_boundaries_with_last_significant_code_point() {
    let text = Text::from("a”  ");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &clusters(text.as_str(), "latin", 8.0),
        &[FontRole::LatinText; 4],
        &[],
    );
    assert!(result
        .decisions
        .iter()
        .any(|decision| decision.forbidden_position == "LineStart"));
}

#[test]
#[should_panic(expected = "Clusters, roles and East_Asian_Spacing edges must align.")]
fn resolve_attached_inline_inter_char_boundaries_requires_matching_edges_size() {
    let text = Text::from("a");
    let _ = resolve_attached_inline_inter_char_boundaries(
        &text,
        &[Cluster::new(TextRange::new(0, 1), Text::from("a"), "latin".to_owned(), 8.0)],
        &[FontRole::LatinText],
        &[EastAsianSpacingEdges {
            leading: EastAsianSpacingValue::Wide,
            trailing: EastAsianSpacingValue::Wide,
            contains_wide: false,
        }; 2],
        &HashSet::new(),
        &[InlineAttachment::None],
    );
}

#[test]
fn resolve_attached_inline_inter_char_boundaries_punctuation_western_narrow_trailing() {
    let text = Text::from("a,。");
    let result = resolve_attached_inline_inter_char_boundaries(
        &text,
        &[
            Cluster::new(TextRange::new(0, 1), Text::from("a"), "latin".to_owned(), 8.0),
            Cluster::new(TextRange::new(1, 2), Text::from(","), "latin".to_owned(), 8.0),
            Cluster::new(TextRange::new(2, 3), Text::from("。"), "cjk".to_owned(), 16.0),
        ],
        &[FontRole::LatinText, FontRole::CjkPunctuation, FontRole::CjkPunctuation],
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
                leading: EastAsianSpacingValue::Wide,
                trailing: EastAsianSpacingValue::Wide,
                contains_wide: false,
            },
        ],
        &HashSet::new(),
        &[InlineAttachment::None, InlineAttachment::Previous, InlineAttachment::None],
    );
    assert!(!result.virtual_boundary_after_clusters.is_empty());
}

#[test]
fn resolve_attached_inline_inter_char_boundaries_punctuation_western_trailing_not_narrow() {
    let text = Text::from("a,中");
    let result = resolve_attached_inline_inter_char_boundaries(
        &text,
        &[
            Cluster::new(TextRange::new(0, 1), Text::from("a"), "latin".to_owned(), 8.0),
            Cluster::new(TextRange::new(1, 2), Text::from(","), "latin".to_owned(), 8.0),
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
fn resolve_attached_inline_inter_char_boundaries_punctuation_western_trailing_narrow_not_cjk_punct() {
    let text = Text::from("a,中");
    let result = resolve_attached_inline_inter_char_boundaries(
        &text,
        &[
            Cluster::new(TextRange::new(0, 1), Text::from("a"), "latin".to_owned(), 8.0),
            Cluster::new(TextRange::new(1, 2), Text::from(","), "latin".to_owned(), 8.0),
            Cluster::new(TextRange::new(2, 3), Text::from("中"), "cjk".to_owned(), 16.0),
        ],
        &[FontRole::LatinText, FontRole::CjkPunctuation, FontRole::CjkText],
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
                leading: EastAsianSpacingValue::Wide,
                trailing: EastAsianSpacingValue::Wide,
                contains_wide: false,
            },
        ],
        &HashSet::new(),
        &[InlineAttachment::None, InlineAttachment::Previous, InlineAttachment::None],
    );
    assert!(!result.virtual_boundary_after_clusters.is_empty());
}
