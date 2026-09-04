use tiqian::clreq::clreq_profile::ClreqProfile;
use tiqian::common::{HashMap, HashSet};
use tiqian::core::geometry::{scalar_offset, text_range, TextRange};
use tiqian::core::layout_model::Cluster;
use tiqian::core::text::Text;
use tiqian::core::text_model::InlineObjectSpan;
use tiqian::font::font_policy::{
    CjkFontRoleClassifier, FontCandidate, FontDecision, FontRole, FontRoleContext,
};
use tiqian::layout::cluster_role_resolution::{
    ClusterRoleRangeOptions, cluster_role_ranges, cluster_role_ranges_with_options,
    require_covered_by,
};

fn role_ranges(text: &str) -> Vec<tiqian::layout::cluster_role_resolution::ResolvedClusterRange> {
    cluster_role_ranges(
        &Text::from(text),
        &CjkFontRoleClassifier,
        &FontRoleContext::default(),
        &ClreqProfile::mainland_horizontal(),
    )
}

#[test]
fn cluster_role_ranges_with_simple_text() {
    assert!(!role_ranges("中文").is_empty());
}

#[test]
fn cluster_role_ranges_with_emoji() {
    assert!(!role_ranges("😀").is_empty());
}

#[test]
fn cluster_role_ranges_with_crlf_mandatory_break() {
    assert!(role_ranges("line1\r\nline2")
        .iter()
        .any(|range| range.mandatory_break));
}

#[test]
fn cluster_role_ranges_with_lf_only() {
    assert!(role_ranges("line1\nline2")
        .iter()
        .any(|range| range.mandatory_break));
}

#[test]
fn cluster_role_ranges_with_zero_width_space() {
    assert!(role_ranges("ab\u{200B}cd")
        .iter()
        .any(|range| range.zero_width_soft_break));
}

#[test]
fn cluster_role_ranges_with_inline_object() {
    let text = Text::from("abxcd");
    let options = ClusterRoleRangeOptions::builder()
        .inline_objects_by_start(HashMap::from([(
            scalar_offset(2),
            InlineObjectSpan::with_fixed_boundaries(text_range(2, 3), 8.0, 8.0, 8.0),
        )]))
        .build();
    let ranges = cluster_role_ranges_with_options(
        &text,
        &CjkFontRoleClassifier,
        &FontRoleContext::default(),
        &ClreqProfile::mainland_horizontal(),
        &options,
    );
    assert_eq!(1, ranges.len());
}

fn latin_cluster(range: TextRange, text: &str) -> Cluster {
    Cluster::with_display_text(
        range,
        Text::from(text),
        Text::from(text),
        "latin".to_owned(),
        8.0,
    )
}

fn latin_decision(range: TextRange) -> FontDecision {
    FontDecision {
        range,
        candidate: FontCandidate {
            key: "test".to_owned(),
            family: "test".to_owned(),
            role: FontRole::LatinText,
        },
        role: FontRole::LatinText,
        reason: "test".to_owned(),
    }
}

#[test]
fn require_covered_by_with_contiguous_clusters() {
    require_covered_by(
        &[
            latin_cluster(text_range(0, 2), "ab"),
            latin_cluster(text_range(2, 4), "cd"),
        ],
        &[latin_decision(text_range(0, 2)), latin_decision(text_range(2, 4))],
    );
}

#[test]
fn require_covered_by_with_single_cluster() {
    require_covered_by(
        &[
            latin_cluster(text_range(0, 1), "a"),
            latin_cluster(text_range(1, 2), "b"),
        ],
        &[latin_decision(text_range(0, 2))],
    );
}

#[test]
fn require_covered_by_with_multiple_decisions() {
    require_covered_by(
        &[
            latin_cluster(text_range(0, 1), "a"),
            latin_cluster(text_range(1, 2), "b"),
            latin_cluster(text_range(2, 3), "c"),
        ],
        &[
            latin_decision(text_range(0, 1)),
            latin_decision(text_range(1, 2)),
            latin_decision(text_range(2, 3)),
        ],
    );
}

#[test]
#[should_panic(expected = "crossing")]
fn require_covered_by_fails_when_cluster_crosses_decision_range() {
    require_covered_by(
        &[latin_cluster(text_range(0, 3), "abc")],
        &[latin_decision(text_range(0, 2))],
    );
}

#[test]
#[should_panic(expected = "non-contiguous")]
fn require_covered_by_fails_when_clusters_are_non_contiguous() {
    require_covered_by(
        &[
            latin_cluster(text_range(0, 1), "a"),
            latin_cluster(text_range(2, 3), "c"),
        ],
        &[latin_decision(text_range(0, 3))],
    );
}

#[test]
#[should_panic(expected = "must return clusters covering")]
fn require_covered_by_fails_when_clusters_do_not_cover_end() {
    require_covered_by(
        &[latin_cluster(text_range(0, 1), "a")],
        &[latin_decision(text_range(0, 3))],
    );
}

#[test]
#[should_panic(expected = "must return clusters covering")]
fn require_covered_by_with_gap_between_decisions() {
    require_covered_by(
        &[
            latin_cluster(text_range(0, 1), "a"),
            latin_cluster(text_range(1, 2), "b"),
        ],
        &[
            latin_decision(text_range(0, 1)),
            latin_decision(text_range(1, 2)),
            latin_decision(text_range(2, 3)),
        ],
    );
}

#[test]
fn require_covered_by_with_empty_decisions() {
    require_covered_by(&[latin_cluster(text_range(0, 1), "a")], &[]);
}

#[test]
#[should_panic(expected = "crossing")]
fn require_covered_by_with_overlapping_decisions() {
    require_covered_by(
        &[
            latin_cluster(text_range(0, 2), "ab"),
            latin_cluster(text_range(2, 4), "cd"),
        ],
        &[latin_decision(text_range(0, 3))],
    );
}

#[test]
fn cluster_role_ranges_with_grapheme_extend() { assert!(!role_ranges("a\u{0300}").is_empty()); }

#[test]
fn cluster_role_ranges_with_variation_selector() { assert!(!role_ranges("A\u{FE0F}").is_empty()); }

#[test]
fn cluster_role_ranges_with_keycap_sequence() { assert!(!role_ranges("1\u{20E3}").is_empty()); }

#[test]
fn cluster_role_ranges_with_emoji_modifier_sequence() { assert!(!role_ranges("🏻").is_empty()); }

#[test]
fn cluster_role_ranges_with_emoji_style_variation() { assert!(!role_ranges("☀️").is_empty()); }

#[test]
fn cluster_role_ranges_with_empty_text() { assert_eq!(0, role_ranges("").len()); }

#[test]
fn cluster_role_ranges_with_only_whitespace() { assert!(!role_ranges("  ").is_empty()); }

#[test]
fn cluster_role_ranges_modifier_base_with_variation_selector_and_modifier() {
    let ranges = role_ranges("✊️🏻");
    assert!(!ranges.is_empty());
    assert_eq!(FontRole::Emoji, ranges[0].role);
}

#[test]
fn cluster_role_ranges_with_crlf_only() { assert!(role_ranges("\r\n").iter().any(|range| range.mandatory_break)); }

#[test]
fn cluster_role_ranges_with_lf_inside_crlf() { assert!(role_ranges("a\r\nb").iter().any(|range| range.mandatory_break)); }

#[test]
fn cluster_role_ranges_with_cr_only() { assert!(role_ranges("a\rb").iter().any(|range| range.mandatory_break)); }

#[test]
fn cluster_role_ranges_with_emoji_shaping_boundary_inside() {
    let text = Text::from("#\u{FE0F}A");
    let options = ClusterRoleRangeOptions::builder().emoji_shaping_boundaries(HashSet::from([scalar_offset(2)])).build();
    assert!(!cluster_role_ranges_with_options(&text, &CjkFontRoleClassifier, &FontRoleContext::default(), &ClreqProfile::mainland_horizontal(), &options).is_empty());
}

#[test]
fn cluster_role_ranges_with_grapheme_extend_after_emoji() { assert!(!role_ranges("😀\u{0300}").is_empty()); }

#[test]
fn cluster_role_ranges_with_variation_selector_after_latin() { assert!(!role_ranges("A\u{FE0F}").is_empty()); }

#[test]
fn cluster_role_ranges_with_cjk_punctuation_coalesce() { assert!(!role_ranges("、、、").is_empty()); }

#[test]
fn cluster_role_ranges_with_role_override() { assert!(role_ranges("☀️").iter().any(|range| range.role == FontRole::Emoji)); }

#[test]
fn cluster_role_ranges_with_ascii_point_mark_attached() { assert!(!role_ranges(",abc").is_empty()); }

#[test]
fn cluster_role_ranges_with_emoji_modifier_base_combining_mark() { assert!(!role_ranges("✋\u{0300}🏻").is_empty()); }

#[test]
fn cluster_role_ranges_with_keycap_base_and_keycap() {
    let ranges = role_ranges("1\u{FE0F}\u{20E3}");
    assert!(!ranges.is_empty());
    assert_eq!(FontRole::Emoji, ranges[0].role);
}

#[test]
fn cluster_role_ranges_with_emoji_style_variation_no_fe0f() { assert!(!role_ranges("☀").is_empty()); }

#[test]
fn cluster_role_ranges_with_multiple_span_boundaries() {
    let text = Text::from("abcdef");
    let options = ClusterRoleRangeOptions::builder().span_boundaries(HashSet::from([scalar_offset(2), scalar_offset(4)])).build();
    assert!(!cluster_role_ranges_with_options(&text, &CjkFontRoleClassifier, &FontRoleContext::default(), &ClreqProfile::mainland_horizontal(), &options).is_empty());
}

#[test]
fn cluster_role_ranges_with_non_cjk_punctuation() { assert!(!role_ranges(".,;").is_empty()); }

#[test]
fn cluster_role_ranges_with_emoji_variation_and_modifier() {
    let ranges = role_ranges("✊️🏻");
    assert!(!ranges.is_empty());
    assert_eq!(FontRole::Emoji, ranges[0].role);
}

#[test]
fn cluster_role_ranges_with_zwj_sequence() { assert!(!role_ranges("😀\u{200D}🚻").is_empty()); }

#[test]
fn cluster_role_ranges_with_lf_at_start() { assert!(role_ranges("\nabc").iter().any(|range| range.mandatory_break)); }

#[test]
fn cluster_role_ranges_with_cr_not_followed_by_lf() { assert!(role_ranges("a\rb").iter().any(|range| range.mandatory_break)); }

#[test]
fn cluster_role_ranges_with_cr_at_end() { assert!(role_ranges("a\r").iter().any(|range| range.mandatory_break)); }

#[test]
fn cluster_role_ranges_with_single_grapheme() { assert_eq!(1, role_ranges("a").len()); }

#[test]
fn cluster_role_ranges_with_emoji_shaping_boundary_at_grapheme_end() {
    let text = Text::from("#\u{FE0F}");
    let options = ClusterRoleRangeOptions::builder().emoji_shaping_boundaries(HashSet::from([scalar_offset(2)])).build();
    assert!(!cluster_role_ranges_with_options(&text, &CjkFontRoleClassifier, &FontRoleContext::default(), &ClreqProfile::mainland_horizontal(), &options).is_empty());
}

#[test]
fn cluster_role_ranges_with_attached_ascii_point_mark_at_start() { assert!(!role_ranges(",a").is_empty()); }

#[test]
fn cluster_role_ranges_with_attached_ascii_point_mark_followed_by_latin() { assert!(!role_ranges(",ab").is_empty()); }

#[test]
fn cluster_role_ranges_with_supplementary_character() { assert!(!role_ranges("😀").is_empty()); }

#[test]
fn cluster_role_ranges_with_cjk_punctuation_and_coalesce() { assert!(!role_ranges("、。、").is_empty()); }

#[test]
fn cluster_role_ranges_with_multiple_emoji_shaping_boundaries() {
    let text = Text::from("#\u{FE0F}*\u{FE0F}");
    let options = ClusterRoleRangeOptions::builder().emoji_shaping_boundaries(HashSet::from([scalar_offset(2), scalar_offset(4)])).build();
    assert!(!cluster_role_ranges_with_options(&text, &CjkFontRoleClassifier, &FontRoleContext::default(), &ClreqProfile::mainland_horizontal(), &options).is_empty());
}

#[test]
fn cluster_role_ranges_with_variation_selector_after_emoji() { assert!(!role_ranges("😀\u{FE0F}").is_empty()); }

#[test]
fn cluster_role_ranges_with_keycap_base_no_keycap() { assert!(!role_ranges("1\u{FE0F}").is_empty()); }

#[test]
fn cluster_role_ranges_with_crlf_pair_produces_single_cluster() {
    let range = role_ranges("a\r\nb").into_iter().find(|range| range.range == text_range(1, 3)).unwrap();
    assert!(range.mandatory_break);
}

#[test]
fn cluster_role_ranges_with_emoji_role_promotion_null() { assert!(!role_ranges("😀").is_empty()); }

#[test]
fn cluster_role_ranges_with_non_variation_selector() { assert!(!role_ranges("AB").is_empty()); }

#[test]
fn cluster_role_ranges_with_non_combining_mark() { assert!(!role_ranges("A\u{0300}B").is_empty()); }

#[test]
fn cluster_role_ranges_with_non_ascii_point_mark() { assert!(!role_ranges("A!B").is_empty()); }

#[test]
fn cluster_role_ranges_with_emoji_shaping_boundary_inside_and_outside_range() {
    let text = Text::from("😀\u{FE0F}😁");
    let options = ClusterRoleRangeOptions::builder().emoji_shaping_boundaries(HashSet::from([scalar_offset(4)])).build();
    assert!(!cluster_role_ranges_with_options(&text, &CjkFontRoleClassifier, &FontRoleContext::default(), &ClreqProfile::mainland_horizontal(), &options).is_empty());
}

#[test]
fn cluster_role_ranges_with_attached_ascii_point_mark_not_adjacent() { assert!(!role_ranges("a.!b").is_empty()); }

#[test]
fn astral_variation_selector_extends_the_run_before_it() {
    let ranges = role_ranges("中\u{E0100}中");
    assert_eq!(2, ranges.len());
    assert_eq!(text_range(0, 2), ranges[0].range);
    assert_eq!(FontRole::CjkText, ranges[0].role);
    assert_eq!(text_range(2, 3), ranges[1].range);
}

#[test]
fn astral_variation_selector_after_an_attached_point_mark_ends_the_run() {
    let ranges = role_ranges("中,\u{E0100}中");
    assert_eq!(3, ranges.len());
    assert_eq!(text_range(1, 3), ranges[1].range);
    assert_eq!(FontRole::LatinText, ranges[1].role);
}

#[test]
fn astral_variation_selector_between_base_and_modifier_keeps_the_sequence() {
    let ranges = role_ranges("✊\u{E0100}🏻");
    assert_eq!(1, ranges.len());
    assert_eq!(text_range(0, 3), ranges[0].range);
    assert_eq!(FontRole::Emoji, ranges[0].role);
}

#[test]
fn code_point_above_the_supplementary_selector_range_stands_alone() {
    let ranges = role_ranges("中\u{E01F0}中");
    assert_eq!(3, ranges.len());
    assert_eq!(text_range(0, 1), ranges[0].range);
    assert_eq!(text_range(1, 2), ranges[1].range);
    assert_eq!(text_range(2, 3), ranges[2].range);
}

#[test]
fn modifier_base_with_a_bmp_selector_walks_the_selector_true_arm() {
    let ranges = role_ranges("✊\u{FE0F}🏻");
    assert_eq!(1, ranges.len());
    assert_eq!(text_range(0, 3), ranges[0].range);
    assert_eq!(FontRole::Emoji, ranges[0].role);
}

#[test]
fn modifier_base_with_only_a_selector_ends_the_walk_at_the_cluster_end() {
    let ranges = role_ranges("✊\u{E0100}");
    assert_eq!(1, ranges.len());
    assert_eq!(text_range(0, 2), ranges[0].range);
}

#[test]
fn zwj_member_inside_a_modifier_base_cluster_breaks_the_walk_below_the_range() {
    let ranges = role_ranges("✊\u{200D}♀️");
    assert_eq!(1, ranges.len());
    assert_eq!(text_range(0, 4), ranges[0].range);
}

#[test]
fn span_boundary_after_a_space_let_the_point_mark_see_its_whitespace_neighbour() {
    let text = Text::from("a ,");
    let options = ClusterRoleRangeOptions::builder().span_boundaries(HashSet::from([scalar_offset(2)])).build();
    let ranges = cluster_role_ranges_with_options(&text, &CjkFontRoleClassifier, &FontRoleContext::default(), &ClreqProfile::mainland_horizontal(), &options);
    assert_eq!(2, ranges.len());
    assert_eq!(text_range(0, 2), ranges[0].range);
    assert_eq!(text_range(2, 3), ranges[1].range);
}

#[test]
fn inline_object_over_the_cr_walks_the_lf_with_a_cr_behind_it() {
    let text = Text::from("\r\n");
    let options = ClusterRoleRangeOptions::builder().inline_objects_by_start(HashMap::from([(
        scalar_offset(0),
        InlineObjectSpan::with_fixed_boundaries(text_range(0, 1), 8.0, 8.0, 8.0),
    )])).build();
    let ranges = cluster_role_ranges_with_options(&text, &CjkFontRoleClassifier, &FontRoleContext::default(), &ClreqProfile::mainland_horizontal(), &options);
    assert_eq!(2, ranges.len());
    assert_eq!(text_range(0, 1), ranges[0].range);
    assert_eq!(text_range(1, 2), ranges[1].range);
    assert!(!ranges[1].mandatory_break);
    assert!(ranges.iter().all(|range| !range.mandatory_break));
}

#[test]
fn cluster_role_ranges_with_span_boundaries() {
    let text = Text::from("abcd");
    let options = ClusterRoleRangeOptions::builder()
        .span_boundaries(HashSet::from([scalar_offset(2)]))
        .build();
    let ranges = cluster_role_ranges_with_options(
        &text,
        &CjkFontRoleClassifier,
        &FontRoleContext::default(),
        &ClreqProfile::mainland_horizontal(),
        &options,
    );
    assert!(!ranges.is_empty());
}

#[test]
fn cluster_role_ranges_with_emoji_shaping_boundaries() {
    let text = Text::from("#\u{FE0F}");
    let options = ClusterRoleRangeOptions::builder()
        .emoji_shaping_boundaries(HashSet::from([scalar_offset(1)]))
        .build();
    let ranges = cluster_role_ranges_with_options(
        &text,
        &CjkFontRoleClassifier,
        &FontRoleContext::default(),
        &ClreqProfile::mainland_horizontal(),
        &options,
    );
    assert!(!ranges.is_empty());
}

#[test]
fn cluster_role_ranges_with_ascii_point_mark() {
    assert!(!role_ranges("a,b").is_empty());
}

#[test]
fn cluster_role_ranges_with_coalesce_repeatable_punctuation() {
    assert!(!role_ranges("，，").is_empty());
}