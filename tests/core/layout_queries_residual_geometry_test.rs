use tiqian::core::geometry::{scalar_offset, text_range, Rect};
use tiqian::core::int_range::IntRange;
use tiqian::core::layout_model::{
    ClusterGeometryDecisionInfo, Glyph, GlyphRun, LayoutDebugInfo, RubyDecisionInfo,
};
use tiqian::core::layout_queries::{
    get_bounding_box, get_bounding_boxes, get_cursor_rect, get_line_for_offset,
    get_offset_for_position, get_selection_offset_for_position, glyph_ink_bounds,
    positioned_clusters, positioned_clusters_for_line_box, positioned_rich_text_segments,
};
use tiqian::core::text::Text;
use tiqian::core::text_model::{RichTextRole, RichTextSpan};

use super::layout_queries_test_support::{cluster, line, result};

#[test]
fn positioned_clusters_by_line_rejects_foreign_lines() {
    let owned = line(text_range(0, 2), IntRange::new(0, 0), 15.0, 0.0, 20.0, 20.0);
    let foreign = line(text_range(0, 2), IntRange::new(0, 0), 15.0, 99.0, 119.0, 20.0);
    let content = result("ab", vec![cluster(text_range(0, 2), "ab", 20.0)], vec![owned.clone()], Vec::new(), Vec::new(), LayoutDebugInfo::default());
    assert_eq!(1, positioned_clusters_for_line_box(&content, &owned).len());
    assert!(std::panic::catch_unwind(|| positioned_clusters_for_line_box(&content, &foreign)).is_err());
}

#[test]
fn glyph_ink_bounds_skips_unmatched_glyphs_and_returns_null_without_ink() {
    let content = result("ab", vec![cluster(text_range(0, 1), "a", 10.0), cluster(text_range(1, 2), "b", 10.0)], vec![line(text_range(0, 2), IntRange::new(0, 1), 15.0, 0.0, 20.0, 20.0)], vec![GlyphRun::new(text_range(0, 2), "test".to_owned(), vec![Glyph::builder(1, text_range(0, 1), 10.0).bounds(Some(Rect { left: 0.0, top: 2.0, right: 8.0, bottom: 12.0 })).build(), Glyph::builder(2, text_range(0, 1), 10.0).build(), Glyph::builder(3, text_range(5, 6), 10.0).bounds(Some(Rect { left: 0.0, top: 0.0, right: 1.0, bottom: 1.0 })).build()], 20.0)], Vec::new(), LayoutDebugInfo::default());
    assert_eq!(Some(Rect { left: 0.0, top: 17.0, right: 8.0, bottom: 27.0 }), glyph_ink_bounds(&content));
    let no_ink = result("ab", vec![cluster(text_range(0, 1), "a", 10.0), cluster(text_range(1, 2), "b", 10.0)], vec![line(text_range(0, 2), IntRange::new(0, 1), 15.0, 0.0, 20.0, 20.0)], Vec::new(), Vec::new(), LayoutDebugInfo::default());
    assert_eq!(None, glyph_ink_bounds(&no_ink));
}

#[test]
fn empty_line_results_short_circuit_every_query() {
    let content = result("ab", Vec::new(), Vec::new(), Vec::new(), Vec::new(), LayoutDebugInfo::default());
    assert_eq!(-1, get_line_for_offset(&content, scalar_offset(0)));
    assert_eq!(Rect { left: 0.0, top: 0.0, right: 0.0, bottom: 0.0 }, get_bounding_box(&content, scalar_offset(0)));
    assert_eq!(Rect { left: 0.0, top: 0.0, right: 0.0, bottom: 0.0 }, get_cursor_rect(&content, scalar_offset(0)));
    assert_eq!(scalar_offset(0), get_offset_for_position(&content, 5.0, 5.0));
    assert_eq!(scalar_offset(0), get_selection_offset_for_position(&content, 5.0, 5.0));
    assert!(get_bounding_boxes(&content, text_range(0, 2)).is_empty());
    assert!(positioned_rich_text_segments(&content, &[RichTextSpan::new(text_range(0, 1), RichTextRole::Underline)]).is_empty());
}

#[test]
fn bounding_box_falls_back_to_the_cursor_rect_at_cluster_gaps() {
    let content = result("abc", vec![cluster(text_range(0, 1), "a", 10.0), cluster(text_range(2, 3), "c", 10.0)], vec![line(text_range(0, 3), IntRange::new(0, 1), 15.0, 0.0, 20.0, 20.0)], Vec::new(), Vec::new(), LayoutDebugInfo::default());
    assert_eq!(Rect { left: 10.0, top: 0.0, right: 11.0, bottom: 20.0 }, get_bounding_box(&content, scalar_offset(1)));
    assert_eq!(Rect { left: 20.0, top: 0.0, right: 21.0, bottom: 20.0 }, get_bounding_box(&content, scalar_offset(3)));
    assert!(get_bounding_boxes(&content, text_range(3, 5)).is_empty());
}

#[test]
fn cursor_rect_covers_empty_lines_empty_clusters_and_multi_unit_clusters() {
    let empty = result("a", Vec::new(), vec![line(text_range(0, 0), IntRange::new(0, -1), 15.0, 0.0, 20.0, 0.0).clone()], Vec::new(), Vec::new(), LayoutDebugInfo::default());
    let mut empty = empty;
    empty.lines[0].indent = 6.0;
    assert_eq!(6.0, get_cursor_rect(&empty, scalar_offset(0)).left);
    let linear = result("ab", vec![cluster(text_range(0, 2), "ab", 20.0)], vec![line(text_range(0, 2), IntRange::new(0, 0), 15.0, 0.0, 20.0, 20.0)], Vec::new(), Vec::new(), LayoutDebugInfo::default());
    assert_eq!(10.0, get_cursor_rect(&linear, scalar_offset(1)).left);
    let stops = result("ab", vec![cluster(text_range(0, 2), "ab", 20.0)], vec![line(text_range(0, 2), IntRange::new(0, 0), 15.0, 0.0, 20.0, 20.0)], vec![GlyphRun::new(text_range(0, 2), "test".to_owned(), vec![Glyph::builder(1, text_range(0, 2), 10.0).x(0.0).build(), Glyph::builder(2, text_range(0, 2), 10.0).x(12.0).build()], 20.0)], Vec::new(), LayoutDebugInfo::default());
    assert_eq!(12.0, get_cursor_rect(&stops, scalar_offset(1)).left);
}

#[test]
fn offset_for_position_covers_vertical_distances_and_nan_points() {
    let content = result("ab", vec![cluster(text_range(0, 1), "a", 10.0), cluster(text_range(1, 2), "b", 10.0)], vec![line(text_range(0, 2), IntRange::new(0, 1), 15.0, 0.0, 20.0, 20.0), line(text_range(2, 2), IntRange::new(2, -1), 35.0, 20.0, 40.0, 0.0)], Vec::new(), Vec::new(), LayoutDebugInfo::default());
    assert_eq!(scalar_offset(0), get_offset_for_position(&content, 2.0, -50.0));
    assert_eq!(scalar_offset(2), get_offset_for_position(&content, 5.0, 90.0));
    assert_eq!(scalar_offset(2), get_selection_offset_for_position(&content, 5.0, 30.0));
    let stops = result("ab", vec![cluster(text_range(0, 2), "ab", 20.0)], vec![line(text_range(0, 2), IntRange::new(0, 0), 15.0, 0.0, 20.0, 20.0)], vec![GlyphRun::new(text_range(0, 2), "test".to_owned(), vec![Glyph::builder(1, text_range(0, 2), 10.0).build(), Glyph::builder(2, text_range(0, 2), 10.0).x(10.0).build()], 20.0)], Vec::new(), LayoutDebugInfo::default());
    assert_eq!(scalar_offset(0), get_offset_for_position(&stops, f32::NAN, 5.0));
    assert_eq!(scalar_offset(0), get_selection_offset_for_position(&stops, f32::NAN, 5.0));
}

#[test]
fn nearest_line_falls_back_to_the_only_line_at_its_end_offset() {
    let content = result("abc", vec![cluster(text_range(0, 2), "ab", 20.0)], vec![line(text_range(0, 2), IntRange::new(0, 0), 15.0, 0.0, 20.0, 20.0)], Vec::new(), Vec::new(), LayoutDebugInfo::default());
    assert_eq!(0, get_line_for_offset(&content, scalar_offset(2)));
}

#[test]
fn ruby_geometry_redistributes_selection_boxes_and_drops_source_stops() {
    let debug = LayoutDebugInfo::builder().ruby_decisions(vec![RubyDecisionInfo::builder(text_range(0, 3), Text::from("zhù"), 0, 15.0, 4.0, 6.0, 0.0).width(30.0).build()]).build();
    let content = result("abc", vec![cluster(text_range(0, 2), "ab", 20.0), cluster(text_range(2, 3), "c", 10.0)], vec![line(text_range(0, 3), IntRange::new(0, 1), 15.0, 0.0, 20.0, 30.0)], vec![GlyphRun::new(text_range(0, 2), "test".to_owned(), vec![Glyph::builder(1, text_range(0, 2), 10.0).build(), Glyph::builder(2, text_range(0, 2), 10.0).x(10.0).build()], 20.0)], Vec::new(), debug);
    let positioned = positioned_clusters(&content);
    assert_eq!(None, positioned[0].source_stops);
    assert_eq!(None, positioned[1].source_stops);
    assert_eq!(17.5, positioned[0].right);
    assert_eq!(17.5, positioned[1].left);
    assert_eq!(30.0, positioned[1].right);
}

#[test]
fn bounding_boxes_slice_zero_width_and_empty_clusters() {
    let content = result("ab", vec![cluster(text_range(0, 1), "a", 10.0), cluster(text_range(1, 2), "b", 0.0)], vec![line(text_range(0, 2), IntRange::new(0, 1), 15.0, 0.0, 20.0, 10.0)], Vec::new(), Vec::new(), LayoutDebugInfo::default());
    let boxes = get_bounding_boxes(&content, text_range(0, 2));
    assert_eq!(2, boxes.len());
    assert_eq!(10.0, boxes[1].left);
    assert_eq!(10.0, boxes[1].right);
    assert_eq!(1, get_bounding_boxes(&content, text_range(1, 2)).len());
}

#[test]
fn positioned_clusters_and_segments_return_empty_without_lines() {
    let no_lines = result("ab", vec![cluster(text_range(0, 1), "a", 10.0)], Vec::new(), Vec::new(), Vec::new(), LayoutDebugInfo::default());
    assert!(positioned_clusters(&no_lines).is_empty());
    assert!(positioned_rich_text_segments(&no_lines, &[RichTextSpan::new(text_range(0, 2), RichTextRole::Background)]).is_empty());
}

#[test]
fn glyph_ink_bounds_skips_unusable_glyphs_and_reports_null() {
    let clusters = vec![cluster(text_range(0, 1), "a", 10.0), cluster(text_range(1, 2), "b", 10.0)];
    let lines = vec![line(text_range(0, 2), IntRange::new(0, 1), 15.0, 0.0, 20.0, 20.0)];
    let no_bounds = result("ab", clusters.clone(), lines.clone(), vec![GlyphRun::new(text_range(0, 2), "test".to_owned(), vec![Glyph::builder(1, text_range(0, 1), 10.0).build()], 20.0)], Vec::new(), LayoutDebugInfo::default());
    assert_eq!(None, glyph_ink_bounds(&no_bounds));
    let nan = result("ab", clusters, lines, vec![GlyphRun::new(text_range(1, 2), "test".to_owned(), vec![Glyph::builder(9, text_range(1, 2), 9.0).x(f32::NAN).bounds(Some(Rect { left: 1.0, top: 2.0, right: 8.0, bottom: 4.0 })).build()], 10.0)], Vec::new(), LayoutDebugInfo::default());
    assert_eq!(None, glyph_ink_bounds(&nan));
}

#[test]
fn cursor_rect_finds_later_clusters_and_rejects_gapped_ranges() {
    let content = result("abc", vec![cluster(text_range(0, 1), "a", 10.0), cluster(text_range(1, 2), "b", 10.0), cluster(text_range(2, 3), "c", 10.0)], vec![line(text_range(0, 3), IntRange::new(0, 2), 15.0, 0.0, 20.0, 30.0)], Vec::new(), Vec::new(), LayoutDebugInfo::default());
    assert_eq!(20.0, get_cursor_rect(&content, scalar_offset(2)).left);
    let gapped = result("abcde", vec![cluster(text_range(0, 1), "a", 10.0), cluster(text_range(4, 5), "e", 10.0)], vec![line(text_range(0, 5), IntRange::new(0, 1), 15.0, 0.0, 20.0, 20.0)], Vec::new(), Vec::new(), LayoutDebugInfo::default());
    assert!(std::panic::catch_unwind(|| get_cursor_rect(&gapped, scalar_offset(2))).is_err());
}

#[test]
fn empty_mid_cluster_holds_the_caret_and_slices_keep_degenerate_rects() {
    let content = result("abc", vec![cluster(text_range(0, 1), "a", 10.0), cluster(text_range(2, 2), "", 0.0), cluster(text_range(2, 3), "c", 10.0)], vec![line(text_range(0, 3), IntRange::new(0, 2), 15.0, 0.0, 20.0, 20.0)], Vec::new(), Vec::new(), LayoutDebugInfo::default());
    assert_eq!(10.0, get_cursor_rect(&content, scalar_offset(2)).left);
    let zero = result("abc", vec![cluster(text_range(0, 1), "a", 10.0), cluster(text_range(1, 2), "b", 0.0), cluster(text_range(2, 3), "c", 10.0)], vec![line(text_range(0, 3), IntRange::new(0, 2), 15.0, 0.0, 20.0, 20.0)], Vec::new(), Vec::new(), LayoutDebugInfo::default());
    let boxes = get_bounding_boxes(&zero, text_range(0, 3));
    assert_eq!(3, boxes.len());
    assert_eq!(boxes[1].left, boxes[1].right);
}

#[test]
fn line_for_offset_inside_a_range_takes_the_zero_distance_arm() {
    let content = result("abcde", vec![cluster(text_range(0, 1), "a", 10.0), cluster(text_range(4, 5), "e", 10.0)], vec![line(text_range(0, 2), IntRange::new(0, 0), 15.0, 0.0, 20.0, 10.0), line(text_range(4, 5), IntRange::new(1, 1), 35.0, 20.0, 40.0, 10.0)], Vec::new(), Vec::new(), LayoutDebugInfo::default());
    assert_eq!(0, get_line_for_offset(&content, scalar_offset(1)));
}

#[test]
fn ruby_spread_shifts_selection_boxes_and_zero_width_rubies_are_ignored() {
    let geometry = |range, text, spread, advance| ClusterGeometryDecisionInfo::builder(range, Text::from(text), Text::from(text), advance - spread, advance - spread, 0.0, 0.0, 0.0, 0.0, 0.0, advance, "test".to_owned(), "test".to_owned()).ruby_spread(spread).build();
    let debug = LayoutDebugInfo::builder().geometry_decisions(vec![geometry(text_range(0, 2), "ab", 5.0, 20.0), geometry(text_range(2, 3), "c", 2.0, 10.0)]).ruby_decisions(vec![RubyDecisionInfo::builder(text_range(0, 3), Text::from("zhù"), 0, 15.0, 4.0, 6.0, 0.0).width(30.0).build(), RubyDecisionInfo::builder(text_range(2, 3), Text::from("x"), 0, 25.0, 4.0, 6.0, 0.0).width(0.0).build()]).build();
    let content = result("abc", vec![cluster(text_range(0, 2), "ab", 20.0), cluster(text_range(2, 3), "c", 10.0)], vec![line(text_range(0, 3), IntRange::new(0, 1), 15.0, 0.0, 20.0, 30.0)], Vec::new(), Vec::new(), debug);
    let positioned = positioned_clusters(&content);
    assert_eq!(15.75, positioned[0].right);
    assert_eq!(15.75, positioned[1].left);
    assert_eq!(30.0, positioned[1].right);
}

#[test]
fn no_arg_positioned_clusters_walks_every_line() {
    let content = result("abcd", vec![cluster(text_range(0, 1), "a", 10.0), cluster(text_range(1, 2), "b", 10.0), cluster(text_range(2, 3), "c", 10.0), cluster(text_range(3, 4), "d", 10.0)], vec![line(text_range(0, 2), IntRange::new(0, 1), 15.0, 0.0, 20.0, 20.0), line(text_range(2, 4), IntRange::new(2, 3), 35.0, 20.0, 40.0, 20.0), line(text_range(4, 4), IntRange::new(2, 1), 55.0, 40.0, 60.0, 0.0)], Vec::new(), Vec::new(), LayoutDebugInfo::default());
    let positioned = positioned_clusters(&content);
    assert_eq!(4, positioned.len());
    assert_eq!(0, positioned[0].line_index);
    assert_eq!(1, positioned[2].line_index);
    assert_eq!(20.0, positioned[3].right);
}

#[test]
fn glyph_ink_bounds_rejects_each_non_finite_edge_independently() {
    for bounds in [Rect { left: f32::NAN, top: 2.0, right: 8.0, bottom: 4.0 }, Rect { left: 1.0, top: f32::NAN, right: 8.0, bottom: 4.0 }, Rect { left: 1.0, top: 2.0, right: f32::NAN, bottom: 4.0 }, Rect { left: 1.0, top: 2.0, right: 8.0, bottom: f32::NAN }] {
        let content = result("ab", vec![cluster(text_range(0, 1), "a", 10.0), cluster(text_range(1, 2), "b", 10.0)], vec![line(text_range(0, 2), IntRange::new(0, 1), 15.0, 0.0, 20.0, 20.0)], vec![GlyphRun::new(text_range(0, 2), "test".to_owned(), vec![Glyph::builder(1, text_range(0, 1), 10.0).bounds(Some(bounds)).build()], 20.0)], Vec::new(), LayoutDebugInfo::default());
        assert_eq!(None, glyph_ink_bounds(&content));
    }
}

#[test]
fn nearest_line_search_covers_all_three_distance_arms() {
    let content = result("abcde", vec![cluster(text_range(0, 1), "a", 10.0), cluster(text_range(4, 5), "e", 10.0)], vec![line(text_range(0, 2), IntRange::new(0, 0), 15.0, 0.0, 20.0, 10.0), line(text_range(4, 5), IntRange::new(1, 1), 35.0, 20.0, 40.0, 10.0)], Vec::new(), Vec::new(), LayoutDebugInfo::default());
    assert_eq!(10.0, get_cursor_rect(&content, scalar_offset(2)).left);
    assert_eq!(10.0, get_cursor_rect(&content, scalar_offset(3)).left);
}

#[test]
fn rubies_on_other_lines_do_not_affect_this_line_geometry() {
    let content = result("ab", vec![cluster(text_range(0, 1), "a", 10.0), cluster(text_range(1, 2), "b", 10.0)], vec![line(text_range(0, 2), IntRange::new(0, 1), 15.0, 0.0, 20.0, 20.0)], Vec::new(), Vec::new(), LayoutDebugInfo::builder().ruby_decisions(vec![RubyDecisionInfo::builder(text_range(0, 2), Text::from("zhù"), 1, 10.0, 4.0, 6.0, 0.0).width(30.0).build()]).build());
    let positioned = positioned_clusters(&content);
    assert_eq!(0.0, positioned[0].left);
    assert_eq!(10.0, positioned[0].right);
    assert_eq!(10.0, positioned[1].left);
    assert_eq!(20.0, positioned[1].right);
}

#[test]
fn nearest_line_search_updates_to_a_strictly_closer_later_line() {
    let mut later = line(text_range(5, 7), IntRange::new(1, 1), 35.0, 20.0, 40.0, 10.0);
    later.indent = 10.0;
    let content = result("abcde", vec![cluster(text_range(0, 1), "a", 10.0), cluster(text_range(5, 6), "e", 10.0)], vec![line(text_range(0, 2), IntRange::new(0, 0), 15.0, 0.0, 20.0, 10.0), later], Vec::new(), Vec::new(), LayoutDebugInfo::default());
    assert_eq!(10.0, get_cursor_rect(&content, scalar_offset(4)).left);
}

#[test]
fn nearest_line_search_covers_both_lambda_copies_of_each_arm() {
    let content = result("abcdefghij", vec![cluster(text_range(2, 3), "c", 10.0), cluster(text_range(3, 4), "d", 10.0), cluster(text_range(6, 7), "g", 10.0), cluster(text_range(7, 8), "h", 10.0)], vec![line(text_range(2, 4), IntRange::new(0, 1), 15.0, 0.0, 20.0, 20.0), line(text_range(6, 8), IntRange::new(2, 3), 35.0, 20.0, 40.0, 20.0)], Vec::new(), Vec::new(), LayoutDebugInfo::default());
    assert_eq!(0.0, get_cursor_rect(&content, scalar_offset(1)).left);
    assert_eq!(20.0, get_cursor_rect(&content, scalar_offset(8)).left);
    assert_eq!(20.0, get_cursor_rect(&content, scalar_offset(9)).left);
}