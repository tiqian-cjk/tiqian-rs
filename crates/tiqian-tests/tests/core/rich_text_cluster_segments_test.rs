use crate::support::DeterministicStubFontBackend;
use tiqian::api::ParagraphLayoutEngineBuilder;
use tiqian::core::geometry::{
    LayoutConstraints, ScalarOffset, TextRange, scalar_offset, text_range,
};
use tiqian::core::layout_model::LayoutResult;
use tiqian::core::layout_queries::positioned_clusters;
use tiqian::core::text::Text;
use tiqian::core::text_model::{
    DecorationKind, DecorationSpan, LayoutInput, LineLengthGrid, ParagraphStyle,
    RichTextBackgroundPaint, RichTextLayer, RichTextLayerKind, RichTextLinePaint,
    RichTextLinePattern, RichTextPaint, RichTextSpan, TiqianTextContent,
};
use tiqian::core::units::Ic;

fn layout(
    text: &str,
    max_width: f32,
    rich_text: Vec<RichTextSpan>,
    decorations: Vec<DecorationSpan>,
) -> LayoutResult {
    ParagraphLayoutEngineBuilder::new(Box::new(DeterministicStubFontBackend::default()))
        .build()
        .layout(
            LayoutInput::builder(
                TiqianTextContent::new(Text::from(text)),
                LayoutConstraints::with_defaults(max_width),
            )
            .paragraph_style(
                ParagraphStyle::builder()
                    .first_line_indent(Some(Ic::ZERO))
                    .line_length_grid(LineLengthGrid::with_enabled(false))
                    .build(),
            )
            .rich_text(rich_text)
            .decorations(decorations)
            .build(),
        )
}

fn span(range: TextRange, layers: Vec<RichTextLayer>) -> RichTextSpan {
    RichTextSpan {
        range,
        layers,
        semantics: Vec::new(),
    }
}

fn background_layer(id: Option<u32>, fill: i32) -> RichTextLayer {
    RichTextLayer {
        kind: RichTextLayerKind::Background {
            background: RichTextBackgroundPaint::builder()
                .horizontal_padding(1.0)
                .vertical_padding(1.0)
                .corner_radius(3.0)
                .build(),
        },
        paints: vec![RichTextPaint::Fill { argb: fill }],
        id,
    }
}

fn underline_layer(id: Option<u32>, fill: i32, clearance: f32) -> RichTextLayer {
    RichTextLayer {
        kind: RichTextLayerKind::Underline {
            line: RichTextLinePaint {
                thickness: 1.5,
                pattern: RichTextLinePattern::Solid,
                adjacent_same_style_clearance: clearance,
            },
        },
        paints: vec![RichTextPaint::Fill { argb: fill }],
        id,
    }
}

#[test]
fn background_cluster_segments_tile_each_line_segment() {
    let result = layout(
        "中文中文",
        240.0,
        vec![span(
            text_range(0, 4),
            vec![background_layer(Some(7), 0x33_88_AA_FF)],
        )],
        Vec::new(),
    );
    let line_segments = result.rich_text_background_segments();
    assert_eq!(1, line_segments.len());
    let line = &line_segments[0];
    let segments = result.rich_text_layer_cluster_segments();

    assert_eq!(4, segments.len());
    assert_eq!(
        vec![0, 1, 2, 3],
        segments
            .iter()
            .map(|segment| segment.cluster_index)
            .collect::<Vec<_>>()
    );
    assert_eq!(Some(7), segments[0].span.layers[0].id);
    assert!(segments.iter().all(|segment| segment.line_index == 0
        && segment.line_left == line.left
        && segment.line_right == line.right
        && segment.top == line.top
        && segment.bottom == line.bottom
        && segment.baseline == line.baseline
        && !segment.continues_from_previous_line
        && !segment.continues_on_next_line));
    // 片段平铺行段：首片段拥有左外缘，末片段拥有右外缘，相邻边界重合。
    assert_eq!(line.left, segments[0].left);
    assert_eq!(line.right, segments[3].right);
    for pair in segments.windows(2) {
        assert_eq!(pair[0].right, pair[1].left);
    }
    // 聚合等价：片段覆盖的范围与逐行查询一致。
    let aggregated_left = segments
        .iter()
        .map(|segment| segment.left)
        .fold(f32::INFINITY, f32::min);
    let aggregated_right = segments
        .iter()
        .map(|segment| segment.right)
        .fold(f32::NEG_INFINITY, f32::max);
    assert_eq!(line.left, aggregated_left);
    assert_eq!(line.right, aggregated_right);
    // 解析值一致：片段复用行段的四角半径与纵向范围。
    let radii = result.rich_text_background_corner_radii(line, 0.0);
    assert!(
        segments
            .iter()
            .all(|segment| segment.corner_radii == Some(radii) && segment.line_y.is_none())
    );
    assert!(segments.iter().all(|segment| !segment.range.is_empty()));
}

#[test]
fn underline_cluster_segments_reuse_the_resolved_center_line() {
    let result = layout(
        "中文中文",
        240.0,
        vec![span(
            text_range(0, 4),
            vec![underline_layer(Some(1), 0x22_22_22_FF, 0.0)],
        )],
        Vec::new(),
    );
    let line_segments = result.rich_text_decoration_segments();
    assert_eq!(1, line_segments.len());
    let line_y = result.rich_text_decoration_line_y(&line_segments[0], 1.5);
    let segments = result.rich_text_layer_cluster_segments();

    assert_eq!(4, segments.len());
    assert!(
        segments
            .iter()
            .all(|segment| segment.line_y == Some(line_y) && segment.corner_radii.is_none())
    );
    assert_eq!(line_segments[0].left, segments[0].left);
    assert_eq!(line_segments[0].right, segments[3].right);
}

#[test]
fn line_through_cluster_segments_use_their_own_layer_resolution() {
    let result = layout(
        "中文中文",
        240.0,
        vec![span(
            text_range(0, 4),
            vec![RichTextLayer {
                kind: RichTextLayerKind::LineThrough {
                    line: RichTextLinePaint {
                        thickness: 2.0,
                        pattern: RichTextLinePattern::Dotted { gap_length: 1.5 },
                        adjacent_same_style_clearance: 0.0,
                    },
                },
                paints: vec![RichTextPaint::Fill {
                    argb: 0x44_44_44_FF,
                }],
                id: Some(5),
            }],
        )],
        Vec::new(),
    );
    let line_segments = result.rich_text_decoration_segments();
    assert_eq!(1, line_segments.len());
    let line_y = result.rich_text_decoration_line_y(&line_segments[0], 2.0);
    let segments = result.rich_text_layer_cluster_segments();

    assert_eq!(4, segments.len());
    assert!(segments.iter().all(|segment| segment.line_y == Some(line_y)
        && segment.corner_radii.is_none()
        && segment.span.layers[0].id == Some(5)
        && matches!(
            segment.span.layers[0].kind,
            RichTextLayerKind::LineThrough { .. }
        )));
    assert_eq!(line_segments[0].left, segments[0].left);
    assert_eq!(line_segments[0].right, segments[3].right);
}

#[test]
fn adjacent_same_style_clearance_shortens_cluster_segment_edges() {
    let result = layout(
        "中文中文中文中文",
        240.0,
        vec![
            span(
                text_range(0, 2),
                vec![underline_layer(None, 0x11_11_11_FF, 4.0)],
            ),
            span(
                text_range(2, 6),
                vec![underline_layer(None, 0x11_11_11_FF, 4.0)],
            ),
        ],
        Vec::new(),
    );
    let line_segments = result.rich_text_decoration_segments();
    assert_eq!(2, line_segments.len());
    // 相邻同样式范围各自让出 clearance 的一半；范围外侧没有邻居，外缘不变。
    assert_eq!(0.0, line_segments[0].left);
    assert_eq!(30.0, line_segments[0].right);
    assert_eq!(34.0, line_segments[1].left);
    assert_eq!(96.0, line_segments[1].right);

    let segments = result.rich_text_layer_cluster_segments();
    assert_eq!(6, segments.len());
    assert_eq!(0.0, segments[0].left);
    assert_eq!(30.0, segments[1].right);
    assert_eq!(34.0, segments[2].left);
    assert_eq!(96.0, segments[5].right);
}

#[test]
fn decoration_cluster_segments_follow_line_edges_and_open_flags() {
    let result = layout(
        "王小明大同先生",
        64.0,
        Vec::new(),
        vec![DecorationSpan {
            range: text_range(0, 5),
            kind: DecorationKind::Mourning,
            id: Some(9),
        }],
    );
    let line_segments = &result.debug.decoration_segments;
    assert_eq!(2, line_segments.len());

    let segments = result.decoration_cluster_segments();
    assert!(segments.len() >= 4);
    assert!(
        segments
            .iter()
            .all(|segment| segment.id == Some(9) && segment.kind == DecorationKind::Mourning)
    );
    for line in line_segments {
        let line_fragments: Vec<_> = segments
            .iter()
            .filter(|segment| segment.line_index == line.line_index)
            .collect();
        assert!(!line_fragments.is_empty());
        assert_eq!(line.left, line_fragments[0].left);
        assert_eq!(line.right, line_fragments[line_fragments.len() - 1].right);
        assert_eq!(line.top, line_fragments[0].top);
        assert_eq!(line.bottom, line_fragments[0].bottom);
        assert!(
            line_fragments
                .iter()
                .all(|segment| segment.open_start == line.open_start
                    && segment.open_end == line.open_end
                    && segment.line_left == line.left
                    && segment.line_right == line.right
                    && !segment.range.is_empty())
        );
        for pair in line_fragments.windows(2) {
            assert_eq!(pair[0].right, pair[1].left);
            assert!(pair[0].cluster_index < pair[1].cluster_index);
        }
    }
    assert!(segments.iter().any(|segment| segment.open_end));
    assert!(segments.iter().any(|segment| segment.open_start));
}

#[test]
fn interlinear_decoration_cluster_segments_carry_the_center_line() {
    let result = layout(
        "中文著作",
        240.0,
        Vec::new(),
        vec![DecorationSpan {
            range: text_range(0, 4),
            kind: DecorationKind::ProperNoun,
            id: None,
        }],
    );
    let line_segment = result
        .debug
        .decoration_segments
        .first()
        .expect("proper-noun decoration must produce a line segment");
    assert_eq!(line_segment.top, line_segment.bottom);

    let segments = result.decoration_cluster_segments();
    assert_eq!(4, segments.len());
    assert!(segments.iter().all(|segment| segment.top == segment.bottom
        && segment.id.is_none()
        && segment.kind == DecorationKind::ProperNoun));
    assert_eq!(line_segment.left, segments[0].left);
    assert_eq!(line_segment.right, segments[3].right);
}

#[test]
fn emphasis_cluster_segments_are_not_duplicated_as_geometry() {
    let result = layout(
        "中文著作",
        240.0,
        Vec::new(),
        vec![DecorationSpan {
            range: text_range(0, 4),
            kind: DecorationKind::Emphasis,
            id: Some(3),
        }],
    );
    assert!(!result.debug.decoration_decisions.is_empty());
    assert!(
        result
            .debug
            .decoration_decisions
            .iter()
            .all(|decision| decision.id == Some(3))
    );
    assert!(result.debug.decoration_segments.is_empty());
    assert!(result.decoration_cluster_segments().is_empty());
}

#[test]
fn declared_source_boundary_only_changes_cluster_granularity() {
    // 绝对 glyph 位置与 advance：声明边界只切成更小的 cluster，不移动文字。
    fn absolute_glyph_positions(result: &LayoutResult) -> Vec<(f32, f32, f32)> {
        let mut out = Vec::new();
        for position in positioned_clusters(result) {
            for glyph in result
                .glyph_runs
                .iter()
                .flat_map(|run| run.glyphs.iter())
                .filter(|glyph| glyph.cluster_range == position.range)
            {
                out.push((
                    position.draw_x + glyph.x,
                    position.baseline + glyph.y,
                    glyph.advance,
                ));
            }
        }
        out
    }

    fn run(boundary: Option<ScalarOffset>) -> LayoutResult {
        let content = match boundary {
            Some(offset) => TiqianTextContent::builder(Text::from("abc"))
                .source_boundaries(tiqian::common::HashSet::from([offset]))
                .build(),
            None => TiqianTextContent::new(Text::from("abc")),
        };
        ParagraphLayoutEngineBuilder::new(Box::new(DeterministicStubFontBackend::default()))
            .build()
            .layout(
                LayoutInput::builder(content, LayoutConstraints::with_defaults(240.0))
                    .paragraph_style(
                        ParagraphStyle::builder()
                            .first_line_indent(Some(Ic::ZERO))
                            .line_length_grid(LineLengthGrid::with_enabled(false))
                            .build(),
                    )
                    .build(),
            )
    }

    let split = run(Some(scalar_offset(1)));
    let whole = run(None);

    assert_eq!(2, split.clusters.len());
    assert_eq!(1, whole.clusters.len());
    assert_eq!(whole.lines.len(), split.lines.len());
    assert_eq!(whole.lines[0].range, split.lines[0].range);
    assert_eq!(whole.lines[0].visual_width, split.lines[0].visual_width);
    assert_eq!(whole.lines[0].adjusted_width, split.lines[0].adjusted_width);
    assert_eq!(
        absolute_glyph_positions(&whole),
        absolute_glyph_positions(&split)
    );
}

#[test]
fn out_of_range_and_duplicate_source_boundaries_are_inert() {
    fn layout_with(boundaries: Vec<ScalarOffset>) -> LayoutResult {
        ParagraphLayoutEngineBuilder::new(Box::new(DeterministicStubFontBackend::default()))
            .build()
            .layout(
                LayoutInput::builder(
                    TiqianTextContent::builder(Text::from("中文"))
                        .source_boundaries(tiqian::common::HashSet::from_iter(boundaries))
                        .build(),
                    LayoutConstraints::with_defaults(240.0),
                )
                .paragraph_style(
                    ParagraphStyle::builder()
                        .first_line_indent(Some(Ic::ZERO))
                        .line_length_grid(LineLengthGrid::with_enabled(false))
                        .build(),
                )
                .build(),
            )
    }

    let plain = layout_with(Vec::new());
    let noisy = layout_with(vec![
        ScalarOffset::ZERO,
        scalar_offset(1),
        scalar_offset(1),
        scalar_offset(9),
    ]);

    assert_eq!(plain.clusters.len(), noisy.clusters.len());
    assert_eq!(plain.lines[0].range, noisy.lines[0].range);
    assert_eq!(plain.lines[0].visual_width, noisy.lines[0].visual_width);
}
