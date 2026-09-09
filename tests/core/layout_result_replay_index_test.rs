use tiqian::core::geometry::{text_range, Rect};
use tiqian::core::int_range::IntRange;
use tiqian::core::layout_model::LayoutDebugInfo;
use tiqian::core::layout_result_replay_index::{
    cursor_rect, selection_boxes, selection_offset_for_position, to_replay_index,
};
use tiqian::core::text_model::{RichTextLayer, RichTextLayerKind, RichTextSpan};

use super::layout_queries_test_support::{cluster, line, result_with_rich_text};

#[test]
fn replay_index_keeps_selection_and_rich_text_on_engine_geometry() {
    let rich_text = vec![RichTextSpan {
        range: text_range(1, 3),
        layers: vec![RichTextLayer {
            kind: RichTextLayerKind::Underline { line: Default::default() },
            paints: Vec::new(),
        }, RichTextLayer {
            kind: RichTextLayerKind::Background { background: Default::default() },
            paints: Vec::new(),
        }],
        semantics: Vec::new(),
    }];
    let result = result_with_rich_text(
        "甲乙丙",
        vec![
            cluster(text_range(0, 1), "甲", 10.0),
            cluster(text_range(1, 2), "乙", 10.0),
            cluster(text_range(2, 3), "丙", 10.0),
        ],
        vec![line(text_range(0, 3), IntRange::new(0, 2), 15.0, 0.0, 20.0, 30.0)],
        Vec::new(),
        Vec::new(),
        rich_text,
        LayoutDebugInfo::default(),
    );

    let index = to_replay_index(&result);

    assert_eq!(tiqian::core::layout_queries::positioned_clusters(&result), index.positioned_clusters);
    assert_eq!(result.positioned_rich_text_segments(), index.rich_text_segments);
    assert_eq!(result.rich_text_background_segments(), index.rich_text_background_segments);
    assert_eq!(result.rich_text_decoration_segments(), index.rich_text_decoration_segments);
    assert_eq!(1, index.rich_text_background_segments.len());
    assert_eq!(1, index.rich_text_decoration_segments.len());

    assert_eq!(3, index.positioned_clusters.len());
    assert_eq!(text_range(1, 3), index.rich_text_segments[0].range);
    assert_eq!(
        tiqian::core::geometry::scalar_offset(2),
        selection_offset_for_position(&index, &result, 19.0, 10.0),
    );
    assert_eq!(
        vec![Rect { left: 10.0, top: 0.0, right: 30.0, bottom: 20.0 }],
        selection_boxes(&index, &result, text_range(1, 3)),
    );
    assert_eq!(
        Rect { left: 20.0, top: 0.0, right: 21.0, bottom: 20.0 },
        cursor_rect(&index, &result, tiqian::core::geometry::scalar_offset(2)),
    );
}