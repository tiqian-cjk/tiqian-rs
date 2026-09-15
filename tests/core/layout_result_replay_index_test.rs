use tiqian::core::font_face::FontFaceId;
use tiqian::core::geometry::{scalar_offset, text_range, Rect};
use tiqian::core::int_range::IntRange;
use tiqian::core::layout_model::{FontDecisionInfo, Glyph, GlyphRun, LayoutDebugInfo};
use tiqian::core::layout_queries::PositionedCluster;
use tiqian::core::layout_result_replay_index::{
    cursor_rect, selection_boxes, selection_offset_for_position, selection_word_range_for_position,
    to_replay_index,
};
use tiqian::core::text::Text;
use tiqian::core::text_model::{RichTextLayer, RichTextLayerKind, RichTextSpan};

use super::layout_queries_test_support::{cluster, line, result, result_with_rich_text, word_boundary_result};

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

#[test]
fn replay_index_handles_empty_layout_and_word_selection_boundaries() {
    let empty = result("", Vec::new(), Vec::new(), Vec::new(), Vec::new(), LayoutDebugInfo::default());
    let empty_index = to_replay_index(&empty);
    assert_eq!(scalar_offset(0), selection_offset_for_position(&empty_index, &empty, 20.0, 20.0));
    assert_eq!(None, selection_word_range_for_position(&empty_index, &empty, 20.0, 20.0));
    assert_eq!(Rect { left: 0.0, top: 0.0, right: 0.0, bottom: 0.0 }, cursor_rect(&empty_index, &empty, scalar_offset(5)));
    assert!(selection_boxes(&empty_index, &empty, text_range(0, 1)).is_empty());

    let words = word_boundary_result();
    let word_index = to_replay_index(&words);
    assert_eq!(Some(text_range(2, 10)), selection_word_range_for_position(&word_index, &words, 45.0, 10.0));
}

#[test]
fn replay_index_builds_glyph_feature_and_font_role_evidence_per_cluster() {
    let clusters = vec![
        cluster(text_range(0, 1), "甲", 10.0),
        cluster(text_range(1, 2), "乙", 10.0),
        cluster(text_range(2, 3), "丙", 10.0),
    ];
    let result = result(
        "甲乙丙",
        clusters,
        vec![
            line(text_range(0, 2), IntRange::new(0, 1), 15.0, 0.0, 20.0, 20.0),
            line(text_range(2, 3), IntRange::new(2, 2), 35.0, 20.0, 40.0, 10.0),
        ],
        vec![GlyphRun::with_open_type_features(
            text_range(0, 2),
            FontFaceId::with_resource_id("test"),
            vec![
                Glyph::builder(1, text_range(0, 1), 10.0).build(),
                Glyph::builder(2, text_range(0, 1), 10.0).build(),
                Glyph::builder(3, text_range(1, 2), 10.0).build(),
            ],
            20.0,
            vec!["kern=1".to_owned()],
        )],
        Vec::new(),
        LayoutDebugInfo::builder()
            .font_decisions(vec![FontDecisionInfo {
                range: text_range(0, 2),
                source_text: Text::from("甲乙"),
                display_text: Text::from("甲乙"),
                role: "CjkText".to_owned(),
                candidate_key: "test".to_owned(),
                resolved_face: Some(FontFaceId::with_resource_id("test")),
                reason: "test".to_owned(),
                substitution_reason: "test".to_owned(),
            }])
            .build(),
    );

    let index = to_replay_index(&result);
    assert_eq!(2, index.glyphs_by_cluster_range[&text_range(0, 1)].len());
    assert_eq!(vec!["kern=1"], index.open_type_features_by_cluster_range[&text_range(1, 2)]);
    assert_eq!(Vec::<String>::new(), index.open_type_features_by_cluster_range[&text_range(2, 3)]);
    assert_eq!(Some("CjkText".to_owned()), index.font_role_by_cluster_range[&text_range(0, 1)]);
    assert_eq!(None, index.font_role_by_cluster_range[&text_range(2, 3)]);
    assert_eq!(2, selection_boxes(&index, &result, text_range(1, 3)).len());
    assert_eq!(scalar_offset(0), selection_offset_for_position(&index, &result, -1.0, 10.0));
    assert_eq!(scalar_offset(3), selection_offset_for_position(&index, &result, 50.0, 30.0));
}

#[test]
fn replay_index_uses_line_indent_for_empty_lines_and_nearest_cluster_for_gaps() {
    let empty_line = result(
        "甲",
        Vec::new(),
        vec![line(text_range(0, 1), IntRange::new(0, -1), 15.0, 0.0, 20.0, 10.0)],
        Vec::new(),
        Vec::new(),
        LayoutDebugInfo::default(),
    );
    let empty_index = to_replay_index(&empty_line);
    assert_eq!(scalar_offset(0), selection_offset_for_position(&empty_index, &empty_line, 5.0, 10.0));
    assert_eq!(None, selection_word_range_for_position(&empty_index, &empty_line, 5.0, 10.0));
    assert_eq!(0.0, cursor_rect(&empty_index, &empty_line, scalar_offset(0)).left);

    let gapped = result(
        "甲乙",
        vec![cluster(text_range(0, 1), "甲", 10.0), cluster(text_range(1, 2), "乙", 10.0)],
        vec![line(text_range(0, 2), IntRange::new(0, 1), 15.0, 0.0, 20.0, 20.0)],
        Vec::new(),
        Vec::new(),
        LayoutDebugInfo::default(),
    );
    let index = to_replay_index(&gapped);
    assert_eq!(scalar_offset(1), selection_offset_for_position(&index, &gapped, 10.0, 10.0));
    assert_eq!(Rect { left: 10.0, top: 0.0, right: 11.0, bottom: 20.0 }, cursor_rect(&index, &gapped, scalar_offset(1)));
}

#[test]
fn replay_index_selects_the_nearest_source_boundary_inside_visible_cluster_gaps() {
    let result = result(
        "甲·乙",
        vec![cluster(text_range(0, 1), "甲", 10.0), cluster(text_range(2, 3), "乙", 10.0)],
        vec![line(text_range(0, 3), IntRange::new(0, 1), 15.0, 0.0, 20.0, 30.0)],
        Vec::new(),
        Vec::new(),
        LayoutDebugInfo::default(),
    );
    let mut index = to_replay_index(&result);
    let first = PositionedCluster::builder(0, 0, text_range(0, 1), 0.0, 0.0, 10.0, 20.0, 15.0).build();
    let second = PositionedCluster::builder(0, 1, text_range(2, 3), 20.0, 0.0, 30.0, 20.0, 15.0).build();
    index.positioned_clusters = vec![first.clone(), second.clone()];
    index.positioned_clusters_by_line = vec![vec![first, second]];

    assert_eq!(scalar_offset(1), selection_offset_for_position(&index, &result, 12.0, 10.0));
    assert_eq!(scalar_offset(2), selection_offset_for_position(&index, &result, 18.0, 10.0));
}