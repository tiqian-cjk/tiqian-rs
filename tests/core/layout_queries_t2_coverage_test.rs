use std::sync::Arc;

use tiqian::core::geometry::{scalar_offset, text_range, Rect};
use tiqian::core::layout_queries::{
    coerce_selection_offset, get_offset_for_position, get_selection_word_boundary,
    get_selection_word_boundary_for_position, positioned_clusters, positioned_rich_text_segments,
    resolved_background_corner_radii, rich_text_background_segments,
    rich_text_decoration_line_y, trimmed_rich_text_decoration_segments, RichTextCornerRadii,
    RichTextLineSegment,
};
use tiqian::core::source_interaction_boundaries::SourceBoundaryBias;
use tiqian::core::text_model::{
    RichTextBackgroundMetricPolicy, RichTextBackgroundPaint, RichTextPaint, RichTextRole,
    RichTextSpan,
};

use super::layout_queries_test_support::{
    background_geometry_result, interaction_boundary_result, metric, punctuation_glue_result,
    ruby_selection_result, sample_result, word_boundary_result,
};

#[test]
fn positioned_clusters_follow_line_indent_and_advance() {
    let positioned = positioned_clusters(&sample_result());
    assert_eq!(Rect { left: 4.0, top: 0.0, right: 14.0, bottom: 20.0 }, positioned[0].rect());
    assert_eq!(Rect { left: 14.0, top: 0.0, right: 34.0, bottom: 20.0 }, positioned[1].rect());
    assert_eq!(Rect { left: 0.0, top: 20.0, right: 10.0, bottom: 40.0 }, positioned[2].rect());
}

#[test]
fn positioned_clusters_separate_occupied_box_from_consumed_leading_glue_draw_origin() {
    let result = punctuation_glue_result(4.0);
    let positioned = positioned_clusters(&result);
    assert_eq!(Rect { left: 0.0, top: 0.0, right: 10.0, bottom: 20.0 }, positioned[0].rect());
    assert_eq!(-4.0, positioned[0].draw_x);
    assert_eq!(scalar_offset(0), get_offset_for_position(&result, -3.0, 5.0));
}

#[test]
fn rich_text_segments_reuse_positioned_cluster_geometry_and_split_lines() {
    let result = sample_result();
    let span = RichTextSpan::with_paint(text_range(1, 4), RichTextRole::Background, RichTextPaint::builder().argb(0x33FF0000).build());
    let segments = positioned_rich_text_segments(&result, &[span]);
    assert_eq!(2, segments.len());
    assert_eq!(text_range(1, 3), segments[0].range);
    assert_eq!(Rect { left: 14.0, top: 0.0, right: 34.0, bottom: 20.0 }, segments[0].rect());
    assert_eq!(text_range(3, 4), segments[1].range);
    assert_eq!(Rect { left: 0.0, top: 20.0, right: 10.0, bottom: 40.0 }, segments[1].rect());
}

#[test]
fn rich_text_decoration_keeps_punctuation_glue_inside_its_range() {
    let result = punctuation_glue_result(0.0);
    let span = RichTextSpan::new(text_range(1, 4), RichTextRole::Underline);
    let decoration = trimmed_rich_text_decoration_segments(&result, &positioned_rich_text_segments(&result, &[span]));
    assert_eq!(Rect { left: 10.0, top: 0.0, right: 35.0, bottom: 20.0 }, decoration[0].rect());
}

#[test]
fn rich_text_decoration_does_not_trim_already_consumed_opening_glue_twice() {
    let result = punctuation_glue_result(5.0);
    let span = RichTextSpan::new(text_range(0, 1), RichTextRole::Underline);
    let decoration = trimmed_rich_text_decoration_segments(&result, &positioned_rich_text_segments(&result, &[span]));
    assert_eq!(0.0, decoration[0].left);
}

#[test]
fn custom_line_styles_reuse_the_renderer_underline_height() {
    let result = punctuation_glue_result(0.0);
    let span = RichTextSpan::new(text_range(0, 4), RichTextRole::Underline);
    let segment = trimmed_rich_text_decoration_segments(&result, &positioned_rich_text_segments(&result, &[span])).remove(0);
    assert!((segment.baseline + 10.0 * 0.18 - rich_text_decoration_line_y(&result, &segment, 1.0)).abs() < 0.001);
}

#[test]
fn line_through_bisects_the_ideographic_metric_box() {
    let result = background_geometry_result(vec![metric(text_range(0, 3), 8.0, 2.0, "IdeographicEmBox")]);
    let span = RichTextSpan::new(text_range(0, 3), RichTextRole::LineThrough);
    let segment = trimmed_rich_text_decoration_segments(&result, &positioned_rich_text_segments(&result, &[span])).remove(0);
    assert!((17.0 - rich_text_decoration_line_y(&result, &segment, 1.0)).abs() < 0.001);
}

#[test]
fn rich_text_background_keeps_internal_gaps_but_trims_its_outer_layout_space() {
    let result = background_geometry_result(Vec::new());
    let full = RichTextSpan::new(text_range(0, 3), RichTextRole::Background);
    let final_character = RichTextSpan::new(text_range(2, 3), RichTextRole::Background);
    let full_segment = rich_text_background_segments(&result, &positioned_rich_text_segments(&result, &[full])).remove(0);
    let final_segment = rich_text_background_segments(&result, &positioned_rich_text_segments(&result, &[final_character])).remove(0);
    assert_eq!(Rect { left: 0.0, top: 11.2, right: 29.0, bottom: 21.2 }, full_segment.rect());
    assert_eq!(Rect { left: 19.0, top: 11.2, right: 29.0, bottom: 21.2 }, final_segment.rect());
}

#[test]
fn uniform_text_style_background_ignores_fallback_face_height_and_adds_padding() {
    let result = background_geometry_result(vec![metric(text_range(0, 1), 8.0, 2.0, "IdeographicEmBox"), metric(text_range(1, 3), 12.0, 4.0, "RawFontBox")]);
    let paint = RichTextPaint::builder().background(RichTextBackgroundPaint::builder().vertical_padding(1.0).corner_radius(2.0).metric_policy(RichTextBackgroundMetricPolicy::UniformTextStyle).build()).build();
    let first = RichTextSpan::with_paint(text_range(0, 1), RichTextRole::Background, paint.clone());
    let mixed = RichTextSpan::with_paint(text_range(0, 3), RichTextRole::Background, paint);
    let segments = rich_text_background_segments(&result, &positioned_rich_text_segments(&result, &[first, mixed]));
    assert_eq!(2, segments.len());
    assert_eq!(11.0, segments[0].top);
    assert_eq!(23.0, segments[0].bottom);
    assert_eq!(segments[0].top, segments[1].top);
    assert_eq!(segments[0].bottom, segments[1].bottom);
}

#[test]
fn background_continuation_corners_keep_only_true_source_ends_fully_rounded() {
    let span = Arc::new(RichTextSpan::with_paint(text_range(0, 12), RichTextRole::InlineCode, RichTextPaint::builder().background(RichTextBackgroundPaint::builder().corner_radius(3.0).continuation_corner_radius(1.0).build()).build()));
    let segment = |start, end| RichTextLineSegment::new(Arc::clone(&span), 0, text_range(start, end), 0.0, 0.0, 40.0, 20.0, 16.0);
    assert_eq!(RichTextCornerRadii { top_left: 3.0, top_right: 1.0, bottom_right: 1.0, bottom_left: 3.0 }, resolved_background_corner_radii(&segment(0, 4), 0.0));
    assert_eq!(RichTextCornerRadii { top_left: 1.0, top_right: 1.0, bottom_right: 1.0, bottom_left: 1.0 }, resolved_background_corner_radii(&segment(4, 8), 0.0));
    assert_eq!(RichTextCornerRadii { top_left: 1.0, top_right: 3.0, bottom_right: 3.0, bottom_left: 1.0 }, resolved_background_corner_radii(&segment(8, 12), 0.0));
    assert_eq!(RichTextCornerRadii { top_left: 3.0, top_right: 3.0, bottom_right: 3.0, bottom_left: 3.0 }, resolved_background_corner_radii(&segment(0, 12), 0.0));
}

#[test]
fn background_continuation_radius_defaults_to_the_authored_corner_radius() {
    let background = RichTextBackgroundPaint::builder().corner_radius(5.0).build();
    assert_eq!(5.0, background.continuation_corner_radius);
}

#[test]
fn adjacent_backgrounds_with_the_same_style_share_one_clearance() {
    let result = sample_result();
    let paint = RichTextPaint::builder().adjacent_same_style_clearance(2.0).build();
    let spans = [RichTextSpan::with_paint(text_range(0, 1), RichTextRole::Background, paint.clone()), RichTextSpan::with_paint(text_range(1, 3), RichTextRole::Background, paint)];
    let segments = rich_text_background_segments(&result, &positioned_rich_text_segments(&result, &spans));
    assert_eq!(2, segments.len());
    assert!((2.0 - (segments[1].left - segments[0].right)).abs() < 0.001);
}

#[test]
fn adjacent_line_decorations_with_the_same_style_share_one_clearance() {
    let result = sample_result();
    let paint = RichTextPaint::builder().adjacent_same_style_clearance(2.0).build();
    let spans = [RichTextSpan::with_paint(text_range(0, 1), RichTextRole::Underline, paint.clone()), RichTextSpan::with_paint(text_range(1, 3), RichTextRole::Underline, paint)];
    let segments = trimmed_rich_text_decoration_segments(&result, &positioned_rich_text_segments(&result, &spans));
    assert_eq!(2, segments.len());
    assert!((2.0 - (segments[1].left - segments[0].right)).abs() < 0.001);
}

#[test]
fn adjacent_background_and_underline_do_not_avoid_across_styles() {
    let result = sample_result();
    let paint = RichTextPaint::builder().adjacent_same_style_clearance(2.0).build();
    let occupied = positioned_rich_text_segments(&result, &[
        RichTextSpan::with_paint(text_range(0, 1), RichTextRole::Background, paint.clone()),
        RichTextSpan::with_paint(text_range(1, 3), RichTextRole::Underline, paint),
    ]);
    let fill = rich_text_background_segments(&result, &occupied).remove(0);
    let line = trimmed_rich_text_decoration_segments(&result, &occupied).remove(0);
    assert_eq!(14.0, fill.right);
    assert_eq!(14.0, line.left);
}

#[test]
fn hit_testing_chooses_offset_from_tiqian_cluster_advances() {
    let result = sample_result();
    for (x, y, expected) in [(3.0, 5.0, 0), (18.0, 5.0, 1), (24.0, 5.0, 2), (4.0, 25.0, 3), (30.0, 25.0, 4)] {
        assert_eq!(scalar_offset(expected), get_offset_for_position(&result, x, y));
    }
}

#[test]
fn external_selection_offsets_respect_directional_boundary_bias() {
    let result = interaction_boundary_result();
    assert_eq!(
        vec![text_range(0, 1), text_range(1, 3), text_range(3, 6)],
        positioned_clusters(&result)
            .iter()
            .map(|cluster| cluster.range)
            .collect::<Vec<_>>(),
    );
    assert_eq!(scalar_offset(1), coerce_selection_offset(&result, scalar_offset(2), SourceBoundaryBias::Backward));
    assert_eq!(scalar_offset(3), coerce_selection_offset(&result, scalar_offset(2), SourceBoundaryBias::Forward));
    assert_eq!(scalar_offset(3), coerce_selection_offset(&result, scalar_offset(2), SourceBoundaryBias::Nearest));
    assert_eq!(scalar_offset(3), coerce_selection_offset(&result, scalar_offset(4), SourceBoundaryBias::Backward));
    assert_eq!(scalar_offset(6), coerce_selection_offset(&result, scalar_offset(4), SourceBoundaryBias::Forward));
}

#[test]
fn supported_source_sequence_remains_atomic_across_engine_cluster_boundaries() {
    let result = super::layout_queries_test_support::result(
        "é",
        vec![
            super::layout_queries_test_support::cluster(text_range(0, 1), "e", 10.0),
            super::layout_queries_test_support::cluster(text_range(1, 2), "́", 10.0),
        ],
        vec![super::layout_queries_test_support::line(text_range(0, 2), tiqian::core::int_range::IntRange::new(0, 1), 15.0, 0.0, 20.0, 20.0)],
        Vec::new(), Vec::new(), tiqian::core::layout_model::LayoutDebugInfo::default(),
    );
    assert_eq!(scalar_offset(0), coerce_selection_offset(&result, scalar_offset(1), SourceBoundaryBias::Backward));
    assert_eq!(scalar_offset(2), coerce_selection_offset(&result, scalar_offset(1), SourceBoundaryBias::Forward));
}

#[test]
fn selection_word_boundary_expands_latin_but_keeps_han_atomic() {
    let result = word_boundary_result();
    assert_eq!(text_range(2, 10), get_selection_word_boundary(&result, scalar_offset(6)));
    assert_eq!(text_range(0, 1), get_selection_word_boundary(&result, scalar_offset(0)));
    assert_eq!(text_range(1, 2), get_selection_word_boundary(&result, scalar_offset(1)));
    assert_eq!(text_range(11, 12), get_selection_word_boundary(&result, scalar_offset(12)));
    assert_eq!(Some(text_range(0, 1)), get_selection_word_boundary_for_position(&result, 5.0, 10.0));
    assert_eq!(Some(text_range(2, 10)), get_selection_word_boundary_for_position(&result, 60.0, 10.0));
}

#[test]
fn ruby_selection_geometry_redistributes_avoidance_spread_without_overlap() {
    let positioned = positioned_clusters(&ruby_selection_result());
    assert_eq!(Rect { left: -6.0, top: 0.0, right: 26.0, bottom: 20.0 }, positioned[0].rect());
    assert_eq!(Rect { left: 29.0, top: 0.0, right: 61.0, bottom: 20.0 }, positioned[1].rect());
    assert_eq!(Rect { left: 64.0, top: 0.0, right: 96.0, bottom: 20.0 }, positioned[2].rect());
    assert!(positioned.windows(2).all(|items| items[0].right <= items[1].left));
}