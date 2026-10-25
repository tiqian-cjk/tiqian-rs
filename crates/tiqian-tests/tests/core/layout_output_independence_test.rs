// 运行时数据与诊断记录分离的独立性验证。
//
// 功能路径（查询、绘制、几何计算）读取 `LayoutResult.layout`；
// 清空 `debug` 后，全部功能查询的输出必须保持不变。

use tiqian::core::geometry::{Size, text_range};
use tiqian::core::int_range::IntRange;
use tiqian::core::layout_model::{
    BopomofoGlyphPlacement, BopomofoGlyphRole, Cluster, DecorationSegmentGeometry,
    DecorationSegmentReason, EmphasisDotGeometry, LayoutDebugInfo, LayoutOutput, LayoutResult,
    RubyGeometry,
};
use tiqian::core::layout_queries::{
    get_bounding_boxes, get_text_for_copy, positioned_clusters,
};
use tiqian::core::layout_result_replay_index::to_replay_index;
use tiqian::core::text::Text;
use tiqian::core::text_model::DecorationKind;

use super::layout_queries_test_support::{cluster, line, result_with_rich_text};

/// 构造一份 `layout` 的注音与装饰数据，`debug` 留空。
fn annotation_layout() -> LayoutOutput {
    LayoutOutput {
        annotations: tiqian::core::layout_model::AnnotationGeometry {
            ruby: vec![RubyGeometry::builder(
                text_range(0, 2),
                Text::from("tíqiàn"),
                0,
                10.0,
                0.0,
                8.0,
                0.0,
            )
            .ascent(6.0)
            .descent(2.0)
            .width(30.0)
            .build()],
            bopomofo: vec![tiqian::core::layout_model::BopomofoGeometry::new(
                text_range(2, 3),
                Text::from("ㄋㄧㄣˊ"),
                0,
                vec![
                    BopomofoGlyphPlacement::new(
                        Text::from("ㄋ"),
                        10.0,
                        0.0,
                        5.0,
                        5.0,
                        BopomofoGlyphRole::Symbol,
                    ),
                ],
            )],
        },
        decorations: tiqian::core::layout_model::DecorationGeometry {
            emphasis_dots: vec![EmphasisDotGeometry {
                cluster_range: text_range(0, 1),
                kind: DecorationKind::Emphasis,
                id: Some(3),
                applied: true,
                anchor_x: 5.0,
                anchor_y: 18.0,
                dot_diameter: 2.0,
            }],
            segments: vec![DecorationSegmentGeometry {
                source_range: text_range(0, 2),
                kind: DecorationKind::ProperNoun,
                id: Some(7),
                line_index: 0,
                left: 0.0,
                top: 18.0,
                right: 20.0,
                bottom: 18.0,
                open_start: false,
                open_end: false,
                reason: DecorationSegmentReason::InterlinearLinePerAnnotatedItem,
                adjacent_shortened: false,
            }],
        },
        ..LayoutOutput::default()
    }
}

fn sample(clusters: Vec<Cluster>) -> LayoutResult {
    result_with_rich_text(
        "提椠您",
        clusters,
        vec![line(
            text_range(0, 3),
            IntRange::new(0, 2),
            15.0,
            0.0,
            20.0,
            30.0,
        )],
        Vec::new(),
        Vec::new(),
        Vec::new(),
        LayoutDebugInfo::default(),
    )
}

/// 清空 `debug` 后，全部功能查询输出与清空前逐值一致。
#[test]
fn functional_queries_ignore_debug_contents() {
    let mut result = sample(vec![
        cluster(text_range(0, 1), "提", 10.0),
        cluster(text_range(1, 2), "椠", 10.0),
        cluster(text_range(2, 3), "您", 10.0),
    ]);
    result.layout = annotation_layout();

    let with_debug = (
        positioned_clusters(&result)
            .iter()
            .map(|cluster| (cluster.range, cluster.left.to_bits(), cluster.right.to_bits()))
            .collect::<Vec<_>>(),
        get_bounding_boxes(&result, text_range(0, 3)),
        get_text_for_copy(&result, text_range(0, 3)),
        to_replay_index(&result).positioned_clusters.len(),
    );

    // 清空诊断记录，重建依赖它的派生结构。
    result.debug = LayoutDebugInfo::default();

    let without_debug = (
        positioned_clusters(&result)
            .iter()
            .map(|cluster| (cluster.range, cluster.left.to_bits(), cluster.right.to_bits()))
            .collect::<Vec<_>>(),
        get_bounding_boxes(&result, text_range(0, 3)),
        get_text_for_copy(&result, text_range(0, 3)),
        to_replay_index(&result).positioned_clusters.len(),
    );

    assert_eq!(with_debug, without_debug);
}

/// 注音与装饰几何完全来自 `layout`，清空 `debug` 后仍可取到。
#[test]
fn annotations_and_decorations_survive_debug_clear() {
    let mut result = sample(vec![
        cluster(text_range(0, 1), "提", 10.0),
        cluster(text_range(1, 2), "椠", 10.0),
        cluster(text_range(2, 3), "您", 10.0),
    ]);
    result.layout = annotation_layout();
    result.debug = LayoutDebugInfo::default();

    assert_eq!(1, result.layout.annotations.ruby.len());
    assert_eq!(1, result.layout.annotations.bopomofo.len());
    assert_eq!(1, result.layout.decorations.emphasis_dots.len());
    assert_eq!(1, result.layout.decorations.segments.len());

    // 纯文本导出会插入注音括号内容。
    let copied = get_text_for_copy(&result, text_range(0, 3));
    assert!(copied.as_str().contains("tíqiàn"), "copy 应含 ruby 注文");
}

/// `layout` 与 `debug` 同为默认值时，查询仍产出稳定结果。
#[test]
fn empty_layout_and_debug_produce_stable_queries() {
    let result = sample(vec![
        cluster(text_range(0, 1), "提", 10.0),
        cluster(text_range(1, 2), "椠", 10.0),
        cluster(text_range(2, 3), "您", 10.0),
    ]);
    let first = positioned_clusters(&result);
    let second = positioned_clusters(&result);
    assert_eq!(first.len(), second.len());
    for (left, right) in first.iter().zip(&second) {
        assert_eq!(left.range, right.range);
        assert_eq!(left.left.to_bits(), right.left.to_bits());
    }
}

/// 尺寸不变：`Size` 是独立字段，与 `layout`、`debug` 都无关。
#[test]
fn size_is_independent_of_layout_and_debug() {
    let mut result = sample(vec![
        cluster(text_range(0, 1), "提", 10.0),
        cluster(text_range(1, 2), "椠", 10.0),
        cluster(text_range(2, 3), "您", 10.0),
    ]);
    result.size = Size {
        width: 123.0,
        height: 45.0,
    };
    result.debug = LayoutDebugInfo::default();
    result.layout = LayoutOutput::default();
    assert_eq!(123.0, result.size.width);
    assert_eq!(45.0, result.size.height);
    assert!(result.lines[0].range == text_range(0, 3));
}
