use tiqian::core::geometry::{text_range, LayoutConstraints, Size, TextRange};
use tiqian::core::int_range::IntRange;
use tiqian::core::layout_model::{
    Cluster, DecorationDecisionInfo, Glyph, GlyphRun, LayoutDebugInfo, LayoutResult, LineBox,
    ShapingDecisionInfo,
};
use tiqian::core::text::Text;
use tiqian::core::text_model::{InlineBoxSpan, LayoutInput, TextSpan, TextStyle, TiqianTextContent};
use tiqian::layout::prepared_paragraph::{ecma_json_number, to_prepared_paragraph_json};

fn line(range: TextRange, clusters: IntRange, width: f32) -> LineBox {
    LineBox::builder(range, clusters, 20.0, 0.0, 24.0, width, width, width).build()
}

#[test]
fn style_at_and_style_deltas_in_prepared_paragraph_json() {
    let input = LayoutInput::builder(
        TiqianTextContent::builder(Text::from("甲乙丙"))
            .spans(vec![
                TextSpan { range: text_range(1, 3), style: TextStyle::builder().font_size(20.0).font_weight(700).italic(true).build() },
                TextSpan { range: text_range(2, 3), style: TextStyle::builder().font_weight(700).build() },
            ])
            .build(),
        LayoutConstraints::with_defaults(200.0),
    )
    .build();
    let clusters = vec![
        Cluster::new(text_range(0, 1), Text::from("甲"), "k".into(), 16.0),
        Cluster::new(text_range(1, 2), Text::from("乙"), "k".into(), 20.0),
        Cluster::new(text_range(2, 3), Text::from("丙"), "k".into(), 16.0),
    ];
    let glyphs = (0..3).map(|index| Glyph::builder(index + 1, text_range(index as i32, index as i32 + 1), clusters[index as usize].advance).build()).collect();
    let result = LayoutResult::new(input, Size { width: 200.0, height: 24.0 }, clusters, vec![GlyphRun::with_open_type_features(text_range(0, 3), "k".into(), glyphs, 52.0, vec!["liga".into(), "dlig".into()])], vec![line(text_range(0, 3), IntRange::new(0, 2), 52.0)]);
    let json = to_prepared_paragraph_json(&result, true);
    assert!(json.contains("\"openTypeFeatures\":[\"liga\",\"dlig\"]"), "{json}");
    assert!(json.contains("\"style\":{\"fontSize\":20,\"fontWeight\":700,\"italic\":true}"), "{json}");
    assert!(json.contains("\"style\":{\"fontWeight\":700}"), "{json}");
}

#[test]
fn inline_box_edges_and_emphasis_dots_filter() {
    let input = LayoutInput::builder(TiqianTextContent::new(Text::from("甲乙")), LayoutConstraints::with_defaults(200.0))
        .inline_boxes(vec![InlineBoxSpan::with_edges(text_range(0, 1), 4.0, 0.0), InlineBoxSpan::with_edges(text_range(1, 2), 0.0, 6.0)])
        .build();
    let clusters = vec![Cluster::new(text_range(0, 1), Text::from("甲"), "k".into(), 16.0), Cluster::new(text_range(1, 2), Text::from("乙"), "k".into(), 16.0)];
    let glyphs = vec![Glyph::builder(1, text_range(0, 1), 16.0).build(), Glyph::builder(2, text_range(1, 2), 16.0).build()];
    let debug = LayoutDebugInfo::builder().decoration_decisions(vec![
        DecorationDecisionInfo::builder(text_range(0, 1), Text::from("甲"), "Emphasis".into(), false, "skip".into()).dot_diameter(4.0).build(),
        DecorationDecisionInfo::builder(text_range(0, 1), Text::from("甲"), "ProperNoun".into(), true, "skip".into()).dot_diameter(4.0).build(),
        DecorationDecisionInfo::builder(text_range(0, 1), Text::from("甲"), "Emphasis".into(), true, "skip".into()).build(),
        DecorationDecisionInfo::builder(text_range(0, 1), Text::from("甲"), "Emphasis".into(), true, "dot".into()).anchor_x(8.0).anchor_y(20.0).dot_diameter(4.0).build(),
    ]).build();
    let result = LayoutResult::with_debug(input, Size { width: 200.0, height: 24.0 }, clusters, vec![GlyphRun::new(text_range(0, 2), "k".into(), glyphs, 32.0)], vec![line(text_range(0, 2), IntRange::new(0, 1), 32.0)], debug);
    let json = to_prepared_paragraph_json(&result, true);
    assert!(json.contains("\"inlineStart\":4"), "{json}");
    assert!(json.contains("\"inlineEnd\":6"), "{json}");
    assert_eq!(1, json.matches("\"emphasisDots\"").count(), "{json}");
}

#[test]
fn dash_shaping_decision_with_glyph_ids() {
    let input = LayoutInput::builder(TiqianTextContent::new(Text::from("——")), LayoutConstraints::with_defaults(200.0)).build();
    let cluster = Cluster::new(text_range(0, 2), Text::from("——"), "k".into(), 32.0);
    let decision = ShapingDecisionInfo::builder(text_range(0, 2), Text::from("——"), Text::from("——"), "k".into(), 2, 32.0, "test".into(), "DashRule".into()).language(Some("zh".into())).resolved_face(Some("NotoSansCJK".into())).strategy(Some("DashTwoEmLigature".into())).build();
    let result = LayoutResult::with_debug(input, Size { width: 200.0, height: 24.0 }, vec![cluster], vec![GlyphRun::new(text_range(0, 2), "k".into(), vec![Glyph::builder(42, text_range(0, 2), 16.0).build(), Glyph::builder(43, text_range(0, 2), 16.0).build()], 32.0)], vec![line(text_range(0, 2), IntRange::new(0, 0), 32.0)], LayoutDebugInfo::builder().shaping_decisions(vec![decision]).build());
    let json = to_prepared_paragraph_json(&result, true);
    for expected in ["\"glyphIds\":\"42,43\"", "\"shapingLanguage\":\"zh\"", "\"resolvedFace\":\"NotoSansCJK\"", "\"naturalWidth\":32"] { assert!(json.contains(expected), "missing {expected}: {json}"); }
}

#[test]
fn ecma_json_number_edge_cases() {
    assert!(!ecma_json_number(f32::from_bits(1)).is_empty());
    assert!(!ecma_json_number(f32::from_bits(0x007f_ffff)).is_empty());
    assert_eq!("8.999999688540309e-17", ecma_json_number(9.000000000000001e-17));
    for index in 1..=2000 { assert!(!ecma_json_number(index as f32 * 1e-17).is_empty()); assert!(!ecma_json_number(index as f32 * 1e-15).is_empty()); assert!(!ecma_json_number(index as f32 * 1e-20).is_empty()); }
    for shift in 1..=60 { let value = (1_i64 << shift) as f32; assert!(!ecma_json_number(value).is_empty()); assert!(!ecma_json_number(-value).is_empty()); assert!(!ecma_json_number(1.0 / value).is_empty()); assert!(!ecma_json_number(-1.0 / value).is_empty()); }
}
