use std::collections::HashMap;

use tiqian::org::tiqian::linebreak::Hyphenation::{
    Hyphenator, LiangHyphenator, NoHyphenator, parse_tex_hyphenation_patterns,
};

#[test]
fn no_hyphenator_yields_no_opportunities() {
    assert!(NoHyphenator.hyphenate("international").is_empty());
}

#[test]
fn odd_level_gap_becomes_a_break_outside_the_margins() {
    let hyphenator = LiangHyphenator::with_margins(
        HashMap::from([("c".to_owned(), vec![1, 0])]),
        1,
        1,
    );
    assert_eq!(vec![2], hyphenator.hyphenate("abc"));
    assert!(hyphenator.hyphenate("cab").is_empty());
}

#[test]
fn max_level_wins_and_even_forbids_the_break() {
    let hyphenator = LiangHyphenator::with_margins(
        HashMap::from([
            ("ab".to_owned(), vec![0, 1, 0]),
            ("zab".to_owned(), vec![0, 0, 2, 0]),
        ]),
        1,
        1,
    );
    assert_eq!(vec![1], hyphenator.hyphenate("ab"));
    assert!(hyphenator.hyphenate("zab").is_empty());
}

#[test]
fn margins_and_short_words_are_respected() {
    let hyphenator = LiangHyphenator::with_margins(
        HashMap::from([("a".to_owned(), vec![1, 0])]),
        2,
        3,
    );
    assert!(hyphenator.hyphenate("the").is_empty());
}

#[test]
fn exceptions_override_patterns_and_are_case_insensitive() {
    let hyphenator = LiangHyphenator::with_options(
        HashMap::new(),
        HashMap::from([("table".to_owned(), vec![2])]),
        1,
        1,
    );
    assert_eq!(vec![2], hyphenator.hyphenate("table"));
    assert_eq!(vec![2], hyphenator.hyphenate("Table"));
}

#[test]
fn parses_patterns_and_exception_blocks_stripping_comments() {
    let (patterns, exceptions) = parse_tex_hyphenation_patterns(
        r#"
        % a comment line
        \patterns{ % inline comment
        .ach4
        a5bal
        }
        \hyphenation{
        ta-ble
        present
        }
        "#,
    );
    assert_eq!(Some(&vec![0, 0, 0, 0, 4]), patterns.get(".ach"));
    assert_eq!(Some(&vec![0, 5, 0, 0, 0]), patterns.get("abal"));
    assert_eq!(Some(&vec![2]), exceptions.get("table"));
    assert_eq!(Some(&Vec::new()), exceptions.get("present"));
}
