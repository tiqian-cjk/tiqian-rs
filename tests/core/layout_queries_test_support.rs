use tiqian::core::geometry::{LayoutConstraints, Size, TextRange};
use tiqian::core::int_range::IntRange;
use tiqian::core::layout_model::{
    AutoSpaceDecisionInfo, Cluster, ClusterGeometryDecisionInfo, Glyph, GlyphRun,
    LayoutDebugInfo, LayoutResult, LineBox, MetricDecisionInfo, RubyDecisionInfo,
};
use tiqian::core::text::Text;
use tiqian::core::text_model::{
    LayoutInput, TextSpan, TextStyle, TiqianTextContent,
};

pub fn input(text: &str, max_width: f32) -> LayoutInput {
    LayoutInput::builder(
        TiqianTextContent::new(Text::from(text)),
        LayoutConstraints::with_defaults(max_width),
    )
    .text_style(TextStyle::builder().font_size(10.0).build())
    .build()
}

pub fn line(
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

pub fn cluster(range: TextRange, text: &str, advance: f32) -> Cluster {
    Cluster::new(range, Text::from(text), "test".to_owned(), advance)
}

pub fn result(
    text: &str,
    clusters: Vec<Cluster>,
    lines: Vec<LineBox>,
    glyph_runs: Vec<GlyphRun>,
    spans: Vec<TextSpan>,
    debug: LayoutDebugInfo,
) -> LayoutResult {
    let content = TiqianTextContent::builder(Text::from(text)).spans(spans).build();
    LayoutResult::with_debug(
        LayoutInput::builder(content, LayoutConstraints::with_defaults(100.0))
            .text_style(TextStyle::builder().font_size(10.0).build())
            .build(),
        Size {
            width: 120.0,
            height: 40.0,
        },
        clusters,
        glyph_runs,
        lines,
        debug,
    )
}

pub fn metric(range: TextRange, ascent: f32, descent: f32, metric_box: &str) -> MetricDecisionInfo {
    MetricDecisionInfo {
        range,
        source_text: Text::from("test"),
        role: "test".to_owned(),
        font_key: "test".to_owned(),
        raw_ascent: ascent,
        raw_descent: descent,
        raw_leading: 0.0,
        raw_source: "test".to_owned(),
        layout_ascent: ascent,
        layout_descent: descent,
        baseline_class: "test".to_owned(),
        metric_box: metric_box.to_owned(),
        layout_source: "test".to_owned(),
        reason: "test".to_owned(),
    }
}

pub fn punctuation_geometry(
    range: TextRange,
    text: &str,
    leading_glue: f32,
    trailing_glue: f32,
    leading_consumed: f32,
) -> ClusterGeometryDecisionInfo {
    ClusterGeometryDecisionInfo::builder(
        range,
        Text::from(text),
        Text::from(text),
        10.0,
        10.0 - leading_glue - trailing_glue,
        leading_glue,
        leading_consumed,
        trailing_glue,
        0.0,
        0.0,
        10.0,
        "test".to_owned(),
        "PunctuationGlueTest".to_owned(),
    )
    .build()
}

pub fn sample_result() -> LayoutResult {
    LayoutResult::new(
        input("甲——乙", 40.0),
        Size { width: 34.0, height: 40.0 },
        vec![
            cluster(TextRange::new(0, 1), "甲", 10.0),
            Cluster::with_display_text(
                TextRange::new(1, 3),
                Text::from("——"),
                Text::from("⸺"),
                "cjk".to_owned(),
                20.0,
            ),
            cluster(TextRange::new(3, 4), "乙", 10.0),
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
            line(TextRange::new(3, 4), IntRange::new(2, 2), 35.0, 20.0, 40.0, 10.0),
        ],
    )
}

pub fn punctuation_glue_result(leading_consumed: f32) -> LayoutResult {
    result(
        "（，中）",
        vec![
            cluster(TextRange::new(0, 1), "（", 10.0),
            cluster(TextRange::new(1, 2), "，", 10.0),
            cluster(TextRange::new(2, 3), "中", 10.0),
            cluster(TextRange::new(3, 4), "）", 10.0),
        ],
        vec![line(TextRange::new(0, 4), IntRange::new(0, 3), 15.0, 0.0, 20.0, 40.0)],
        Vec::new(),
        Vec::new(),
        LayoutDebugInfo::builder()
            .geometry_decisions(vec![
                punctuation_geometry(TextRange::new(0, 1), "（", 5.0, 0.0, leading_consumed),
                punctuation_geometry(TextRange::new(1, 2), "，", 0.0, 5.0, 0.0),
                punctuation_geometry(TextRange::new(2, 3), "中", 0.0, 0.0, 0.0),
                punctuation_geometry(TextRange::new(3, 4), "）", 0.0, 5.0, 0.0),
            ])
            .build(),
    )
}

pub fn background_geometry_result(metrics: Vec<MetricDecisionInfo>) -> LayoutResult {
    result(
        "A B",
        vec![
            cluster(TextRange::new(0, 1), "A", 12.0),
            cluster(TextRange::new(1, 2), " ", 5.0),
            cluster(TextRange::new(2, 3), "B", 14.0),
        ],
        vec![line(TextRange::new(0, 3), IntRange::new(0, 2), 20.0, 0.0, 30.0, 31.0)],
        vec![
            GlyphRun::new(TextRange::new(0, 1), "latin".to_owned(), vec![Glyph::builder(1, TextRange::new(0, 1), 10.0).build()], 10.0),
            GlyphRun::new(TextRange::new(2, 3), "latin".to_owned(), vec![Glyph::builder(2, TextRange::new(2, 3), 10.0).build()], 10.0),
        ],
        Vec::new(),
        LayoutDebugInfo::builder()
            .auto_space_decisions(vec![AutoSpaceDecisionInfo {
                cluster_range: TextRange::new(2, 3),
                side: "leading".to_owned(),
                boundary_role: "CjkLatin".to_owned(),
                mode: "Insert".to_owned(),
                characters_affected: 1,
                reduction_per_char: -2.0,
                total_reduction: -2.0,
                reason: "test-leading-gap".to_owned(),
            }])
            .metric_decisions(metrics)
            .build(),
    )
}

pub fn interaction_boundary_result() -> LayoutResult {
    result(
        "😀é👩‍👩",
        vec![
            cluster(TextRange::new(0, 2), "😀", 20.0),
            cluster(TextRange::new(2, 4), "é", 20.0),
            cluster(TextRange::new(4, 9), "👩‍👩", 50.0),
        ],
        vec![line(TextRange::new(0, 9), IntRange::new(0, 2), 15.0, 0.0, 20.0, 90.0)],
        Vec::new(),
        Vec::new(),
        LayoutDebugInfo::default(),
    )
}

pub fn word_boundary_result() -> LayoutResult {
    result(
        "前 template 后",
        vec![
            cluster(TextRange::new(0, 1), "前", 10.0),
            cluster(TextRange::new(1, 2), " ", 10.0),
            cluster(TextRange::new(2, 10), "template", 80.0),
            cluster(TextRange::new(10, 11), " ", 10.0),
            cluster(TextRange::new(11, 12), "后", 10.0),
        ],
        vec![line(TextRange::new(0, 12), IntRange::new(0, 4), 15.0, 0.0, 20.0, 120.0)],
        Vec::new(),
        Vec::new(),
        LayoutDebugInfo::default(),
    )
}

pub fn ruby_selection_result() -> LayoutResult {
    result(
        "张王李",
        vec![
            cluster(TextRange::new(0, 1), "张", 35.0),
            cluster(TextRange::new(1, 2), "王", 35.0),
            cluster(TextRange::new(2, 3), "李", 20.0),
        ],
        vec![line(TextRange::new(0, 3), IntRange::new(0, 2), 15.0, 0.0, 20.0, 90.0)],
        vec![GlyphRun::new(TextRange::new(0, 3), "cjk".to_owned(), vec![
            Glyph::builder(1, TextRange::new(0, 1), 20.0).build(),
            Glyph::builder(2, TextRange::new(1, 2), 20.0).build(),
            Glyph::builder(3, TextRange::new(2, 3), 20.0).build(),
        ], 60.0)],
        Vec::new(),
        LayoutDebugInfo::builder()
            .geometry_decisions(vec![
                ClusterGeometryDecisionInfo::builder(TextRange::new(0, 1), Text::from("张"), Text::from("张"), 20.0, 20.0, 0.0, 0.0, 0.0, 0.0, 0.0, 35.0, "test".to_owned(), "RubyAvoidanceSpread".to_owned()).ruby_spread(15.0).build(),
                ClusterGeometryDecisionInfo::builder(TextRange::new(1, 2), Text::from("王"), Text::from("王"), 20.0, 20.0, 0.0, 0.0, 0.0, 0.0, 0.0, 35.0, "test".to_owned(), "RubyAvoidanceSpread".to_owned()).ruby_spread(15.0).build(),
                ClusterGeometryDecisionInfo::builder(TextRange::new(2, 3), Text::from("李"), Text::from("李"), 20.0, 20.0, 0.0, 0.0, 0.0, 0.0, 0.0, 20.0, "test".to_owned(), "RubyAvoidanceSpread".to_owned()).build(),
            ])
            .ruby_decisions(vec![
                RubyDecisionInfo::builder(TextRange::new(0, 1), Text::from("zhuāng"), 0, 10.0, 0.0, 10.0, 6.0).width(32.0).build(),
                RubyDecisionInfo::builder(TextRange::new(1, 2), Text::from("chuáng"), 0, 45.0, 0.0, 10.0, 6.0).width(32.0).build(),
                RubyDecisionInfo::builder(TextRange::new(2, 3), Text::from("shuāng"), 0, 80.0, 0.0, 10.0, 6.0).width(32.0).build(),
            ])
            .build(),
    )
}