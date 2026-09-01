use tiqian::core::geometry::TextRange;
use tiqian::core::int_range::IntRange;
use tiqian::core::layout_model::LayoutDebugInfo;
use tiqian::core::layout_queries::{
    coerce_selection_offset, get_offset_for_position, get_selection_offset_for_position,
    get_selection_word_boundary, get_selection_word_boundary_for_position,
};
use tiqian::core::source_interaction_boundaries::SourceBoundaryBias;
use tiqian::core::text_model::InlineObjectSpan;

use super::layout_queries_test_support::{cluster, line, result};

#[test]
fn selection_snap_prefers_the_closer_inline_object_boundary() {
    let mut content = result("abb", vec![cluster(TextRange::new(0, 3), "abb", 30.0)], vec![line(TextRange::new(0, 3), IntRange::new(0, 0), 15.0, 0.0, 20.0, 30.0)], Vec::new(), Vec::new(), LayoutDebugInfo::default());
    content.input.inline_objects = vec![InlineObjectSpan::with_fixed_boundaries(TextRange::new(1, 3), 8.0, 4.0, 4.0)];
    assert_eq!(1, get_selection_offset_for_position(&content, 15.0, 5.0));
    assert_eq!(3, get_selection_offset_for_position(&content, 21.0, 5.0));
}

#[test]
fn selection_word_boundary_for_position_rejects_degenerate_content() {
    let empty_text = result("", Vec::new(), vec![line(TextRange::new(0, 0), IntRange::new(0, -1), 15.0, 0.0, 20.0, 0.0)], Vec::new(), Vec::new(), LayoutDebugInfo::default());
    assert_eq!(None, get_selection_word_boundary_for_position(&empty_text, 0.0, 0.0));
    let empty_line = result("a", vec![cluster(TextRange::new(0, 1), "a", 10.0)], vec![line(TextRange::new(0, 1), IntRange::new(0, 0), 15.0, 0.0, 20.0, 10.0), line(TextRange::new(1, 1), IntRange::new(1, -1), 35.0, 20.0, 40.0, 0.0)], Vec::new(), Vec::new(), LayoutDebugInfo::default());
    assert_eq!(None, get_selection_word_boundary_for_position(&empty_line, 5.0, 30.0));
    let leading_empty = result("a", vec![cluster(TextRange::new(0, 0), "", 0.0), cluster(TextRange::new(0, 1), "a", 10.0)], vec![line(TextRange::new(0, 1), IntRange::new(0, 1), 15.0, 0.0, 20.0, 10.0)], Vec::new(), Vec::new(), LayoutDebugInfo::default());
    assert_eq!(None, get_selection_word_boundary_for_position(&leading_empty, 0.0, 5.0));
    assert_eq!(Some(TextRange::new(0, 1)), get_selection_word_boundary_for_position(&leading_empty, 5.0, 5.0));
}

#[test]
fn zero_width_clusters_return_their_start_in_hit_tests() {
    let empty_range = result("", vec![cluster(TextRange::new(0, 0), "", 5.0)], vec![line(TextRange::new(0, 0), IntRange::new(0, 0), 15.0, 0.0, 20.0, 5.0)], Vec::new(), Vec::new(), LayoutDebugInfo::default());
    assert_eq!(0, get_offset_for_position(&empty_range, 2.0, 5.0));
    let zero_advance = result("ab", vec![cluster(TextRange::new(0, 1), "a", 10.0), cluster(TextRange::new(1, 2), "b", 0.0)], vec![line(TextRange::new(0, 1), IntRange::new(0, 0), 15.0, 0.0, 20.0, 10.0), line(TextRange::new(1, 2), IntRange::new(1, 1), 35.0, 20.0, 40.0, 0.0)], Vec::new(), Vec::new(), LayoutDebugInfo::default());
    assert_eq!(Some(TextRange::new(0, 2)), get_selection_word_boundary_for_position(&zero_advance, 0.0, 30.0));
}

#[test]
fn coerce_selection_offset_honours_inline_object_boundaries() {
    let mut content = result("abb", Vec::new(), vec![line(TextRange::new(0, 3), IntRange::new(0, 0), 15.0, 0.0, 20.0, 0.0)], Vec::new(), Vec::new(), LayoutDebugInfo::default());
    content.input.inline_objects = vec![InlineObjectSpan::with_fixed_boundaries(TextRange::new(1, 3), 8.0, 4.0, 4.0)];
    assert_eq!(1, coerce_selection_offset(&content, 2, SourceBoundaryBias::Backward));
    assert_eq!(3, coerce_selection_offset(&content, 2, SourceBoundaryBias::Forward));
    assert_eq!(3, coerce_selection_offset(&content, 2, SourceBoundaryBias::Nearest));
    assert_eq!(1, coerce_selection_offset(&content, 1, SourceBoundaryBias::Nearest));
    assert_eq!(3, coerce_selection_offset(&content, 3, SourceBoundaryBias::Nearest));
}

#[test]
fn selection_word_boundary_expands_words_and_honours_inline_objects() {
    let content = result("hello", Vec::new(), vec![line(TextRange::new(0, 5), IntRange::new(0, 0), 15.0, 0.0, 20.0, 0.0)], Vec::new(), Vec::new(), LayoutDebugInfo::default());
    assert_eq!(TextRange::new(0, 5), get_selection_word_boundary(&content, 2));
    assert_eq!(TextRange::new(0, 5), get_selection_word_boundary(&content, 5));
    let emoji = result("😀", Vec::new(), vec![line(TextRange::new(0, 2), IntRange::new(0, 0), 15.0, 0.0, 20.0, 0.0)], Vec::new(), Vec::new(), LayoutDebugInfo::default());
    assert_eq!(TextRange::new(0, 2), get_selection_word_boundary(&emoji, 1));
    let mut object = result("abb", Vec::new(), vec![line(TextRange::new(0, 3), IntRange::new(0, 0), 15.0, 0.0, 20.0, 0.0)], Vec::new(), Vec::new(), LayoutDebugInfo::default());
    object.input.inline_objects = vec![InlineObjectSpan::with_fixed_boundaries(TextRange::new(1, 3), 8.0, 4.0, 4.0)];
    assert_eq!(TextRange::new(1, 3), get_selection_word_boundary(&object, 2));
    let mandatory = result("a\nb", Vec::new(), vec![line(TextRange::new(0, 3), IntRange::new(0, 0), 15.0, 0.0, 20.0, 0.0)], Vec::new(), Vec::new(), LayoutDebugInfo::default());
    assert_eq!(TextRange::new(1, 2), get_selection_word_boundary(&mandatory, 1));
    let connectors = result("a_b", Vec::new(), vec![line(TextRange::new(0, 3), IntRange::new(0, 0), 15.0, 0.0, 20.0, 0.0)], Vec::new(), Vec::new(), LayoutDebugInfo::default());
    assert_eq!(TextRange::new(0, 3), get_selection_word_boundary(&connectors, 1));
}

#[test]
fn selection_word_kind_covers_every_han_block() {
    for text in ["㐀", "一", "豈", "𠀀"] {
        let length = text.encode_utf16().count() as i32;
        let content = result(text, Vec::new(), vec![line(TextRange::new(0, length), IntRange::new(0, 0), 15.0, 0.0, 20.0, 0.0)], Vec::new(), Vec::new(), LayoutDebugInfo::default());
        assert_eq!(TextRange::new(0, length), get_selection_word_boundary(&content, 0), "text={text}");
    }
}

#[test]
fn selection_word_boundary_skips_inline_objects_it_does_not_contain() {
    let mut content = result("abcdefg", vec![cluster(TextRange::new(0, 7), "abcdefg", 70.0)], vec![line(TextRange::new(0, 7), IntRange::new(0, 0), 15.0, 0.0, 20.0, 70.0)], Vec::new(), Vec::new(), LayoutDebugInfo::default());
    content.input.inline_objects = vec![InlineObjectSpan::with_fixed_boundaries(TextRange::new(1, 3), 8.0, 4.0, 4.0), InlineObjectSpan::with_fixed_boundaries(TextRange::new(5, 7), 8.0, 4.0, 4.0)];
    assert_eq!(TextRange::new(0, 7), get_selection_word_boundary(&content, 4));
    assert_eq!(TextRange::new(1, 3), get_selection_word_boundary(&content, 2));
}

#[test]
fn selection_word_boundary_for_position_covers_distances_and_fallbacks() {
    let content = result("甲乙", vec![cluster(TextRange::new(0, 1), "甲", 10.0), cluster(TextRange::new(1, 2), "乙", 10.0)], vec![line(TextRange::new(0, 2), IntRange::new(0, 1), 15.0, 0.0, 20.0, 20.0)], Vec::new(), Vec::new(), LayoutDebugInfo::default());
    assert_eq!(Some(TextRange::new(0, 1)), get_selection_word_boundary_for_position(&content, 5.0, 10.0));
    assert_eq!(Some(TextRange::new(0, 1)), get_selection_word_boundary_for_position(&content, 5.0, -10.0));
    assert_eq!(Some(TextRange::new(0, 1)), get_selection_word_boundary_for_position(&content, 5.0, 60.0));
    assert_eq!(Some(TextRange::new(0, 1)), get_selection_word_boundary_for_position(&content, -50.0, 10.0));
    assert_eq!(Some(TextRange::new(1, 2)), get_selection_word_boundary_for_position(&content, 500.0, 10.0));
}

#[test]
fn compatibility_ideographs_form_individual_word_units() {
    let content = result("𠀀豈", vec![cluster(TextRange::new(0, 2), "𠀀", 10.0), cluster(TextRange::new(2, 3), "豈", 10.0)], vec![line(TextRange::new(0, 3), IntRange::new(0, 1), 15.0, 0.0, 20.0, 20.0)], Vec::new(), Vec::new(), LayoutDebugInfo::default());
    assert_eq!(TextRange::new(0, 2), get_selection_word_boundary(&content, 0));
    assert_eq!(TextRange::new(2, 3), get_selection_word_boundary(&content, 2));
}

#[test]
fn word_boundary_for_position_handles_a_non_finite_y() {
    let content = result("甲乙", vec![cluster(TextRange::new(0, 1), "甲", 10.0), cluster(TextRange::new(1, 2), "乙", 10.0)], vec![line(TextRange::new(0, 2), IntRange::new(0, 1), 15.0, 0.0, 20.0, 20.0)], Vec::new(), Vec::new(), LayoutDebugInfo::default());
    assert_eq!(Some(TextRange::new(0, 1)), get_selection_word_boundary_for_position(&content, 5.0, f32::NAN));
}

#[test]
fn supplementary_ideograph_beyond_the_han_ranges_is_its_own_unit() {
    let text = "𰀀";
    let content = result(text, vec![cluster(TextRange::new(0, 2), text, 10.0)], vec![line(TextRange::new(0, 2), IntRange::new(0, 0), 15.0, 0.0, 20.0, 10.0)], Vec::new(), Vec::new(), LayoutDebugInfo::default());
    assert_eq!(TextRange::new(0, 2), get_selection_word_boundary(&content, 0));
}

#[test]
fn plane_four_codepoint_above_the_han_bands_is_its_own_unit() {
    let text = "񀀀";
    let content = result(text, vec![cluster(TextRange::new(0, 2), text, 10.0)], vec![line(TextRange::new(0, 2), IntRange::new(0, 0), 15.0, 0.0, 20.0, 10.0)], Vec::new(), Vec::new(), LayoutDebugInfo::default());
    assert_eq!(TextRange::new(0, 2), get_selection_word_boundary(&content, 0));
}

#[test]
fn selection_word_boundary_for_position_prefers_the_closer_later_line() {
    let content = result("甲乙丙丁", vec![cluster(TextRange::new(0, 1), "甲", 10.0), cluster(TextRange::new(1, 2), "乙", 10.0), cluster(TextRange::new(2, 3), "丙", 10.0), cluster(TextRange::new(3, 4), "丁", 10.0)], vec![line(TextRange::new(0, 2), IntRange::new(0, 1), 15.0, 0.0, 20.0, 20.0), line(TextRange::new(2, 4), IntRange::new(2, 3), 55.0, 40.0, 60.0, 20.0)], Vec::new(), Vec::new(), LayoutDebugInfo::default());
    assert_eq!(Some(TextRange::new(2, 3)), get_selection_word_boundary_for_position(&content, 5.0, 50.0));
    assert_eq!(Some(TextRange::new(0, 1)), get_selection_word_boundary_for_position(&content, 5.0, 30.0));
    assert_eq!(Some(TextRange::new(0, 1)), get_selection_word_boundary_for_position(&content, 5.0, -10.0));
    assert_eq!(Some(TextRange::new(0, 1)), get_selection_word_boundary_for_position(&content, 5.0, 10.0));
    assert_eq!(Some(TextRange::new(2, 3)), get_selection_word_boundary_for_position(&content, 5.0, 100.0));
}