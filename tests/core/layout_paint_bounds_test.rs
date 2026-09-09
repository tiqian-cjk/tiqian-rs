use tiqian::core::geometry::{text_range, Rect};
use tiqian::core::int_range::IntRange;
use tiqian::core::layout_model::{
    DecorationDecisionInfo, Glyph, GlyphRun, LayoutDebugInfo, RubyDecisionInfo,
};
use tiqian::core::layout_paint_bounds::{
    LayoutPaintOverhang, legal_hanging_punctuation_clip_edge, visible_paint_overhang,
};
use tiqian::core::layout_queries::positioned_clusters;
use tiqian::core::text::Text;

use super::layout_queries_test_support::{cluster, line, result};

#[test]
fn visible_paint_overhang_includes_visible_glyph_dot_and_ruby_paint() {
    let result = result(
        "甲",
        vec![cluster(text_range(0, 1), "甲", 10.0)],
        vec![line(text_range(0, 1), IntRange::new(0, 0), 15.0, 0.0, 20.0, 10.0)],
        vec![GlyphRun::new(
            text_range(0, 1),
            "cjk".to_owned(),
            vec![Glyph::builder(1, text_range(0, 1), 10.0)
                .bounds(Some(Rect { left: -2.0, top: -18.0, right: 12.0, bottom: 5.0 }))
                .build()],
            10.0,
        )],
        Vec::new(),
        LayoutDebugInfo::builder()
            .decoration_decisions(vec![
                DecorationDecisionInfo::builder(
                    text_range(0, 1),
                    Text::from("甲"),
                    "Emphasis".to_owned(),
                    true,
                    "test".to_owned(),
                )
                .anchor_x(5.0)
                .anchor_y(-2.0)
                .dot_diameter(2.0)
                .build(),
            ])
            .ruby_decisions(vec![
                RubyDecisionInfo::builder(text_range(0, 1), Text::from("jiǎ"), 0, 5.0, -3.0, 5.0, 0.0)
                    .ascent(5.0)
                    .descent(2.0)
                    .width(16.0)
                    .build(),
            ])
            .build(),
    );

    assert_eq!(
        LayoutPaintOverhang { left: 0.0, top: 8.0, right: 0.0, bottom: 0.0 },
        visible_paint_overhang(&result, 100.0, 100.0, Some(&positioned_clusters(&result))),
    );
}

#[test]
fn offscreen_occupied_geometry_does_not_expand_the_paint_clip() {
    let result = result(
        "甲",
        vec![cluster(text_range(0, 1), "甲", 10.0)],
        vec![line(text_range(0, 1), IntRange::new(0, 0), 45.0, 30.0, 50.0, 10.0)],
        vec![GlyphRun::new(
            text_range(0, 1),
            "cjk".to_owned(),
            vec![Glyph::builder(1, text_range(0, 1), 10.0)
                .bounds(Some(Rect { left: -10.0, top: -30.0, right: 20.0, bottom: 10.0 }))
                .build()],
            10.0,
        )],
        Vec::new(),
        LayoutDebugInfo::default(),
    );

    assert_eq!(
        LayoutPaintOverhang::default(),
        visible_paint_overhang(&result, 100.0, 20.0, None),
    );
}

#[test]
fn hanging_clip_edge_only_expands_for_an_engine_selected_hang() {
    let hanging = tiqian::core::layout_model::LineBox::builder(
        text_range(0, 1),
        IntRange::new(0, 0),
        15.0,
        0.0,
        20.0,
        10.0,
        10.0,
        12.0,
    )
    .indent(6.0)
    .hanging_punctuation_advance(2.0)
    .build();
    let ordinary = line(text_range(0, 1), IntRange::new(0, 0), 15.0, 0.0, 20.0, 12.0);

    assert_eq!(18.0, legal_hanging_punctuation_clip_edge(&hanging, 100.0));
    assert_eq!(100.0, legal_hanging_punctuation_clip_edge(&ordinary, 100.0));
}