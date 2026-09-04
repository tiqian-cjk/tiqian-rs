use tiqian::core::geometry::{text_range};
use tiqian::core::layout_model::Cluster;
use tiqian::core::text::Text;
use tiqian::font::font_policy::FontRole;
use tiqian::layout::unicode_punctuation_boundary_resolver::resolve_unicode_punctuation_boundaries;

fn clusters(text: &str, font_key: &str, advance: f32) -> Vec<Cluster> {
    let mut offset = 0;
    text.chars()
        .map(|character| {
            let end = offset + 1;
            let cluster = Cluster::new(
                text_range(offset, end),
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
fn resolve_unicode_punctuation_boundaries_with_has_authored_break() {
    let text = Text::from("\n“");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &clusters(text.as_str(), "latin", 8.0),
        &[FontRole::LatinText; 2],
        &[],
    );
    assert!(result.forbidden_line_start_clusters.is_empty());
}

#[test]
fn resolve_unicode_punctuation_boundaries_with_next_content_cluster() {
    let text = Text::from("a”中");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &clusters(text.as_str(), "latin", 8.0),
        &[FontRole::LatinText; 3],
        &[],
    );
    assert!(!result.unbreakable_ranges.is_empty());
}

#[test]
fn resolve_unicode_punctuation_boundaries_with_previous_content_cluster_has_content() {
    let text = Text::from("中”");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &clusters(text.as_str(), "cjk", 16.0),
        &[FontRole::CjkText; 2],
        &[],
    );
    assert!(!result.unbreakable_ranges.is_empty());
}

#[test]
fn resolve_unicode_punctuation_boundaries_with_close_punctuation() {
    let text = Text::from("中）");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &clusters(text.as_str(), "cjk", 16.0),
        &[FontRole::CjkText; 2],
        &[],
    );
    assert!(!result.forbidden_line_start_clusters.is_empty());
}

#[test]
fn resolve_unicode_punctuation_boundaries_with_exclamation_class() {
    let text = Text::from("中！");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &clusters(text.as_str(), "cjk", 16.0),
        &[FontRole::CjkText; 2],
        &[],
    );
    assert!(!result.forbidden_line_start_clusters.is_empty());
}

#[test]
fn resolve_unicode_punctuation_boundaries_with_close_parenthesis_class() {
    let text = Text::from("中）");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &clusters(text.as_str(), "cjk", 16.0),
        &[FontRole::CjkText; 2],
        &[],
    );
    assert!(result.decisions.iter().any(|decision| decision.reason.contains("LB13")));
}

#[test]
fn resolve_unicode_punctuation_boundaries_with_infix_numeric_separator_rule() {
    let text = Text::from("1,2");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &clusters(text.as_str(), "latin", 8.0),
        &[FontRole::LatinText; 3],
        &[],
    );
    let infix = result
        .decisions
        .iter()
        .find(|decision| decision.source_text == ",")
        .unwrap();
    assert!(infix.reason.contains("LB15d"));
}

#[test]
fn resolve_unicode_punctuation_boundaries_with_rule_for_line_start_else() {
    let text = Text::from("中、");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &clusters(text.as_str(), "cjk", 16.0),
        &[FontRole::CjkText; 2],
        &[],
    );
    assert!(!result.forbidden_line_start_clusters.is_empty());
}

#[test]
fn resolve_unicode_punctuation_boundaries_with_next_content_cluster_has_authored_break() {
    let text = Text::from("”\n");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &clusters(text.as_str(), "latin", 8.0),
        &[FontRole::LatinText; 2],
        &[],
    );
    assert!(result.unbreakable_ranges.is_empty());
}

#[test]
fn resolve_unicode_punctuation_boundaries_with_is_whitespace_code_point_non_bmp() {
    let text = Text::from("😀");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &[Cluster::new(text_range(0, 1), Text::from("😀"), "latin".to_owned(), 8.0)],
        &[FontRole::LatinText],
        &[],
    );
    assert!(result.decisions.is_empty());
}

#[test]
fn resolve_unicode_punctuation_boundaries_with_has_authored_break_both() {
    let text = Text::from("\n“\n");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &clusters(text.as_str(), "latin", 8.0),
        &[FontRole::LatinText; 3],
        &[],
    );
    assert!(!result.decisions.is_empty());
}

#[test]
fn resolve_unicode_punctuation_boundaries_with_follows_authored_boundary_whitespace() {
    let text = Text::from(" “");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &clusters(text.as_str(), "latin", 8.0),
        &[FontRole::LatinText; 2],
        &[],
    );
    assert!(result.forbidden_line_start_clusters.is_empty());
}

#[test]
fn resolve_unicode_punctuation_boundaries_with_follows_authored_boundary_whitespace_then_non_whitespace() {
    let text = Text::from(" A“");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &clusters(text.as_str(), "latin", 8.0),
        &[FontRole::LatinText; 3],
        &[],
    );
    assert!(!result.decisions.is_empty() || !result.forbidden_line_start_clusters.is_empty());
}

#[test]
fn resolve_unicode_punctuation_boundaries_with_previous_content_cluster_empty() {
    let text = Text::from("“");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &[Cluster::new(text_range(0, 1), Text::from("“"), "latin".to_owned(), 16.0)],
        &[FontRole::LatinText],
        &[],
    );
    assert!(!result.decisions.is_empty()
        || !result.forbidden_line_start_clusters.is_empty()
        || !result.unbreakable_ranges.is_empty());
}

#[test]
fn resolve_unicode_punctuation_boundaries_with_next_content_cluster_empty() {
    let text = Text::from("a”b");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &clusters(text.as_str(), "latin", 8.0),
        &[FontRole::LatinText; 3],
        &[],
    );
    assert!(!result.forbidden_line_start_clusters.is_empty() || !result.unbreakable_ranges.is_empty());
}

#[test]
fn resolve_unicode_punctuation_boundaries_with_code_point_at_or_null_surrogate() {
    let text = Text::from("“😀");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &[
            Cluster::new(text_range(0, 1), Text::from("“"), "latin".to_owned(), 16.0),
            Cluster::new(text_range(1, 2), Text::from("😀"), "emoji".to_owned(), 16.0),
        ],
        &[FontRole::LatinText, FontRole::Emoji],
        &[],
    );
    assert!(!result.decisions.is_empty() || !result.forbidden_line_start_clusters.is_empty());
}

#[test]
fn resolve_unicode_punctuation_boundaries_with_empty_range() {
    let text = Text::from("中");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &[
            Cluster::new(text_range(0, 0), Text::default(), "cjk".to_owned(), 0.0),
            Cluster::new(text_range(0, 1), Text::from("中"), "cjk".to_owned(), 16.0),
        ],
        &[FontRole::CjkText; 2],
        &[],
    );
    assert!(result.decisions.is_empty());
}

#[test]
fn resolve_unicode_punctuation_boundaries_with_first_code_point_length() {
    let text = Text::from("中");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &clusters(text.as_str(), "cjk", 16.0),
        &[FontRole::CjkText],
        &[],
    );
    assert!(result.decisions.is_empty());
}

#[test]
fn resolve_unicode_punctuation_boundaries_with_follows_authored_boundary_mandatory() {
    let text = Text::from("\r“");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &clusters(text.as_str(), "latin", 8.0),
        &[FontRole::LatinText; 2],
        &[],
    );
    assert!(result.forbidden_line_start_clusters.is_empty());
}

#[test]
fn resolve_unicode_punctuation_boundaries_with_follows_authored_boundary_zwsp() {
    let text = Text::from("\u{200B}“");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &clusters(text.as_str(), "latin", 8.0),
        &[FontRole::LatinText; 2],
        &[],
    );
    assert!(result.forbidden_line_start_clusters.is_empty());
}

#[test]
fn resolve_unicode_punctuation_boundaries_with_decimal_mark_following_inside_digit() {
    let text = Text::from(" 1，23");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &[
            Cluster::new(text_range(0, 1), Text::from(" "), "latin".to_owned(), 8.0),
            Cluster::new(text_range(1, 2), Text::from("1"), "latin".to_owned(), 8.0),
            Cluster::new(text_range(2, 3), Text::from("，"), "cjk".to_owned(), 16.0),
            Cluster::new(text_range(3, 5), Text::from("23"), "latin".to_owned(), 8.0),
        ],
        &[FontRole::LatinText, FontRole::LatinText, FontRole::CjkPunctuation, FontRole::LatinText],
        &[],
    );
    assert!(result.forbidden_line_start_clusters.is_empty());
}

#[test]
fn resolve_unicode_punctuation_boundaries_with_decimal_mark_following_outside_digit() {
    let text = Text::from(" a，2");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &[
            Cluster::new(text_range(0, 1), Text::from(" "), "latin".to_owned(), 8.0),
            Cluster::new(text_range(1, 2), Text::from("a"), "latin".to_owned(), 8.0),
            Cluster::new(text_range(2, 3), Text::from("，"), "cjk".to_owned(), 16.0),
            Cluster::new(text_range(3, 4), Text::from("2"), "latin".to_owned(), 8.0),
        ],
        &[FontRole::LatinText; 4],
        &[],
    );
    assert!(!result.decisions.is_empty() || !result.forbidden_line_start_clusters.is_empty());
}

#[test]
fn resolve_unicode_punctuation_boundaries_with_previous_content_cluster_has_authored_break() {
    let text = Text::from("\n（");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &clusters(text.as_str(), "latin", 8.0),
        &[FontRole::LatinText; 2],
        &[],
    );
    assert!(result.forbidden_line_start_clusters.is_empty());
}

#[test]
fn resolve_unicode_punctuation_boundaries_decimal_mark_at_cluster_zero_forbidden() {
    let text = Text::from("a.5");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &[Cluster::new(text_range(1, 3), Text::from(".5"), "latin".to_owned(), 16.0)],
        &[FontRole::LatinText],
        &[],
    );
    assert_eq!(1, result.forbidden_line_start_clusters.len());
}

#[test]
fn resolve_unicode_punctuation_boundaries_decimal_mark_after_letter_cluster_forbidden() {
    let text = Text::from("a.5");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &[
            Cluster::new(text_range(0, 1), Text::from("a"), "latin".to_owned(), 8.0),
            Cluster::new(text_range(1, 3), Text::from(".5"), "latin".to_owned(), 16.0),
        ],
        &[FontRole::LatinText; 2],
        &[],
    );
    assert_eq!(1, result.forbidden_line_start_clusters.len());
}

#[test]
fn resolve_unicode_punctuation_boundaries_decimal_mark_followed_by_letter_forbidden() {
    let text = Text::from("a .x");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &[
            Cluster::new(text_range(0, 1), Text::from("a"), "latin".to_owned(), 8.0),
            Cluster::new(text_range(1, 2), Text::from(" "), "latin".to_owned(), 8.0),
            Cluster::new(text_range(2, 4), Text::from(".x"), "latin".to_owned(), 16.0),
        ],
        &[FontRole::LatinText; 3],
        &[],
    );
    assert_eq!(1, result.forbidden_line_start_clusters.len());
}

#[test]
fn resolve_unicode_punctuation_boundaries_decimal_mark_alone_after_space_forbidden() {
    let text = Text::from("a .");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &[
            Cluster::new(text_range(0, 1), Text::from("a"), "latin".to_owned(), 8.0),
            Cluster::new(text_range(1, 2), Text::from(" "), "latin".to_owned(), 8.0),
            Cluster::new(text_range(2, 3), Text::from("."), "latin".to_owned(), 8.0),
        ],
        &[FontRole::LatinText; 3],
        &[],
    );
    assert_eq!(1, result.forbidden_line_start_clusters.len());
}

#[test]
fn resolve_unicode_punctuation_boundaries_astral_tail_keeps_pair_as_last_significant() {
    let text = Text::from("a .😀");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &[
            Cluster::new(text_range(0, 1), Text::from("a"), "latin".to_owned(), 8.0),
            Cluster::new(text_range(1, 2), Text::from(" "), "latin".to_owned(), 8.0),
            Cluster::new(text_range(2, 4), Text::from(".😀"), "latin".to_owned(), 16.0),
        ],
        &[FontRole::LatinText; 3],
        &[],
    );
    assert_eq!(1, result.forbidden_line_start_clusters.len());
}

#[test]
fn resolve_unicode_punctuation_boundaries_authored_break_inside_previous_cluster_drops_unbreakable() {
    let text = Text::from("a\nb，");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &[
            Cluster::new(text_range(0, 3), Text::from("a\nb"), "latin".to_owned(), 24.0),
            Cluster::new(text_range(3, 4), Text::from("，"), "latin".to_owned(), 16.0),
        ],
        &[FontRole::LatinText; 2],
        &[],
    );
    assert_eq!(1, result.forbidden_line_start_clusters.len());
    assert!(result.unbreakable_ranges.is_empty());
}

#[test]
fn resolve_unicode_punctuation_boundaries_apostrophe_at_text_start_no_left_context() {
    let text = Text::from("’s");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &[
            Cluster::new(text_range(0, 1), Text::from("’"), "latin".to_owned(), 16.0),
            Cluster::new(text_range(1, 2), Text::from("s"), "latin".to_owned(), 8.0),
        ],
        &[FontRole::LatinText; 2],
        &[],
    );
    assert!(result.forbidden_line_start_clusters.is_empty());
}

#[test]
fn resolve_unicode_punctuation_boundaries_apostrophe_left_neighbour_supplementary_pair() {
    let text = Text::from("😀’");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &[
            Cluster::new(text_range(0, 1), Text::from("😀"), "emoji".to_owned(), 16.0),
            Cluster::new(text_range(1, 2), Text::from("’"), "latin".to_owned(), 16.0),
        ],
        &[FontRole::Emoji, FontRole::LatinText],
        &[],
    );
    assert!(result
        .decisions
        .iter()
        .any(|decision| decision.forbidden_position == "LineStart"));
}

#[test]
fn resolve_unicode_punctuation_boundaries_decimal_mark_after_empty_cluster_forbidden() {
    let text = Text::from("a.5");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &[
            Cluster::new(text_range(0, 1), Text::from("a"), "latin".to_owned(), 8.0),
            Cluster::new(text_range(1, 1), Text::default(), "latin".to_owned(), 0.0),
            Cluster::new(text_range(1, 3), Text::from(".5"), "latin".to_owned(), 16.0),
        ],
        &[FontRole::LatinText; 3],
        &[],
    );
    assert_eq!(1, result.forbidden_line_start_clusters.len());
}

#[test]
fn resolve_unicode_punctuation_boundaries_apostrophe_right_neighbour_supplementary_pair() {
    let text = Text::from("’😀");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &[Cluster::new(text_range(0, 2), Text::from("’😀"), "latin".to_owned(), 32.0)],
        &[FontRole::LatinText],
        &[],
    );
    assert!(result.decisions.is_empty());
}

#[test]
fn resolve_unicode_punctuation_boundaries_with_open_punctuation() {
    let text = Text::from("（中文）");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &clusters(text.as_str(), "cjk", 16.0),
        &[FontRole::CjkText; 4],
        &[],
    );
    assert!(result
        .decisions
        .iter()
        .any(|decision| decision.forbidden_position == "LineEnd"));
}

#[test]
fn resolve_unicode_punctuation_boundaries_with_cjk_closing_forbid_line_start() {
    let text = Text::from("中。中");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &clusters(text.as_str(), "cjk", 16.0),
        &[FontRole::CjkText; 3],
        &[],
    );
    assert!(!result.forbidden_line_start_clusters.is_empty());
}

#[test]
fn resolve_unicode_punctuation_boundaries_with_code_point_before_supplementary() {
    let text = Text::from("中”");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &clusters(text.as_str(), "cjk", 16.0),
        &[FontRole::CjkText; 2],
        &[],
    );
    assert!(!result.decisions.is_empty());
}

#[test]
fn resolve_unicode_punctuation_boundaries_with_is_whitespace_code_point() {
    let text = Text::from(" “");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &clusters(text.as_str(), "latin", 8.0),
        &[FontRole::LatinText; 2],
        &[],
    );
    assert!(result
        .decisions
        .iter()
        .any(|decision| decision.forbidden_position == "LineEnd"));
}

#[test]
fn resolve_unicode_punctuation_boundaries_with_has_authored_break_mandatory_only() {
    let text = Text::from("\r“");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &clusters(text.as_str(), "latin", 8.0),
        &[FontRole::LatinText; 2],
        &[],
    );
    assert!(result.forbidden_line_start_clusters.is_empty());
}

#[test]
fn resolve_unicode_punctuation_boundaries_with_code_point_before_surrogate_pair() {
    let text = Text::from("😀”");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &[
            Cluster::new(text_range(0, 1), Text::from("😀"), "emoji".to_owned(), 16.0),
            Cluster::new(text_range(1, 2), Text::from("”"), "latin".to_owned(), 16.0),
        ],
        &[FontRole::Emoji, FontRole::LatinText],
        &[],
    );
    assert!(!result.decisions.is_empty() || !result.forbidden_line_start_clusters.is_empty());
}

#[test]
fn resolve_unicode_punctuation_boundaries_with_code_point_at_or_null_supplementary() {
    let text = Text::from("😀“");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &[
            Cluster::new(text_range(0, 1), Text::from("😀"), "emoji".to_owned(), 16.0),
            Cluster::new(text_range(1, 2), Text::from("“"), "latin".to_owned(), 16.0),
        ],
        &[FontRole::Emoji, FontRole::LatinText],
        &[],
    );
    assert!(!result.decisions.is_empty() || !result.forbidden_line_start_clusters.is_empty());
}

#[test]
fn resolve_unicode_punctuation_boundaries_is_decimal_mark_after_space_index_zero() {
    let text = Text::from(".5");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &clusters(text.as_str(), "latin", 8.0),
        &[FontRole::LatinText; 2],
        &[],
    );
    assert!(result.forbidden_line_start_clusters.is_empty());
}

#[test]
fn resolve_unicode_punctuation_boundaries_is_decimal_mark_after_space_non_whitespace_prev() {
    let text = Text::from("1，2");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &[
            Cluster::new(text_range(0, 1), Text::from("1"), "latin".to_owned(), 8.0),
            Cluster::new(text_range(1, 2), Text::from("，"), "latin".to_owned(), 16.0),
            Cluster::new(text_range(2, 3), Text::from("2"), "latin".to_owned(), 8.0),
        ],
        &[FontRole::LatinText; 3],
        &[],
    );
    assert!(!result.forbidden_line_start_clusters.is_empty());
}

#[test]
fn resolve_unicode_punctuation_boundaries_is_decimal_mark_after_space_empty_prev() {
    let text = Text::from("a，5");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &[
            Cluster::new(text_range(0, 1), Text::from("a"), "latin".to_owned(), 8.0),
            Cluster::new(text_range(1, 2), Text::from("，"), "latin".to_owned(), 16.0),
            Cluster::new(text_range(2, 3), Text::from("5"), "latin".to_owned(), 8.0),
        ],
        &[FontRole::LatinText; 3],
        &[],
    );
    assert!(!result.forbidden_line_start_clusters.is_empty());
}

#[test]
fn resolve_unicode_punctuation_boundaries_follows_authored_boundary_non_whitespace() {
    let text = Text::from("a“");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &clusters(text.as_str(), "latin", 8.0),
        &[FontRole::LatinText; 2],
        &[],
    );
    assert!(result
        .decisions
        .iter()
        .any(|decision| decision.forbidden_position == "LineEnd"));
}

#[test]
fn resolve_unicode_punctuation_boundaries_previous_content_cluster_returns_content() {
    let text = Text::from("a ”");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &clusters(text.as_str(), "latin", 8.0),
        &[FontRole::LatinText; 3],
        &[],
    );
    assert!(!result.unbreakable_ranges.is_empty());
}

#[test]
fn resolve_unicode_punctuation_boundaries_previous_content_cluster_empty_only() {
    let text = Text::from("“");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &[
            Cluster::new(text_range(0, 0), Text::default(), "latin".to_owned(), 0.0),
            Cluster::new(text_range(0, 1), Text::from("“"), "latin".to_owned(), 16.0),
        ],
        &[FontRole::LatinText; 2],
        &[],
    );
    assert!(!result.forbidden_line_end_clusters.is_empty());
}

#[test]
fn resolve_unicode_punctuation_boundaries_next_content_cluster_returns_content() {
    let text = Text::from(")”中");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &[
            Cluster::new(text_range(0, 1), Text::from(")"), "latin".to_owned(), 8.0),
            Cluster::new(text_range(1, 2), Text::from("”"), "latin".to_owned(), 8.0),
            Cluster::new(text_range(2, 3), Text::from("中"), "cjk".to_owned(), 16.0),
        ],
        &[FontRole::LatinText, FontRole::LatinText, FontRole::CjkText],
        &[],
    );
    assert!(!result.unbreakable_ranges.is_empty());
}

#[test]
fn resolve_unicode_punctuation_boundaries_has_authored_break_with_code_point() {
    let text = Text::from("a\n“");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &[
            Cluster::new(text_range(0, 1), Text::from("a"), "latin".to_owned(), 8.0),
            Cluster::new(text_range(1, 2), Text::from("\n"), "latin".to_owned(), 0.0),
            Cluster::new(text_range(2, 3), Text::from("“"), "latin".to_owned(), 8.0),
        ],
        &[FontRole::LatinText; 3],
        &[],
    );
    assert!(result.forbidden_line_start_clusters.is_empty());
}

#[test]
fn resolve_unicode_punctuation_boundaries_has_authored_break_null_code_point() {
    let text = Text::from("“");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &[Cluster::new(text_range(0, 1), Text::from("“"), "latin".to_owned(), 16.0)],
        &[FontRole::LatinText],
        &[],
    );
    assert!(!result.forbidden_line_end_clusters.is_empty());
}

#[test]
fn resolve_unicode_punctuation_boundaries_first_code_point_length_bmp() {
    let text = Text::from("a“");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &clusters(text.as_str(), "latin", 8.0),
        &[FontRole::LatinText; 2],
        &[],
    );
    assert!(!result.decisions.is_empty());
}

#[test]
fn resolve_unicode_punctuation_boundaries_first_code_point_length_surrogate() {
    let text = Text::from("😀“");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &[
            Cluster::new(text_range(0, 1), Text::from("😀"), "emoji".to_owned(), 16.0),
            Cluster::new(text_range(1, 2), Text::from("“"), "latin".to_owned(), 16.0),
        ],
        &[FontRole::Emoji, FontRole::LatinText],
        &[],
    );
    assert!(!result.decisions.is_empty());
}

#[test]
fn resolve_unicode_punctuation_boundaries_code_point_at_or_null_surrogate_pair() {
    let text = Text::from("😀“");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &[
            Cluster::new(text_range(0, 1), Text::from("😀"), "emoji".to_owned(), 16.0),
            Cluster::new(text_range(1, 2), Text::from("“"), "latin".to_owned(), 16.0),
        ],
        &[FontRole::Emoji, FontRole::LatinText],
        &[],
    );
    assert!(!result.decisions.is_empty());
}

#[test]
fn resolve_unicode_punctuation_boundaries_code_point_before_surrogate_pair() {
    let text = Text::from("😀");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &[Cluster::new(text_range(0, 1), Text::from("😀"), "emoji".to_owned(), 16.0)],
        &[FontRole::Emoji],
        &[],
    );
    assert!(result.decisions.is_empty());
}
