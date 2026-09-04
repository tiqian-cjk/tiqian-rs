use std::sync::Arc;

use tiqian::core::geometry::{text_range, TextRange};
use tiqian::core::int_range::IntRange;
use tiqian::core::layout_model::{ClusterGeometryDecisionInfo, Glyph, GlyphRun, LayoutDebugInfo};
use tiqian::core::layout_queries::{
    positioned_rich_text_segments, resolved_background_corner_radii, rich_text_background_segments,
    rich_text_decoration_line_y, trimmed_rich_text_decoration_segments, RichTextCornerRadii,
    RichTextLineSegment,
};
use tiqian::core::text::Text;
use tiqian::core::text_model::{
    RichTextBackgroundMetricPolicy, RichTextBackgroundPaint, RichTextPaint, RichTextRole,
    RichTextSpan, TextSpan, TextStyle,
};

use super::layout_queries_test_support::{cluster, line, metric, result};

fn segment(range: TextRange, role: RichTextRole, paint: RichTextPaint, span_range: TextRange, left: f32, right: f32) -> RichTextLineSegment {
    RichTextLineSegment::new(Arc::new(RichTextSpan::with_paint(span_range, role, paint)), 0, range, left, 0.0, right, 20.0, 15.0)
}

fn two_cluster_result(debug: LayoutDebugInfo) -> tiqian::core::layout_model::LayoutResult {
    result("ab", vec![cluster(text_range(0, 1), "a", 10.0), cluster(text_range(1, 2), "b", 10.0)], vec![line(text_range(0, 2), IntRange::new(0, 1), 15.0, 0.0, 20.0, 20.0)], Vec::new(), Vec::new(), debug)
}

#[test]
fn corner_radii_predicates_cover_every_comparison() {
    assert!(RichTextCornerRadii { top_left: 0.0, top_right: 0.0, bottom_right: 0.0, bottom_left: 0.0 }.is_square());
    for radii in [RichTextCornerRadii { top_left: 1.0, top_right: 0.0, bottom_right: 0.0, bottom_left: 0.0 }, RichTextCornerRadii { top_left: 0.0, top_right: 1.0, bottom_right: 0.0, bottom_left: 0.0 }, RichTextCornerRadii { top_left: 0.0, top_right: 0.0, bottom_right: 1.0, bottom_left: 0.0 }, RichTextCornerRadii { top_left: 0.0, top_right: 0.0, bottom_right: 0.0, bottom_left: 1.0 }] { assert!(!radii.is_square()); }
    assert!(RichTextCornerRadii { top_left: 2.0, top_right: 2.0, bottom_right: 2.0, bottom_left: 2.0 }.is_uniform());
    for radii in [RichTextCornerRadii { top_left: 1.0, top_right: 2.0, bottom_right: 2.0, bottom_left: 2.0 }, RichTextCornerRadii { top_left: 2.0, top_right: 1.0, bottom_right: 2.0, bottom_left: 2.0 }, RichTextCornerRadii { top_left: 2.0, top_right: 2.0, bottom_right: 1.0, bottom_left: 2.0 }, RichTextCornerRadii { top_left: 2.0, top_right: 2.0, bottom_right: 2.0, bottom_left: 1.0 }] { assert!(!radii.is_uniform()); }
}

#[test]
fn resolved_corner_radii_rejects_invalid_insets_and_resolves_continuations() {
    let content = segment(text_range(1, 2), RichTextRole::Background, RichTextPaint::builder().background(RichTextBackgroundPaint::builder().corner_radius(6.0).continuation_corner_radius(2.0).build()).build(), text_range(0, 3), 0.0, 30.0);
    assert!(std::panic::catch_unwind(|| resolved_background_corner_radii(&content, -1.0)).is_err());
    assert!(std::panic::catch_unwind(|| resolved_background_corner_radii(&content, f32::NAN)).is_err());
    assert_eq!(RichTextCornerRadii { top_left: 2.0, top_right: 2.0, bottom_right: 2.0, bottom_left: 2.0 }, resolved_background_corner_radii(&content, 0.0));
}

#[test]
fn rich_text_segments_split_on_line_breaks_and_cluster_gaps() {
    let content = result("abcd", vec![cluster(text_range(0, 1), "a", 10.0), cluster(text_range(2, 3), "c", 10.0), cluster(text_range(3, 4), "d", 10.0)], vec![line(text_range(0, 3), IntRange::new(0, 1), 15.0, 0.0, 20.0, 20.0), line(text_range(3, 4), IntRange::new(2, 2), 35.0, 20.0, 40.0, 10.0)], Vec::new(), Vec::new(), LayoutDebugInfo::default());
    let split = positioned_rich_text_segments(&content, &[RichTextSpan::new(text_range(0, 4), RichTextRole::Underline)]);
    assert_eq!(3, split.len());
    assert_eq!(text_range(0, 1), split[0].range); assert_eq!(text_range(2, 3), split[1].range); assert_eq!(text_range(3, 4), split[2].range);
    assert_eq!(0, split[0].line_index); assert_eq!(0, split[1].line_index); assert_eq!(1, split[2].line_index);
}

#[test]
fn rich_text_segments_skip_zero_length_clusters_between_slices() {
    let content = result("ab", vec![cluster(text_range(0, 1), "a", 10.0), cluster(text_range(1, 1), "", 0.0), cluster(text_range(1, 2), "b", 10.0)], vec![line(text_range(0, 2), IntRange::new(0, 2), 15.0, 0.0, 20.0, 20.0)], Vec::new(), Vec::new(), LayoutDebugInfo::default());
    let segments = positioned_rich_text_segments(&content, &[RichTextSpan::new(text_range(0, 2), RichTextRole::Underline)]);
    assert_eq!(1, segments.len()); assert_eq!(text_range(0, 2), segments[0].range); assert_eq!(0.0, segments[0].left); assert_eq!(20.0, segments[0].right);
}

#[test]
fn trimmed_decoration_segments_keep_only_decoration_roles() {
    let content = two_cluster_result(LayoutDebugInfo::default());
    let underline = segment(text_range(0, 2), RichTextRole::Underline, RichTextPaint::default(), text_range(0, 2), 0.0, 20.0);
    assert_eq!(vec![underline.clone()], trimmed_rich_text_decoration_segments(&content, &[underline]));
    let background = segment(text_range(0, 2), RichTextRole::Background, RichTextPaint::default(), text_range(0, 2), 0.0, 20.0);
    assert!(trimmed_rich_text_decoration_segments(&content, &[background]).is_empty());
}

#[test]
fn background_segments_pass_through_unmatchable_segments() {
    let content = two_cluster_result(LayoutDebugInfo::default());
    let far = segment(text_range(10, 12), RichTextRole::Background, RichTextPaint::default(), text_range(10, 12), 0.0, 20.0);
    assert_eq!(vec![far.clone()], rich_text_background_segments(&content, &[far]));
    let orphan = RichTextLineSegment::new(Arc::new(RichTextSpan::new(text_range(0, 1), RichTextRole::Background)), 5, text_range(0, 1), 0.0, 0.0, 20.0, 20.0, 15.0);
    assert_eq!(vec![orphan.clone()], rich_text_background_segments(&content, &[orphan]));
}

#[test]
fn background_segments_trim_glue_apply_padding_and_use_glyph_advances() {
    let glue = ClusterGeometryDecisionInfo::builder(text_range(0, 1), Text::from("，"), Text::from("，"), 10.0, 5.0, 4.0, 1.0, 4.0, 1.0, 0.0, 10.0, "test".to_owned(), "test".to_owned()).build();
    let content = result("，字", vec![cluster(text_range(0, 1), "，", 10.0), cluster(text_range(1, 2), "字", 10.0)], vec![line(text_range(0, 2), IntRange::new(0, 1), 15.0, 0.0, 20.0, 20.0)], vec![GlyphRun::new(text_range(1, 2), "test".to_owned(), vec![Glyph::builder(9, text_range(1, 2), 9.0).x(1.0).build()], 10.0)], Vec::new(), LayoutDebugInfo::builder().geometry_decisions(vec![glue]).build());
    let full = rich_text_background_segments(&content, &[segment(text_range(0, 2), RichTextRole::Background, RichTextPaint::default(), text_range(0, 2), 0.0, 20.0)]).remove(0);
    assert_eq!(3.0, full.left); assert_eq!(20.0, full.right); assert_eq!(15.0 - 10.0 * 0.88, full.top); assert_eq!(15.0 + 10.0 * 0.12, full.bottom);
}

#[test]
fn marked_faces_use_metric_decisions_when_they_cover_the_cluster() {
    let content = two_cluster_result(LayoutDebugInfo::builder().metric_decisions(vec![metric(text_range(0, 2), 7.0, 3.0, "IdeographicEmBox")]).build());
    let box_ = rich_text_background_segments(&content, &[segment(text_range(0, 2), RichTextRole::Background, RichTextPaint::default(), text_range(0, 2), 0.0, 20.0)]).remove(0);
    assert_eq!(8.0, box_.top); assert_eq!(18.0, box_.bottom);
}

#[test]
fn uniform_text_style_falls_back_when_every_metric_field_differs() {
    let base = TextStyle::builder().font_size(10.0).build();
    for style in [TextStyle::builder().font_families(vec!["other".to_owned()]).font_size(10.0).build(), TextStyle::builder().font_size(11.0).build(), TextStyle::builder().font_size(10.0).locale("ja-JP".to_owned()).build(), TextStyle::builder().font_size(10.0).font_weight(700).build(), TextStyle::builder().font_size(10.0).italic(true).build(), TextStyle::builder().font_size(10.0).baseline_shift(2.0).build()] {
        let content = result("ab", vec![cluster(text_range(0, 1), "a", 10.0), cluster(text_range(1, 2), "b", 10.0)], vec![line(text_range(0, 2), IntRange::new(0, 1), 15.0, 0.0, 20.0, 20.0)], Vec::new(), vec![TextSpan { range: text_range(0, 1), style: style.clone() }], LayoutDebugInfo::builder().metric_decisions(vec![metric(text_range(1, 2), 9.0, 1.0, "LatinBox")]).build());
        let paint = RichTextPaint::builder().background(RichTextBackgroundPaint::builder().metric_policy(RichTextBackgroundMetricPolicy::UniformTextStyle).build()).build();
        let box_ = rich_text_background_segments(&content, &[segment(text_range(0, 2), RichTextRole::Background, paint, text_range(0, 2), 0.0, 20.0)]).remove(0);
        assert_eq!(15.0 - style.font_size * 0.88, box_.top);
    }
    assert_eq!(10.0, base.font_size);
}

#[test]
fn uniform_text_style_prefers_ideographic_metrics_then_any_matching_face() {
    let paint = RichTextPaint::builder().background(RichTextBackgroundPaint::builder().metric_policy(RichTextBackgroundMetricPolicy::UniformTextStyle).build()).build();
    let latin = two_cluster_result(LayoutDebugInfo::builder().metric_decisions(vec![metric(text_range(0, 2), 9.0, 1.0, "LatinBox")]).build());
    let latin_box = rich_text_background_segments(&latin, &[segment(text_range(0, 2), RichTextRole::Background, paint.clone(), text_range(0, 2), 0.0, 20.0)]).remove(0);
    assert_eq!(6.0, latin_box.top); assert_eq!(16.0, latin_box.bottom);
    let both = two_cluster_result(LayoutDebugInfo::builder().metric_decisions(vec![metric(text_range(0, 1), 9.0, 1.0, "LatinBox"), metric(text_range(0, 2), 8.0, 2.0, "IdeographicEmBox")]).build());
    let ideographic = rich_text_background_segments(&both, &[segment(text_range(0, 2), RichTextRole::Background, paint, text_range(0, 2), 0.0, 20.0)]).remove(0);
    assert_eq!(7.0, ideographic.top); assert_eq!(17.0, ideographic.bottom);
}

#[test]
fn adjacent_same_style_segments_share_clearance() {
    let content = two_cluster_result(LayoutDebugInfo::default());
    let paint = RichTextPaint::builder().adjacent_same_style_clearance(4.0).build();
    let out = rich_text_background_segments(&content, &[segment(text_range(0, 1), RichTextRole::Background, paint.clone(), text_range(0, 1), 0.0, 10.0), segment(text_range(1, 2), RichTextRole::Background, paint, text_range(1, 2), 10.0, 20.0)]);
    assert_eq!(8.0, out[0].right); assert_eq!(12.0, out[1].left);
}

#[test]
fn decoration_line_y_requires_valid_stroke_and_decoration_roles() {
    let content = two_cluster_result(LayoutDebugInfo::default());
    let underline = segment(text_range(0, 1), RichTextRole::Underline, RichTextPaint::default(), text_range(0, 1), 0.0, 10.0);
    assert!(std::panic::catch_unwind(|| rich_text_decoration_line_y(&content, &underline, -1.0)).is_err());
    assert!(std::panic::catch_unwind(|| rich_text_decoration_line_y(&content, &underline, f32::NAN)).is_err());
    let background = segment(text_range(0, 1), RichTextRole::Background, RichTextPaint::default(), text_range(0, 1), 0.0, 10.0);
    assert!(std::panic::catch_unwind(|| rich_text_decoration_line_y(&content, &background, 1.0)).is_err());
}

#[test]
fn same_span_slices_across_a_source_boundary_merge_into_one_segment() {
    let content = two_cluster_result(LayoutDebugInfo::default());
    let segments = positioned_rich_text_segments(&content, &[RichTextSpan::new(text_range(0, 2), RichTextRole::Background)]);
    assert_eq!(1, segments.len()); assert_eq!(text_range(0, 2), segments[0].range); assert_eq!(20.0, segments[0].right);
}

#[test]
fn background_trailing_edge_uses_glyph_advances_when_available() {
    let content = result("ab", vec![cluster(text_range(0, 1), "a", 10.0), cluster(text_range(1, 2), "b", 10.0)], vec![line(text_range(0, 2), IntRange::new(0, 1), 15.0, 0.0, 20.0, 20.0)], vec![GlyphRun::new(text_range(1, 2), "test".to_owned(), vec![Glyph::builder(2, text_range(1, 2), 5.0).build()], 10.0)], Vec::new(), LayoutDebugInfo::default());
    assert_eq!(15.0, rich_text_background_segments(&content, &[segment(text_range(0, 2), RichTextRole::Background, RichTextPaint::default(), text_range(0, 2), 0.0, 20.0)]).remove(0).right);
}

#[test]
fn clearance_needs_same_role_and_uses_the_smaller_side() {
    let content = two_cluster_result(LayoutDebugInfo::default());
    let weak = RichTextPaint::builder().adjacent_same_style_clearance(2.0).build(); let strong = RichTextPaint::builder().adjacent_same_style_clearance(6.0).build();
    let out = rich_text_background_segments(&content, &[segment(text_range(0, 1), RichTextRole::Background, weak, text_range(0, 1), 0.0, 10.0), segment(text_range(1, 2), RichTextRole::Background, strong, text_range(1, 2), 10.0, 20.0)]);
    assert_eq!(9.0, out[0].right); assert_eq!(11.0, out[1].left);
}

#[test]
fn metric_decisions_must_fully_contain_the_cluster() {
    for range in [text_range(1, 2), text_range(0, 1)] {
        let content = result("ab", vec![cluster(text_range(0, 2), "ab", 20.0)], vec![line(text_range(0, 2), IntRange::new(0, 0), 15.0, 0.0, 20.0, 20.0)], Vec::new(), Vec::new(), LayoutDebugInfo::builder().metric_decisions(vec![metric(range, 7.0, 3.0, "IdeographicEmBox")]).build());
        let out = rich_text_background_segments(&content, &[segment(text_range(0, 2), RichTextRole::Background, RichTextPaint::default(), text_range(0, 2), 0.0, 20.0)]).remove(0);
        assert_eq!(15.0 - 10.0 * 0.88, out.top);
    }
}

#[test]
fn decoration_style_resolves_inside_spans_and_at_their_edges() {
    let content = result("abc", vec![cluster(text_range(0, 1), "a", 10.0), cluster(text_range(1, 2), "b", 10.0), cluster(text_range(2, 3), "c", 10.0)], vec![line(text_range(0, 3), IntRange::new(0, 2), 15.0, 0.0, 20.0, 30.0)], Vec::new(), vec![TextSpan { range: text_range(0, 1), style: TextStyle::builder().font_size(10.0).build() }, TextSpan { range: text_range(2, 3), style: TextStyle::builder().font_size(20.0).build() }], LayoutDebugInfo::default());
    assert_eq!(16.8, rich_text_decoration_line_y(&content, &segment(text_range(1, 2), RichTextRole::Underline, RichTextPaint::default(), text_range(1, 2), 10.0, 20.0), 1.0));
    assert_eq!(18.6, rich_text_decoration_line_y(&content, &segment(text_range(2, 3), RichTextRole::Underline, RichTextPaint::default(), text_range(2, 3), 20.0, 30.0), 1.0));
}

#[test]
fn glue_trim_skips_interior_segment_edges() {
    let glue = ClusterGeometryDecisionInfo::builder(text_range(0, 2), Text::from("ab"), Text::from("ab"), 20.0, 10.0, 4.0, 1.0, 4.0, 1.0, 0.0, 20.0, "test".to_owned(), "test".to_owned()).build();
    let content = result("ab", vec![cluster(text_range(0, 2), "ab", 20.0)], vec![line(text_range(0, 2), IntRange::new(0, 0), 15.0, 0.0, 20.0, 20.0)], Vec::new(), Vec::new(), LayoutDebugInfo::builder().geometry_decisions(vec![glue]).build());
    assert_eq!(10.0, rich_text_background_segments(&content, &[segment(text_range(1, 2), RichTextRole::Background, RichTextPaint::default(), text_range(1, 2), 10.0, 20.0)]).remove(0).left);
    assert_eq!(10.0, rich_text_background_segments(&content, &[segment(text_range(0, 1), RichTextRole::Background, RichTextPaint::default(), text_range(0, 1), 0.0, 10.0)]).remove(0).right);
}

#[test]
fn background_segment_outside_every_span_uses_the_paragraph_style() {
    let content = result("abc", vec![cluster(text_range(0, 1), "a", 10.0), cluster(text_range(1, 2), "b", 10.0), cluster(text_range(2, 3), "c", 10.0)], vec![line(text_range(0, 3), IntRange::new(0, 2), 15.0, 0.0, 20.0, 30.0)], Vec::new(), vec![TextSpan { range: text_range(1, 2), style: TextStyle::builder().font_size(40.0).build() }], LayoutDebugInfo::default());
    assert_eq!(6.2, rich_text_background_segments(&content, &[segment(text_range(0, 1), RichTextRole::Background, RichTextPaint::default(), text_range(0, 1), 0.0, 10.0)]).remove(0).top);
    assert_eq!(6.2, rich_text_background_segments(&content, &[segment(text_range(2, 3), RichTextRole::Background, RichTextPaint::default(), text_range(2, 3), 20.0, 30.0)]).remove(0).top);
}

#[test]
fn clearance_takes_the_smaller_side_whichever_segment_owns_it() {
    let content = two_cluster_result(LayoutDebugInfo::default());
    let out = rich_text_background_segments(&content, &[segment(text_range(0, 1), RichTextRole::Background, RichTextPaint::builder().adjacent_same_style_clearance(6.0).build(), text_range(0, 1), 0.0, 10.0), segment(text_range(1, 2), RichTextRole::Background, RichTextPaint::builder().adjacent_same_style_clearance(2.0).build(), text_range(1, 2), 10.0, 20.0)]);
    assert_eq!(9.0, out[0].right); assert_eq!(11.0, out[1].left);
}

#[test]
fn uniform_text_style_policy_resolves_span_style_or_paragraph_style() {
    let paint = RichTextPaint::builder().background(RichTextBackgroundPaint::builder().metric_policy(RichTextBackgroundMetricPolicy::UniformTextStyle).build()).build();
    let content = result("abc", vec![cluster(text_range(0, 1), "a", 10.0), cluster(text_range(1, 2), "b", 10.0), cluster(text_range(2, 3), "c", 10.0)], vec![line(text_range(0, 3), IntRange::new(0, 2), 15.0, 0.0, 20.0, 30.0)], Vec::new(), vec![TextSpan { range: text_range(1, 2), style: TextStyle::builder().font_size(40.0).build() }], LayoutDebugInfo::default());
    assert_eq!(6.2, rich_text_background_segments(&content, &[segment(text_range(0, 1), RichTextRole::Background, paint.clone(), text_range(0, 1), 0.0, 10.0)]).remove(0).top);
    assert_eq!(0.0, rich_text_background_segments(&content, &[segment(text_range(1, 2), RichTextRole::Background, paint, text_range(1, 2), 10.0, 20.0)]).remove(0).top);
}

#[test]
fn trailing_glue_is_skipped_when_no_cluster_ends_before_the_segment_end() {
    let content = result("ab", vec![cluster(text_range(1, 2), "b", 10.0)], vec![line(text_range(0, 2), IntRange::new(0, 0), 15.0, 0.0, 20.0, 10.0)], Vec::new(), Vec::new(), LayoutDebugInfo::default());
    assert_eq!(10.0, rich_text_background_segments(&content, &[segment(text_range(0, 1), RichTextRole::Background, RichTextPaint::default(), text_range(0, 1), 0.0, 10.0)]).remove(0).right);
}

#[test]
fn decoration_line_y_without_spans_uses_the_paragraph_style() {
    let content = two_cluster_result(LayoutDebugInfo::default());
    assert_eq!(16.8, rich_text_decoration_line_y(&content, &segment(text_range(0, 2), RichTextRole::Underline, RichTextPaint::default(), text_range(0, 2), 0.0, 20.0), 1.0));
}

#[test]
fn background_trailing_edge_picks_the_largest_glyph_advance() {
    let content = result("ab", vec![cluster(text_range(0, 1), "a", 10.0), cluster(text_range(1, 2), "b", 10.0)], vec![line(text_range(0, 2), IntRange::new(0, 1), 15.0, 0.0, 20.0, 20.0)], vec![GlyphRun::new(text_range(1, 2), "test".to_owned(), vec![Glyph::builder(1, text_range(1, 2), 5.0).build(), Glyph::builder(2, text_range(1, 2), 6.0).build()], 10.0)], Vec::new(), LayoutDebugInfo::default());
    assert_eq!(16.0, rich_text_background_segments(&content, &[segment(text_range(0, 2), RichTextRole::Background, RichTextPaint::default(), text_range(0, 2), 0.0, 20.0)]).remove(0).right);
}

#[test]
fn background_trailing_edge_keeps_the_first_glyph_when_it_is_largest() {
    let content = result("ab", vec![cluster(text_range(0, 1), "a", 10.0), cluster(text_range(1, 2), "b", 10.0)], vec![line(text_range(0, 2), IntRange::new(0, 1), 15.0, 0.0, 20.0, 20.0)], vec![GlyphRun::new(text_range(1, 2), "test".to_owned(), vec![Glyph::builder(1, text_range(1, 2), 6.0).build(), Glyph::builder(2, text_range(1, 2), 5.0).build()], 10.0)], Vec::new(), LayoutDebugInfo::default());
    assert_eq!(16.0, rich_text_background_segments(&content, &[segment(text_range(0, 2), RichTextRole::Background, RichTextPaint::default(), text_range(0, 2), 0.0, 20.0)]).remove(0).right);
}

#[test]
fn uniform_text_style_policy_picks_the_last_matching_span() {
    let paint = RichTextPaint::builder().background(RichTextBackgroundPaint::builder().metric_policy(RichTextBackgroundMetricPolicy::UniformTextStyle).build()).build();
    let content = result("abc", vec![cluster(text_range(0, 1), "a", 10.0), cluster(text_range(1, 2), "b", 10.0), cluster(text_range(2, 3), "c", 10.0)], vec![line(text_range(0, 3), IntRange::new(0, 2), 15.0, 0.0, 20.0, 30.0)], Vec::new(), vec![TextSpan { range: text_range(0, 2), style: TextStyle::builder().font_size(10.0).build() }, TextSpan { range: text_range(1, 3), style: TextStyle::builder().font_size(40.0).build() }], LayoutDebugInfo::default());
    assert_eq!(0.0, rich_text_background_segments(&content, &[segment(text_range(2, 3), RichTextRole::Background, paint, text_range(2, 3), 20.0, 30.0)]).remove(0).top);
}

#[test]
fn decoration_line_y_picks_the_last_matching_span() {
    let content = result("abc", vec![cluster(text_range(0, 1), "a", 10.0), cluster(text_range(1, 2), "b", 10.0), cluster(text_range(2, 3), "c", 10.0)], vec![line(text_range(0, 3), IntRange::new(0, 2), 15.0, 0.0, 20.0, 30.0)], Vec::new(), vec![TextSpan { range: text_range(0, 2), style: TextStyle::builder().font_size(10.0).build() }, TextSpan { range: text_range(1, 3), style: TextStyle::builder().font_size(20.0).build() }], LayoutDebugInfo::default());
    assert_eq!(18.6, rich_text_decoration_line_y(&content, &segment(text_range(2, 3), RichTextRole::Underline, RichTextPaint::default(), text_range(2, 3), 20.0, 30.0), 1.0));
}

#[test]
fn uniform_text_style_policy_keeps_the_earlier_span_when_a_later_one_misses() {
    let paint = RichTextPaint::builder().background(RichTextBackgroundPaint::builder().metric_policy(RichTextBackgroundMetricPolicy::UniformTextStyle).build()).build();
    let content = result("abc", vec![cluster(text_range(0, 1), "a", 10.0), cluster(text_range(1, 2), "b", 10.0), cluster(text_range(2, 3), "c", 10.0)], vec![line(text_range(0, 3), IntRange::new(0, 2), 15.0, 0.0, 20.0, 30.0)], Vec::new(), vec![TextSpan { range: text_range(0, 3), style: TextStyle::builder().font_size(40.0).build() }, TextSpan { range: text_range(1, 2), style: TextStyle::builder().font_size(10.0).build() }], LayoutDebugInfo::default());
    assert_eq!(0.0, rich_text_background_segments(&content, &[segment(text_range(0, 1), RichTextRole::Background, paint, text_range(0, 1), 0.0, 10.0)]).remove(0).top);
}

#[test]
fn decoration_line_y_keeps_the_earlier_span_when_a_later_one_misses() {
    let content = result("abc", vec![cluster(text_range(0, 1), "a", 10.0), cluster(text_range(1, 2), "b", 10.0), cluster(text_range(2, 3), "c", 10.0)], vec![line(text_range(0, 3), IntRange::new(0, 2), 15.0, 0.0, 20.0, 30.0)], Vec::new(), vec![TextSpan { range: text_range(0, 3), style: TextStyle::builder().font_size(20.0).build() }, TextSpan { range: text_range(1, 2), style: TextStyle::builder().font_size(10.0).build() }], LayoutDebugInfo::default());
    assert_eq!(18.6, rich_text_decoration_line_y(&content, &segment(text_range(0, 1), RichTextRole::Underline, RichTextPaint::default(), text_range(0, 1), 0.0, 10.0), 1.0));
}