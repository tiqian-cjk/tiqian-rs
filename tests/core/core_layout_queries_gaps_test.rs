use tiqian::core::geometry::{LayoutConstraints, Rect, Size, TextRange};
use tiqian::core::int_range::IntRange;
use tiqian::core::layout_model::{Cluster, Glyph, GlyphRun, LayoutDebugInfo, LayoutResult, LineBox};
use tiqian::core::layout_queries::{
    get_bounding_boxes, get_bounding_boxes_from_offsets, get_cursor_rect, get_line_for_offset,
    get_offset_for_position, get_selection_offset_for_position, get_selection_word_boundary,
    positioned_clusters, positioned_rich_text_segments, rich_text_background_segments,
};
use tiqian::core::text::Text;
use tiqian::core::text_model::{
    RichTextBackgroundMetricPolicy, RichTextBackgroundPaint, RichTextPaint, RichTextRole,
    RichTextSpan, TextStyle, TiqianTextContent, LayoutInput,
};

fn input(text: &str, max_width: f32) -> LayoutInput {
    LayoutInput::builder(TiqianTextContent::new(Text::from(text)), LayoutConstraints::with_defaults(max_width))
        .text_style(TextStyle::builder().font_size(10.0).build())
        .build()
}

fn line(range: TextRange, cluster_range: IntRange, baseline: f32, top: f32, bottom: f32, width: f32) -> LineBox {
    LineBox::builder(range, cluster_range, baseline, top, bottom, width, width, width).build()
}

fn sample_result() -> LayoutResult {
    LayoutResult::new(
        input("甲——乙", 40.0), Size { width: 34.0, height: 40.0 },
        vec![
            Cluster::new(TextRange::new(0, 1), Text::from("甲"), "cjk".to_owned(), 10.0),
            Cluster::with_display_text(TextRange::new(1, 3), Text::from("——"), Text::from("⸺"), "cjk".to_owned(), 20.0),
            Cluster::new(TextRange::new(3, 4), Text::from("乙"), "cjk".to_owned(), 10.0),
        ], Vec::new(),
        vec![
            LineBox::builder(TextRange::new(0, 3), IntRange::new(0, 1), 15.0, 0.0, 20.0, 30.0, 30.0, 30.0).indent(4.0).build(),
            line(TextRange::new(3, 4), IntRange::new(2, 2), 35.0, 20.0, 40.0, 10.0),
        ],
    )
}

fn latin_result() -> LayoutResult {
    LayoutResult::new(
        input("AB", 100.0), Size { width: 20.0, height: 20.0 },
        vec![
            Cluster::new(TextRange::new(0, 1), Text::from("A"), "latin".to_owned(), 10.0),
            Cluster::new(TextRange::new(1, 2), Text::from("B"), "latin".to_owned(), 10.0),
        ], Vec::new(), vec![line(TextRange::new(0, 2), IntRange::new(0, 1), 15.0, 0.0, 20.0, 20.0)],
    )
}

#[test]
fn positioned_cluster_height_returns_difference() {
    assert_eq!(20.0, positioned_clusters(&sample_result())[0].height());
}

#[test]
fn get_line_for_offset_uses_nearest_line_when_gap_between_lines() {
    let result = LayoutResult::new(input("abcde", 100.0), Size { width: 10.0, height: 40.0 }, vec![
        Cluster::new(TextRange::new(0, 1), Text::from("a"), "cjk".to_owned(), 10.0),
        Cluster::new(TextRange::new(1, 2), Text::from("b"), "cjk".to_owned(), 10.0),
        Cluster::new(TextRange::new(2, 3), Text::from("c"), "cjk".to_owned(), 10.0),
        Cluster::new(TextRange::new(4, 5), Text::from("e"), "cjk".to_owned(), 10.0),
    ], Vec::new(), vec![line(TextRange::new(0, 2), IntRange::new(0, 1), 15.0, 0.0, 20.0, 20.0), line(TextRange::new(4, 5), IntRange::new(2, 3), 35.0, 25.0, 45.0, 10.0)]);
    assert_eq!(0, get_line_for_offset(&result, 3));
}

#[test]
fn get_bounding_boxes_int_delegates_to_text_range() {
    let result = sample_result();
    assert_eq!(get_bounding_boxes(&result, TextRange::new(2, 4)), get_bounding_boxes_from_offsets(&result, 2, 4));
}

#[test]
fn rich_text_background_uses_horizontal_padding() {
    let result = latin_result();
    let span = RichTextSpan::with_paint(TextRange::new(0, 2), RichTextRole::Background, RichTextPaint::builder().background(RichTextBackgroundPaint::builder().horizontal_padding(5.0).build()).build());
    assert_eq!(1, rich_text_background_segments(&result, &positioned_rich_text_segments(&result, &[span])).len());
}

#[test]
fn rich_text_background_trailing_padding_when_span_ends_at_segment_end() {
    let result = latin_result();
    let span = RichTextSpan::with_paint(TextRange::new(0, 2), RichTextRole::Background, RichTextPaint::builder().background(RichTextBackgroundPaint::builder().horizontal_padding(5.0).build()).build());
    let segments = rich_text_background_segments(&result, &positioned_rich_text_segments(&result, &[span]));
    assert_eq!(1, segments.len());
    assert!(segments[0].right > 15.0);
}

#[test]
fn rich_text_background_uniform_paragraph_style_uses_paragraph_style() {
    let result = latin_result();
    let span = RichTextSpan::with_paint(TextRange::new(0, 2), RichTextRole::Background, RichTextPaint::builder().background(RichTextBackgroundPaint::builder().metric_policy(RichTextBackgroundMetricPolicy::UniformParagraphStyle).build()).build());
    assert_eq!(1, rich_text_background_segments(&result, &positioned_rich_text_segments(&result, &[span])).len());
}

#[test]
fn marked_face_vertical_bounds_uses_fallback_when_no_metric_matches() {
    let result = LayoutResult::with_debug(input("AB", 100.0), Size { width: 20.0, height: 20.0 }, vec![Cluster::new(TextRange::new(0, 2), Text::from("AB"), "latin".to_owned(), 20.0)], Vec::new(), vec![line(TextRange::new(0, 2), IntRange::new(0, 0), 15.0, 0.0, 20.0, 20.0)], LayoutDebugInfo::default());
    let span = RichTextSpan::new(TextRange::new(0, 2), RichTextRole::Background);
    assert_eq!(1, rich_text_background_segments(&result, &positioned_rich_text_segments(&result, &[span])).len());
}

#[test]
fn get_selection_offset_for_position_returns_nearest_when_before_first_cluster() {
    assert_eq!(0, get_selection_offset_for_position(&sample_result(), 3.0, 5.0));
}

#[test]
fn get_selection_offset_for_position_returns_nearest_when_after_last_cluster() {
    assert_eq!(4, get_selection_offset_for_position(&sample_result(), 35.0, 25.0));
}

#[test]
fn get_selection_offset_for_position_returns_start_of_line_when_clusters_empty() {
    let result = LayoutResult::new(input("", 100.0), Size { width: 0.0, height: 20.0 }, Vec::new(), Vec::new(), vec![line(TextRange::new(0, 0), IntRange::new(0, -1), 15.0, 0.0, 20.0, 0.0)]);
    assert_eq!(0, get_selection_offset_for_position(&result, 5.0, 10.0));
}

#[test]
fn get_selection_word_boundary_for_emoji_zwj_sequence() {
    let result = LayoutResult::new(input("👩‍👩", 100.0), Size { width: 50.0, height: 20.0 }, vec![Cluster::new(TextRange::new(0, 5), Text::from("👩‍👩"), "emoji".to_owned(), 50.0)], Vec::new(), vec![line(TextRange::new(0, 5), IntRange::new(0, 0), 15.0, 0.0, 20.0, 50.0)]);
    assert_eq!(TextRange::new(0, 5), get_selection_word_boundary(&result, 5));
}

#[test]
fn get_selection_word_boundary_for_punctuation_returns_single() {
    let result = LayoutResult::new(input("A,B", 100.0), Size { width: 30.0, height: 20.0 }, vec![
        Cluster::new(TextRange::new(0, 1), Text::from("A"), "latin".to_owned(), 10.0), Cluster::new(TextRange::new(1, 2), Text::from(","), "latin".to_owned(), 10.0), Cluster::new(TextRange::new(2, 3), Text::from("B"), "latin".to_owned(), 10.0),
    ], Vec::new(), vec![line(TextRange::new(0, 3), IntRange::new(0, 2), 15.0, 0.0, 20.0, 30.0)]);
    assert_eq!(TextRange::new(1, 2), get_selection_word_boundary(&result, 1));
}

#[test]
fn positioned_clusters_produces_source_stops_for_latin_run() {
    let result = latin_with_glyphs(false);
    let positioned = positioned_clusters(&result);
    assert_eq!(3, positioned[0].source_stops.as_ref().unwrap().len());
}

#[test]
fn offset_for_x_uses_source_stops_when_available() {
    let result = latin_with_glyphs(true);
    let positioned = positioned_clusters(&result);
    assert_eq!(0, get_offset_for_position(&result, positioned[0].left, 10.0));
    assert_eq!(1, get_offset_for_position(&result, 15.0, 10.0));
}

#[test]
fn get_bounding_boxes_empty_range_returns_empty_list() {
    assert!(get_bounding_boxes(&sample_result(), TextRange::new(2, 2)).is_empty());
}

#[test]
fn get_line_for_offset_returns_nearest_line() {
    let result = LayoutResult::new(input("abc", 100.0), Size { width: 30.0, height: 40.0 }, vec![
        Cluster::new(TextRange::new(0, 1), Text::from("a"), "cjk".to_owned(), 10.0), Cluster::new(TextRange::new(1, 2), Text::from("b"), "cjk".to_owned(), 10.0), Cluster::new(TextRange::new(2, 3), Text::from("c"), "cjk".to_owned(), 10.0),
    ], Vec::new(), vec![line(TextRange::new(0, 1), IntRange::new(0, 0), 15.0, 0.0, 20.0, 10.0), line(TextRange::new(1, 2), IntRange::new(1, 1), 35.0, 25.0, 45.0, 10.0)]);
    assert_eq!(1, get_line_for_offset(&result, 10));
}

#[test]
fn get_cursor_rect_returns_caret_in_cluster() {
    assert_eq!(Rect { left: 24.0, top: 0.0, right: 25.0, bottom: 20.0 }, get_cursor_rect(&sample_result(), 2));
}

#[test]
fn get_offset_for_position_uses_min_by_when_outside_clusters() {
    assert_eq!(1, get_offset_for_position(&sample_result(), 12.0, 5.0));
}

#[test]
fn get_selection_word_boundary_returns_empty_for_empty_text() {
    let result = LayoutResult::new(input("", 100.0), Size { width: 0.0, height: 20.0 }, Vec::new(), Vec::new(), Vec::new());
    assert_eq!(TextRange::new(0, 0), get_selection_word_boundary(&result, 0));
}

fn latin_with_glyphs(with_positions: bool) -> LayoutResult {
    let glyphs = if with_positions {
        vec![Glyph::builder(1, TextRange::new(0, 2), 10.0).x(5.0).build(), Glyph::builder(2, TextRange::new(0, 2), 10.0).x(15.0).build()]
    } else {
        vec![Glyph::builder(1, TextRange::new(0, 2), 10.0).build(), Glyph::builder(2, TextRange::new(0, 2), 10.0).build()]
    };
    LayoutResult::new(input("Hi", 100.0), Size { width: 20.0, height: 20.0 }, vec![Cluster::new(TextRange::new(0, 2), Text::from("Hi"), "latin".to_owned(), 20.0)], vec![GlyphRun::new(TextRange::new(0, 2), "latin".to_owned(), glyphs, 20.0)], vec![line(TextRange::new(0, 2), IntRange::new(0, 0), 15.0, 0.0, 20.0, 20.0)])
}