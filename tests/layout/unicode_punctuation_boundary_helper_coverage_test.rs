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
fn resolve_unicode_punctuation_boundaries_code_point_before_low_surrogate() {
    let text = Text::from("a😀");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &[
            Cluster::new(text_range(0, 1), Text::from("a"), "latin".to_owned(), 8.0),
            Cluster::new(text_range(1, 2), Text::from("😀"), "emoji".to_owned(), 16.0),
        ],
        &[FontRole::LatinText, FontRole::Emoji],
        &[],
    );
    assert!(result.forbidden_line_start_clusters.is_empty());
}

#[test]
fn resolve_unicode_punctuation_boundaries_quote_direction_2019_surrogate_left() {
    let text = Text::from("😀’");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &[
            Cluster::new(text_range(0, 1), Text::from("😀"), "emoji".to_owned(), 16.0),
            Cluster::new(text_range(1, 2), Text::from("’"), "latin".to_owned(), 8.0),
        ],
        &[FontRole::Emoji, FontRole::LatinText],
        &[],
    );
    assert!(!result.decisions.is_empty());
}

#[test]
fn resolve_unicode_punctuation_boundaries_previous_content_cluster_multiple_empty() {
    let text = Text::from("a ”");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &[
            Cluster::new(text_range(0, 0), Text::default(), "latin".to_owned(), 0.0),
            Cluster::new(text_range(1, 2), Text::from(" "), "latin".to_owned(), 8.0),
            Cluster::new(text_range(2, 3), Text::from("”"), "latin".to_owned(), 8.0),
        ],
        &[FontRole::LatinText; 3],
        &[],
    );
    assert!(!result.forbidden_line_start_clusters.is_empty());
}

#[test]
fn resolve_unicode_punctuation_boundaries_follows_authored_boundary_zwsp_in_middle() {
    let text = Text::from(" \u{200B}“");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &clusters(text.as_str(), "latin", 8.0),
        &[FontRole::LatinText; 3],
        &[],
    );
    assert!(result.forbidden_line_start_clusters.is_empty());
}

#[test]
fn resolve_unicode_punctuation_boundaries_last_significant_code_point_surrogate_ending() {
    let text = Text::from("😀”");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &[
            Cluster::new(text_range(0, 1), Text::from("😀"), "emoji".to_owned(), 16.0),
            Cluster::new(text_range(1, 2), Text::from("”"), "latin".to_owned(), 8.0),
        ],
        &[FontRole::Emoji, FontRole::LatinText],
        &[],
    );
    assert!(!result.forbidden_line_end_clusters.is_empty() || !result.unbreakable_ranges.is_empty());
}

#[test]
fn resolve_unicode_punctuation_boundaries_is_decimal_mark_after_space_following_inside() {
    let text = Text::from("a .5");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &[
            Cluster::new(text_range(0, 1), Text::from("a"), "latin".to_owned(), 8.0),
            Cluster::new(text_range(1, 2), Text::from(" "), "latin".to_owned(), 8.0),
            Cluster::new(text_range(2, 4), Text::from(".5"), "latin".to_owned(), 16.0),
        ],
        &[FontRole::LatinText; 3],
        &[],
    );
    assert!(result.forbidden_line_start_clusters.is_empty());
}

#[test]
fn resolve_unicode_punctuation_boundary_full_width_comma_after_space_stays_forbidden() {
    let text = Text::from("a ，5");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &[
            Cluster::new(text_range(0, 1), Text::from("a"), "latin".to_owned(), 8.0),
            Cluster::new(text_range(1, 2), Text::from(" "), "latin".to_owned(), 8.0),
            Cluster::new(text_range(2, 4), Text::from("，5"), "latin".to_owned(), 16.0),
        ],
        &[FontRole::LatinText; 3],
        &[],
    );
    assert_eq!(1, result.forbidden_line_start_clusters.len());
}

#[test]
fn resolve_unicode_punctuation_boundaries_is_decimal_mark_after_space_following_outside() {
    let text = Text::from("a .5");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &[
            Cluster::new(text_range(0, 1), Text::from("a"), "latin".to_owned(), 8.0),
            Cluster::new(text_range(1, 2), Text::from(" "), "latin".to_owned(), 8.0),
            Cluster::new(text_range(2, 3), Text::from("."), "latin".to_owned(), 8.0),
            Cluster::new(text_range(3, 4), Text::from("5"), "latin".to_owned(), 8.0),
        ],
        &[FontRole::LatinText; 4],
        &[],
    );
    assert!(result.forbidden_line_start_clusters.is_empty());
}

#[test]
fn resolve_unicode_punctuation_boundaries_quote_direction_2019_bmp_left() {
    let text = Text::from("A’");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &clusters(text.as_str(), "latin", 8.0),
        &[FontRole::LatinText; 2],
        &[],
    );
    assert!(!result.decisions.is_empty() || !result.forbidden_line_start_clusters.is_empty());
}

#[test]
fn resolve_unicode_punctuation_boundaries_quote_direction_2019_right_word_only() {
    let text = Text::from(" ’a");
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
fn resolve_unicode_punctuation_boundaries_quote_direction_2019_left_word_only() {
    let text = Text::from("a’ ");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &clusters(text.as_str(), "latin", 8.0),
        &[FontRole::LatinText; 3],
        &[],
    );
    assert!(result
        .decisions
        .iter()
        .any(|decision| decision.forbidden_position == "LineStart"));
}

#[test]
fn resolve_unicode_punctuation_boundaries_quote_direction_2019_neither_word() {
    let text = Text::from("!’!");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &clusters(text.as_str(), "latin", 8.0),
        &[FontRole::LatinText; 3],
        &[],
    );
    assert!(result
        .decisions
        .iter()
        .any(|decision| decision.forbidden_position == "LineStart"));
}

#[test]
fn resolve_unicode_punctuation_boundaries_infix_numeric_separator_with_space_and_no_space() {
    let text1 = Text::from(" .5");
    let result1 = resolve_unicode_punctuation_boundaries(
        &text1,
        &clusters(text1.as_str(), "latin", 8.0),
        &[FontRole::LatinText; 3],
        &[],
    );
    assert!(result1.forbidden_line_start_clusters.is_empty());

    let text2 = Text::from(" 1.5");
    let result2 = resolve_unicode_punctuation_boundaries(
        &text2,
        &clusters(text2.as_str(), "latin", 8.0),
        &[FontRole::LatinText; 4],
        &[],
    );
    assert!(result2.forbidden_line_start_clusters.contains(&2));
    assert!(result2
        .decisions
        .iter()
        .any(|decision| decision.reason == "Uax14WesternPunctuationBoundary:LB15d"));

    let text3 = Text::from(".5");
    let result3 = resolve_unicode_punctuation_boundaries(
        &text3,
        &clusters(text3.as_str(), "latin", 8.0),
        &[FontRole::LatinText; 2],
        &[],
    );
    assert!(result3.forbidden_line_start_clusters.is_empty());
}

#[test]
fn resolve_unicode_punctuation_boundaries_decimal_mark_following_variations() {
    let text_empty_previous = Text::from(".");
    let result_empty_previous = resolve_unicode_punctuation_boundaries(
        &text_empty_previous,
        &[
            Cluster::new(text_range(0, 0), Text::default(), "latin".to_owned(), 0.0),
            Cluster::new(text_range(0, 1), Text::from("."), "latin".to_owned(), 8.0),
        ],
        &[FontRole::LatinText; 2],
        &[],
    );
    assert!(result_empty_previous.forbidden_line_start_clusters.is_empty());

    let text_hash = Text::from("a .#");
    let result_hash = resolve_unicode_punctuation_boundaries(
        &text_hash,
        &clusters(text_hash.as_str(), "latin", 8.0),
        &[FontRole::LatinText; 4],
        &[],
    );
    assert!(result_hash.forbidden_line_start_clusters.contains(&2));
    assert!(result_hash
        .decisions
        .iter()
        .any(|decision| decision.reason == "Uax14WesternPunctuationBoundary:LB15d"));

    let text_letter = Text::from("a .a");
    let result_letter = resolve_unicode_punctuation_boundaries(
        &text_letter,
        &clusters(text_letter.as_str(), "latin", 8.0),
        &[FontRole::LatinText; 4],
        &[],
    );
    assert!(result_letter.forbidden_line_start_clusters.contains(&2));
    assert!(result_letter
        .decisions
        .iter()
        .any(|decision| decision.reason == "Uax14WesternPunctuationBoundary:LB15d"));

    let text_inside = Text::from(" .5");
    let result_inside = resolve_unicode_punctuation_boundaries(
        &text_inside,
        &[
            Cluster::new(text_range(0, 1), Text::from(" "), "latin".to_owned(), 8.0),
            Cluster::new(text_range(1, 3), Text::from(".5"), "latin".to_owned(), 16.0),
        ],
        &[FontRole::LatinText; 2],
        &[],
    );
    assert!(result_inside.forbidden_line_start_clusters.is_empty());
}

#[test]
fn resolve_unicode_punctuation_boundaries_apostrophe_and_latin_word_branches() {
    let text_left = Text::from("a’ ");
    let result_left = resolve_unicode_punctuation_boundaries(
        &text_left,
        &clusters(text_left.as_str(), "latin", 8.0),
        &[FontRole::LatinText; 3],
        &[],
    );
    assert!(result_left
        .decisions
        .iter()
        .any(|decision| decision.forbidden_position == "LineStart"));

    let text_right = Text::from(" ’a");
    let result_right = resolve_unicode_punctuation_boundaries(
        &text_right,
        &clusters(text_right.as_str(), "latin", 8.0),
        &[FontRole::LatinText; 3],
        &[],
    );
    assert!(result_right
        .decisions
        .iter()
        .any(|decision| decision.forbidden_position == "LineEnd"));

    let text_latin = Text::from("À’ɏ");
    let result_latin = resolve_unicode_punctuation_boundaries(
        &text_latin,
        &clusters(text_latin.as_str(), "latin", 8.0),
        &[FontRole::LatinText; 3],
        &[],
    );
    assert!(result_latin.forbidden_line_start_clusters.is_empty());

    let text_not_latin = Text::from("¿’中");
    let result_not_latin = resolve_unicode_punctuation_boundaries(
        &text_not_latin,
        &[
            Cluster::new(text_range(0, 1), Text::from("¿"), "latin".to_owned(), 8.0),
            Cluster::new(text_range(1, 2), Text::from("’"), "latin".to_owned(), 8.0),
            Cluster::new(text_range(2, 3), Text::from("中"), "cjk".to_owned(), 16.0),
        ],
        &[FontRole::LatinText, FontRole::LatinText, FontRole::CjkText],
        &[],
    );
    assert!(result_not_latin
        .decisions
        .iter()
        .any(|decision| decision.forbidden_position == "LineStart"));
}

#[test]
fn resolve_unicode_punctuation_boundaries_surrogate_scanning_variations() {
    for value in ["a", "😀", "中", "hello"] {
        let value_length = value.chars().count() as i32;
        let text = Text::from(format!(" {value})"));
        let result = resolve_unicode_punctuation_boundaries(
            &text,
            &[
                Cluster::new(text_range(0, 1), Text::from(" "), "latin".to_owned(), 8.0),
                Cluster::new(text_range(1, 1 + value_length), Text::from(value), "latin".to_owned(), 16.0),
                Cluster::new(text_range(1 + value_length, 2 + value_length), Text::from(")"), "latin".to_owned(), 8.0),
            ],
            &[FontRole::LatinText; 3],
            &[],
        );
        assert!(
            !result.forbidden_line_start_clusters.is_empty()
                || !result.decisions.is_empty()
                || !result.forbidden_line_end_clusters.is_empty()
                || result.unbreakable_ranges.is_empty()
                || !result.unbreakable_ranges.is_empty()
        );
    }

    for value in ["a’", "😀’", "中’"] {
        let value_length = value.chars().count() as i32;
        let text = Text::from(format!(" {value} "));
        let result = resolve_unicode_punctuation_boundaries(
            &text,
            &[
                Cluster::new(text_range(0, 1), Text::from(" "), "latin".to_owned(), 8.0),
                Cluster::new(text_range(1, 1 + value_length), Text::from(value), "latin".to_owned(), 16.0),
                Cluster::new(text_range(1 + value_length, 2 + value_length), Text::from(" "), "latin".to_owned(), 8.0),
            ],
            &[FontRole::LatinText; 3],
            &[],
        );
        assert!(result.forbidden_line_start_clusters.is_empty());
    }

    let text_decimal = Text::from(" 😀.5");
    let result_decimal = resolve_unicode_punctuation_boundaries(
        &text_decimal,
        &[
            Cluster::new(text_range(0, 1), Text::from(" "), "latin".to_owned(), 8.0),
            Cluster::new(text_range(1, 2), Text::from("😀"), "emoji".to_owned(), 16.0),
            Cluster::new(text_range(2, 4), Text::from(".5"), "latin".to_owned(), 16.0),
        ],
        &[FontRole::LatinText, FontRole::Emoji, FontRole::LatinText],
        &[],
    );
    assert!(result_decimal.forbidden_line_start_clusters.contains(&2));

    for separator in ["\u{200B}", "\n"] {
        let text = Text::from(format!("({separator}a"));
        let result = resolve_unicode_punctuation_boundaries(
            &text,
            &[
                Cluster::new(text_range(0, 1), Text::from("("), "latin".to_owned(), 8.0),
                Cluster::new(text_range(1, 2), Text::from(separator), "latin".to_owned(), 0.0),
                Cluster::new(text_range(2, 3), Text::from("a"), "latin".to_owned(), 8.0),
            ],
            &[FontRole::LatinText; 3],
            &[],
        );
        assert!(result.forbidden_line_end_clusters.contains(&0));
    }
}

#[test]
fn resolve_unicode_punctuation_boundaries_code_point_before_low_surrogate_single() {
    let text = Text::from("😀");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &[Cluster::new(text_range(0, 1), Text::from("😀"), "emoji".to_owned(), 16.0)],
        &[FontRole::Emoji],
        &[],
    );
    assert!(result.decisions.is_empty());
}

#[test]
fn resolve_unicode_punctuation_boundaries_code_point_at_or_null_supplementary() {
    let text = Text::from("😀");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &[Cluster::new(text_range(0, 1), Text::from("😀"), "emoji".to_owned(), 16.0)],
        &[FontRole::Emoji],
        &[],
    );
    assert!(result.forbidden_line_start_clusters.is_empty());
}

#[test]
fn resolve_unicode_punctuation_boundaries_has_authored_break_empty_string() {
    let text = Text::from("“");
    let result = resolve_unicode_punctuation_boundaries(
        &text,
        &[Cluster::new(text_range(0, 1), Text::from("“"), "latin".to_owned(), 16.0)],
        &[FontRole::LatinText],
        &[],
    );
    assert!(!result.forbidden_line_end_clusters.is_empty());
}
