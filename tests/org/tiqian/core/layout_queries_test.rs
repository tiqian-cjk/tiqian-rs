use tiqian::core::geometry::{LayoutConstraints, Rect, Size, TextRange};
use tiqian::core::int_range::IntRange;
use tiqian::core::layout_model::{
    AutoSpaceDecisionInfo, BopomofoDecisionInfo, Cluster, ClusterGeometryDecisionInfo, Glyph,
    GlyphRun, LayoutDebugInfo, LayoutResult, LineBox, RubyDecisionInfo,
};
use tiqian::core::layout_queries::{
    coerce_selection_offset, get_bounding_box, get_bounding_boxes, get_offset_for_position,
    get_selection_offset_for_position, get_text_for_copy, glyph_ink_bounds, positioned_clusters,
    positioned_rich_text_segments, trimmed_rich_text_decoration_segments,
};
use tiqian::core::source_interaction_boundaries::SourceBoundaryBias;
use tiqian::core::text::Text;
use tiqian::core::text_model::{
    InlineObjectSpan, LayoutInput, RichTextRole, RichTextSpan, TextStyle, TiqianTextContent,
};

fn layout_input(text: &str, max_width: f32) -> LayoutInput {
    LayoutInput::builder(
        TiqianTextContent::new(Text::from(text)),
        LayoutConstraints::with_defaults(max_width),
    )
    .text_style(TextStyle::builder().font_size(10.0).build())
    .build()
}

fn line(
    range: TextRange,
    cluster_range: IntRange,
    baseline: f32,
    top: f32,
    bottom: f32,
    width: f32,
) -> LineBox {
    LineBox::builder(
        range,
        cluster_range,
        baseline,
        top,
        bottom,
        width,
        width,
        width,
    )
    .build()
}

#[test]
fn clipboard_projection_restores_source_and_adds_fully_selected_annotations() {
    let debug = LayoutDebugInfo::builder()
        .ruby_decisions(vec![
            RubyDecisionInfo::builder(
                TextRange::new(0, 2),
                Text::from("tíqiàn"),
                0,
                0.0,
                0.0,
                8.0,
                0.0,
            )
            .build(),
        ])
        .bopomofo_decisions(vec![BopomofoDecisionInfo::new(
            TextRange::new(3, 4),
            Text::from("ㄋㄧㄣˊ"),
            0,
            Vec::new(),
        )])
        .build();
    let result = LayoutResult::with_debug(
        layout_input("提椠与您", 200.0),
        Size {
            width: 0.0,
            height: 0.0,
        },
        Vec::new(),
        Vec::new(),
        Vec::new(),
        debug,
    );

    assert_eq!(
        "提椠（tíqiàn）与您（ㄋㄧㄣˊ）",
        get_text_for_copy(&result, TextRange::new(0, 4)).as_str()
    );
    assert_eq!(
        "提",
        get_text_for_copy(&result, TextRange::new(0, 1)).as_str()
    );
    assert_eq!(
        "提椠（tíqiàn）",
        get_text_for_copy(&result, TextRange::new(0, 2)).as_str()
    );
    assert_eq!(
        "您（ㄋㄧㄣˊ）",
        get_text_for_copy(&result, TextRange::new(3, 4)).as_str()
    );
}

#[test]
fn positioned_clusters_separate_occupied_box_from_draw_origin() {
    let debug = LayoutDebugInfo::builder()
        .auto_space_decisions(vec![AutoSpaceDecisionInfo {
            cluster_range: TextRange::new(1, 3),
            side: "leading".to_owned(),
            boundary_role: "CjkLatin".to_owned(),
            mode: "Insert".to_owned(),
            characters_affected: 1,
            reduction_per_char: -2.5,
            total_reduction: -2.5,
            reason: "TextAutoSpaceInsert:ideograph-alpha:quarter-em".to_owned(),
        }])
        .build();
    let result = LayoutResult::with_debug(
        layout_input("中Hi", 40.0),
        Size {
            width: 32.5,
            height: 20.0,
        },
        vec![
            Cluster::new(
                TextRange::new(0, 1),
                Text::from("中"),
                "cjk".to_owned(),
                10.0,
            ),
            Cluster::new(
                TextRange::new(1, 3),
                Text::from("Hi"),
                "latin".to_owned(),
                22.5,
            ),
        ],
        Vec::new(),
        vec![line(
            TextRange::new(0, 3),
            IntRange::new(0, 1),
            15.0,
            0.0,
            20.0,
            32.5,
        )],
        debug,
    );

    let positioned = positioned_clusters(&result);
    assert_eq!(
        Rect {
            left: 10.0,
            top: 0.0,
            right: 32.5,
            bottom: 20.0
        },
        positioned[1].rect()
    );
    assert_eq!(12.5, positioned[1].draw_x);
    assert_eq!(
        Rect {
            left: 10.0,
            top: 0.0,
            right: 32.5,
            bottom: 20.0
        },
        get_bounding_box(&result, 1)
    );
    assert_eq!(1, get_offset_for_position(&result, 11.0, 5.0));
}

#[test]
fn glyph_ink_bounds_keep_overhang_separate_from_occupied_geometry() {
    let result = LayoutResult::new(
        layout_input("f", 10.0),
        Size {
            width: 10.0,
            height: 20.0,
        },
        vec![Cluster::new(
            TextRange::new(0, 1),
            Text::from("f"),
            "latin".to_owned(),
            10.0,
        )],
        vec![GlyphRun::new(
            TextRange::new(0, 1),
            "latin".to_owned(),
            vec![
                Glyph::builder(1, TextRange::new(0, 1), 10.0)
                    .bounds(Some(Rect {
                        left: -3.0,
                        top: -9.0,
                        right: 12.0,
                        bottom: 2.0,
                    }))
                    .build(),
            ],
            10.0,
        )],
        vec![line(
            TextRange::new(0, 1),
            IntRange::new(0, 0),
            14.0,
            0.0,
            20.0,
            10.0,
        )],
    );

    assert_eq!(
        Rect {
            left: 0.0,
            top: 0.0,
            right: 10.0,
            bottom: 20.0
        },
        positioned_clusters(&result)[0].rect()
    );
    assert_eq!(
        Some(Rect {
            left: -3.0,
            top: 5.0,
            right: 12.0,
            bottom: 16.0
        }),
        glyph_ink_bounds(&result)
    );
}

#[test]
fn range_boxes_split_multicode_unit_clusters_across_lines() {
    let result = sample_multiline_result();

    assert_eq!(
        vec![
            Rect {
                left: 24.0,
                top: 0.0,
                right: 34.0,
                bottom: 20.0
            },
            Rect {
                left: 0.0,
                top: 20.0,
                right: 10.0,
                bottom: 40.0
            },
        ],
        get_bounding_boxes(&result, TextRange::new(2, 4)),
    );
}

#[test]
fn selection_hit_testing_keeps_emoji_and_combining_sequences_atomic() {
    let result = LayoutResult::new(
        layout_input("😀é👩‍👩", 90.0),
        Size {
            width: 90.0,
            height: 20.0,
        },
        vec![
            Cluster::new(
                TextRange::new(0, 2),
                Text::from("😀"),
                "emoji".to_owned(),
                20.0,
            ),
            Cluster::new(
                TextRange::new(2, 4),
                Text::from("é"),
                "latin".to_owned(),
                20.0,
            ),
            Cluster::new(
                TextRange::new(4, 9),
                Text::from("👩‍👩"),
                "emoji".to_owned(),
                50.0,
            ),
        ],
        Vec::new(),
        vec![line(
            TextRange::new(0, 9),
            IntRange::new(0, 2),
            15.0,
            0.0,
            20.0,
            90.0,
        )],
    );

    for (x, expected) in [
        (5.0, 0),
        (15.0, 2),
        (25.0, 2),
        (35.0, 4),
        (45.0, 4),
        (75.0, 9),
    ] {
        assert_eq!(
            expected,
            get_selection_offset_for_position(&result, x, 10.0)
        );
    }
    assert_eq!(
        2,
        coerce_selection_offset(&result, 3, SourceBoundaryBias::Backward)
    );
    assert_eq!(
        4,
        coerce_selection_offset(&result, 3, SourceBoundaryBias::Forward)
    );
}

#[test]
fn inline_object_is_a_single_selection_unit() {
    let source = "a\\operatorname{lim}b";
    let object_range = TextRange::new(1, source.encode_utf16().count() as i32 - 1);
    let input = LayoutInput::builder(
        TiqianTextContent::new(Text::from(source)),
        LayoutConstraints::with_defaults(200.0),
    )
    .inline_objects(vec![InlineObjectSpan::with_fixed_boundaries(
        object_range,
        40.0,
        12.0,
        4.0,
    )])
    .build();
    let result = LayoutResult::new(
        input,
        Size {
            width: 60.0,
            height: 20.0,
        },
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );

    assert_eq!(
        1,
        coerce_selection_offset(&result, 5, SourceBoundaryBias::Backward)
    );
    assert_eq!(
        object_range.end(),
        coerce_selection_offset(&result, 5, SourceBoundaryBias::Forward)
    );
    assert_eq!(
        object_range.end(),
        coerce_selection_offset(&result, object_range.end() - 1, SourceBoundaryBias::Nearest)
    );
}

#[test]
fn rich_text_decoration_trims_only_outer_punctuation_glue() {
    let debug = LayoutDebugInfo::builder()
        .geometry_decisions(vec![
            punctuation_geometry(TextRange::new(0, 1), "（", 5.0, 0.0),
            punctuation_geometry(TextRange::new(1, 2), "，", 0.0, 5.0),
            punctuation_geometry(TextRange::new(2, 3), "中", 0.0, 0.0),
            punctuation_geometry(TextRange::new(3, 4), "）", 0.0, 5.0),
        ])
        .build();
    let result = LayoutResult::with_debug(
        layout_input("（，中）", 40.0),
        Size {
            width: 40.0,
            height: 20.0,
        },
        vec![
            Cluster::new(
                TextRange::new(0, 1),
                Text::from("（"),
                "cjk".to_owned(),
                10.0,
            ),
            Cluster::new(
                TextRange::new(1, 2),
                Text::from("，"),
                "cjk".to_owned(),
                10.0,
            ),
            Cluster::new(
                TextRange::new(2, 3),
                Text::from("中"),
                "cjk".to_owned(),
                10.0,
            ),
            Cluster::new(
                TextRange::new(3, 4),
                Text::from("）"),
                "cjk".to_owned(),
                10.0,
            ),
        ],
        Vec::new(),
        vec![line(
            TextRange::new(0, 4),
            IntRange::new(0, 3),
            15.0,
            0.0,
            20.0,
            40.0,
        )],
        debug,
    );
    let underline = RichTextSpan::new(TextRange::new(0, 4), RichTextRole::Underline);
    let occupied = positioned_rich_text_segments(&result, &[underline]);
    let decoration = trimmed_rich_text_decoration_segments(&result, &occupied);

    assert_eq!(
        Rect {
            left: 0.0,
            top: 0.0,
            right: 40.0,
            bottom: 20.0
        },
        occupied[0].rect()
    );
    assert_eq!(
        Rect {
            left: 5.0,
            top: 0.0,
            right: 35.0,
            bottom: 20.0
        },
        decoration[0].rect()
    );
}

fn sample_multiline_result() -> LayoutResult {
    LayoutResult::new(
        layout_input("甲——乙", 40.0),
        Size {
            width: 34.0,
            height: 40.0,
        },
        vec![
            Cluster::new(
                TextRange::new(0, 1),
                Text::from("甲"),
                "cjk".to_owned(),
                10.0,
            ),
            Cluster::with_display_text(
                TextRange::new(1, 3),
                Text::from("——"),
                Text::from("⸺"),
                "cjk".to_owned(),
                20.0,
            ),
            Cluster::new(
                TextRange::new(3, 4),
                Text::from("乙"),
                "cjk".to_owned(),
                10.0,
            ),
        ],
        Vec::new(),
        vec![
            LineBox::builder(
                TextRange::new(0, 3),
                IntRange::new(0, 1),
                15.0,
                0.0,
                20.0,
                30.0,
                30.0,
                30.0,
            )
            .indent(4.0)
            .build(),
            line(
                TextRange::new(3, 4),
                IntRange::new(2, 2),
                35.0,
                20.0,
                40.0,
                10.0,
            ),
        ],
    )
}

fn punctuation_geometry(
    range: TextRange,
    text: &str,
    leading_glue_natural: f32,
    trailing_glue_natural: f32,
) -> ClusterGeometryDecisionInfo {
    ClusterGeometryDecisionInfo::builder(
        range,
        Text::from(text),
        Text::from(text),
        10.0,
        10.0 - leading_glue_natural - trailing_glue_natural,
        leading_glue_natural,
        0.0,
        trailing_glue_natural,
        0.0,
        0.0,
        10.0,
        "test".to_owned(),
        "PunctuationGlueTest".to_owned(),
    )
    .build()
}
