// 对应 Kotlin 源文件：engine/src/commonMain/kotlin/org/tiqian/core/LayoutQueries.kt

use std::sync::Arc;

use icu_properties::{CodePointSetData, props::UnifiedIdeograph};

use super::geometry::{Rect, ScalarOffset, TextRange};
use super::layout_model::{LayoutResult, LineBox, MetricDecisionInfo};
use super::source_interaction_boundaries::{
    SourceBoundaryBias, coerce_to_interaction_boundary, interaction_boundaries,
};
use super::text::Text;
use super::text_model::{
    RichTextBackgroundMetricPolicy, RichTextPaint, RichTextRole, RichTextSpan, TextStyle,
};

const INTERLINEAR_UNDERLINE_OFFSET_EM: f32 = 0.18;

/// A cluster positioned in layout coordinates. The rectangle is the cluster's line box slice, not
/// glyph ink bounds: selection, hit testing, and accessibility need stable occupied text geometry.
#[derive(Clone, Debug, PartialEq)]
pub struct PositionedCluster {
    pub line_index: i32,
    pub cluster_index: i32,
    pub range: TextRange,
    /// Occupied text box left edge, used for selection, hit testing, and accessibility.
    pub left: f32,
    pub top: f32,
    /// Occupied text box right edge, used for selection, hit testing, and accessibility.
    pub right: f32,
    pub bottom: f32,
    pub baseline: f32,
    /// Glyph draw origin. This may differ from [`Self::left`] when the cluster carries
    /// leading autospace or consumed leading punctuation glue.
    pub draw_x: f32,
    /// Per-source-offset x boundaries inside this cluster: `range.length + 1` entries running from
    /// [`Self::left`] to [`Self::right`]. The two ends are always the occupied box edges, so compressed line-edge
    /// punctuation (a full-width glyph advancing past its half-width cluster box) and 两端对齐 stretch
    /// never overshoot. Interior entries come from the shaped glyph origins, so a caret or selection
    /// endpoint inside a proportional Latin word lands on the real letter edge instead of a linear
    /// guess. None when the run did not emit one glyph per source unit; callers then interpolate
    /// linearly over the occupied box.
    pub source_stops: Option<Vec<f32>>,
}

impl PositionedCluster {
    pub fn builder(
        line_index: i32,
        cluster_index: i32,
        range: TextRange,
        left: f32,
        top: f32,
        right: f32,
        bottom: f32,
        baseline: f32,
    ) -> PositionedClusterBuilder {
        PositionedClusterBuilder {
            cluster: Self {
                line_index,
                cluster_index,
                range,
                left,
                top,
                right,
                bottom,
                baseline,
                draw_x: left,
                source_stops: None,
            },
        }
    }

    pub fn width(&self) -> f32 {
        self.right - self.left
    }

    pub fn height(&self) -> f32 {
        self.bottom - self.top
    }

    pub fn rect(&self) -> Rect {
        Rect {
            left: self.left,
            top: self.top,
            right: self.right,
            bottom: self.bottom,
        }
    }
}

pub struct PositionedClusterBuilder {
    cluster: PositionedCluster,
}

impl PositionedClusterBuilder {
    pub fn draw_x(mut self, value: f32) -> Self {
        self.cluster.draw_x = value;
        self
    }

    pub fn source_stops(mut self, value: Option<Vec<f32>>) -> Self {
        self.cluster.source_stops = value;
        self
    }

    pub fn build(self) -> PositionedCluster {
        self.cluster
    }
}

/// A per-line occupied geometry segment covered by a [`RichTextSpan`]. Segments are derived from the
/// same positioned clusters used for selection/hit testing, not from renderer-side text shaping.
#[derive(Clone, Debug, PartialEq)]
pub struct RichTextLineSegment {
    /// FIXME(identity-semantics)：Kotlin 原字段是 `RichTextSpan`，并在 segment 合并处以 `===` 比较对象身份。
    /// Rust 值类型没有等价的引用身份，因此这里妥协为 `Arc<RichTextSpan>`，后续必须用
    /// `Arc::ptr_eq` 保留“仅同一个输入 span 的切片可合并”的原语义。
    pub span: Arc<RichTextSpan>,
    pub line_index: i32,
    pub range: TextRange,
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub baseline: f32,
}

impl RichTextLineSegment {
    pub fn new(
        span: Arc<RichTextSpan>,
        line_index: i32,
        range: TextRange,
        left: f32,
        top: f32,
        right: f32,
        bottom: f32,
        baseline: f32,
    ) -> Self {
        Self {
            span,
            line_index,
            range,
            left,
            top,
            right,
            bottom,
            baseline,
        }
    }

    pub fn width(&self) -> f32 {
        self.right - self.left
    }

    pub fn height(&self) -> f32 {
        self.bottom - self.top
    }

    pub fn rect(&self) -> Rect {
        Rect {
            left: self.left,
            top: self.top,
            right: self.right,
            bottom: self.bottom,
        }
    }

    /// The marked source range began on an earlier visual line.
    pub fn continues_from_previous_line(&self) -> bool {
        self.range.start() > self.span.range.start()
    }

    /// The marked source range continues onto a later visual line.
    pub fn continues_on_next_line(&self) -> bool {
        self.range.end() < self.span.range.end()
    }
}

/// Four physical corner radii resolved from one final rich-text background segment.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RichTextCornerRadii {
    pub top_left: f32,
    pub top_right: f32,
    pub bottom_right: f32,
    pub bottom_left: f32,
}

impl RichTextCornerRadii {
    pub fn is_square(&self) -> bool {
        self.top_left == 0.0
            && self.top_right == 0.0
            && self.bottom_right == 0.0
            && self.bottom_left == 0.0
    }

    pub fn is_uniform(&self) -> bool {
        self.top_left == self.top_right
            && self.top_right == self.bottom_right
            && self.bottom_right == self.bottom_left
    }
}

/// Resolves `RichTextBackgroundContinuationCorners` from source-continuation geometry. A true
/// source endpoint keeps `RichTextBackgroundPaint.cornerRadius`; an edge split by line breaking uses
/// `RichTextBackgroundPaint.continuationCornerRadius`. `inset` mirrors a centered border stroke that
/// is moved inside the measured box. Renderers consume these four values instead of independently
/// deciding whether a fragment is open or closed.
pub fn resolved_background_corner_radii(
    segment: &RichTextLineSegment,
    inset: f32,
) -> RichTextCornerRadii {
    assert!(inset.is_finite() && inset >= 0.0);
    let box_width = (segment.width() - inset * 2.0).max(0.0);
    let box_height = (segment.height() - inset * 2.0).max(0.0);
    let maximum = (box_width / 2.0).min(box_height / 2.0);
    let resolve = |radius: f32| (radius - inset).clamp(0.0, maximum);

    let paint = &segment.span.paint.background;
    let left = resolve(if segment.continues_from_previous_line() {
        paint.continuation_corner_radius
    } else {
        paint.corner_radius
    });
    let right = resolve(if segment.continues_on_next_line() {
        paint.continuation_corner_radius
    } else {
        paint.corner_radius
    });
    RichTextCornerRadii {
        top_left: left,
        top_right: right,
        bottom_right: right,
        bottom_left: left,
    }
}

/// Plain-text clipboard projection for `range`. Visible layout substitutions and soft-wrap glyphs
/// never enter this string: it starts from source text, then mirrors the Web frontend's annotation
/// contract by appending a fully-selected ruby / 注音 reading in full-width parentheses after its
/// base. A partial selection of a multi-character base does not invent a detached reading.
pub fn get_text_for_copy(result: &LayoutResult, range: TextRange) -> Text {
    let source = &result.input.content.text;
    let text_length = source.scalar_len();
    let start = range.start().min(text_length);
    let end = range.end().min(text_length).max(start);
    if start == end {
        return Text::new();
    }

    let mut annotations_by_end: Vec<(ScalarOffset, Vec<Text>)> = Vec::new();
    let mut add_annotation = |base_range: TextRange, text: &Text| {
        if base_range.start() < start || base_range.end() > end {
            return;
        }
        let annotation = Text::from(format!("（{text}）"));
        if let Some((_, annotations)) = annotations_by_end
            .iter_mut()
            .find(|(annotation_end, _)| *annotation_end == base_range.end())
        {
            annotations.push(annotation);
        } else {
            annotations_by_end.push((base_range.end(), vec![annotation]));
        }
    };
    for decision in &result.debug.ruby_decisions {
        add_annotation(decision.base_range, &decision.text);
    }
    for decision in &result.debug.bopomofo_decisions {
        add_annotation(decision.base_range, &decision.text);
    }
    annotations_by_end.sort_by_key(|(annotation_end, _)| *annotation_end);

    let mut out = String::new();
    let mut cursor = start;
    for (annotation_end, annotations) in annotations_by_end {
        if annotation_end < cursor || annotation_end > end {
            continue;
        }
        out.push_str(source.slice_offsets(cursor, annotation_end));
        for annotation in annotations {
            out.push_str(annotation.as_str());
        }
        cursor = annotation_end;
    }
    out.push_str(source.slice_offsets(cursor, end));
    Text::from(out)
}

/// Returns each cluster's occupied rectangle using the same line/cluster advances consumed by
/// renderers. This is the geometry bridge for links, selection, and accessibility.
pub fn positioned_clusters(result: &LayoutResult) -> Vec<PositionedCluster> {
    result
        .lines
        .iter()
        .enumerate()
        .flat_map(|(line_index, line)| {
            positioned_clusters_for_line(result, line_index as i32, line)
        })
        .collect()
}

/// Returns the positioned clusters for `line`.
pub fn positioned_clusters_for_line_box(
    result: &LayoutResult,
    line: &LineBox,
) -> Vec<PositionedCluster> {
    let line_index = result
        .lines
        .iter()
        .position(|candidate| candidate == line)
        .expect("line must belong to this LayoutResult.");
    positioned_clusters_for_line(result, line_index as i32, line)
}

/// Returns the visible glyph-ink bounds of the laid-out lines, in layout coordinates.
///
/// This is intentionally separate from `positioned_clusters`: ink overhang (notably italic
/// Latin) may extend outside the occupied advance box, but selection, links, and hit testing
/// must continue to use stable occupied geometry. Renderers use this only to avoid clipping
/// real ink that belongs to already-emitted line boxes.
pub fn glyph_ink_bounds(result: &LayoutResult) -> Option<Rect> {
    let positions = positioned_clusters(result);
    let mut left = f32::INFINITY;
    let mut top = f32::INFINITY;
    let mut right = f32::NEG_INFINITY;
    let mut bottom = f32::NEG_INFINITY;
    for run in &result.glyph_runs {
        for glyph in &run.glyphs {
            let Some(bounds) = glyph.bounds else {
                continue;
            };
            let Some(cluster) = positions
                .iter()
                .find(|positioned| positioned.range == glyph.cluster_range)
            else {
                continue;
            };
            left = left.min(cluster.draw_x + glyph.x + bounds.left);
            top = top.min(cluster.baseline + glyph.y + bounds.top);
            right = right.max(cluster.draw_x + glyph.x + bounds.right);
            bottom = bottom.max(cluster.baseline + glyph.y + bounds.bottom);
        }
    }
    if !left.is_finite() || !top.is_finite() || !right.is_finite() || !bottom.is_finite() {
        return None;
    }
    Some(Rect {
        left,
        top,
        right,
        bottom,
    })
}

/// Compose-like line lookup backed by Tiqian line boxes. End offsets attach to the previous line so
/// a caret at paragraph end stays on the final visible line.
pub fn get_line_for_offset(result: &LayoutResult, offset: ScalarOffset) -> i32 {
    if result.lines.is_empty() {
        return -1;
    }
    let text_length = result.input.content.text.scalar_len();
    let clamped = nearest_interaction_offset(result, offset.min(text_length));
    if clamped == text_length {
        return result.lines.len() as i32 - 1;
    }
    result
        .lines
        .iter()
        .position(|line| clamped >= line.range.start() && clamped < line.range.end())
        .map(|index| index as i32)
        .unwrap_or_else(|| nearest_line_for_offset(result, clamped))
}

/// Returns the occupied cluster box containing `offset`. A paragraph-end offset returns the final
/// caret rectangle so accessibility callers still get a concrete position.
pub fn get_bounding_box(result: &LayoutResult, offset: ScalarOffset) -> Rect {
    if result.lines.is_empty() {
        return Rect {
            left: 0.0,
            top: 0.0,
            right: 0.0,
            bottom: 0.0,
        };
    }
    let text_length = result.input.content.text.scalar_len();
    let clamped = nearest_interaction_offset(result, offset.min(text_length));
    if clamped == text_length {
        return get_cursor_rect(result, clamped);
    }
    positioned_clusters(result)
        .into_iter()
        .find(|cluster| clamped >= cluster.range.start() && clamped < cluster.range.end())
        .map(|cluster| cluster.rect())
        .unwrap_or_else(|| get_cursor_rect(result, clamped))
}

/// Returns one or more line-box slices covered by `range`. When a source range cuts through a
/// multi-code-unit display cluster, `SourceRangeLinearClusterSplit` divides the cluster box
/// proportionally by source UTF-16 offsets until glyph-level source mapping lands.
pub fn get_bounding_boxes(result: &LayoutResult, range: TextRange) -> Vec<Rect> {
    if range.is_empty() || result.lines.is_empty() {
        return Vec::new();
    }
    let text_length = result.input.content.text.scalar_len();
    let start = range.start().min(text_length);
    let end = range.end().min(text_length).max(start);
    if start == end {
        return Vec::new();
    }
    positioned_clusters(result)
        .into_iter()
        .filter_map(|cluster| {
            let slice_start = start.max(cluster.range.start());
            let slice_end = end.min(cluster.range.end());
            (slice_start < slice_end).then(|| slice_rect(&cluster, slice_start, slice_end))
        })
        .collect()
}

pub fn get_bounding_boxes_from_offsets(
    result: &LayoutResult,
    start: ScalarOffset,
    end: ScalarOffset,
) -> Vec<Rect> {
    get_bounding_boxes(result, TextRange::new(start, end))
}

/// Returns continuous line-local geometry for rich-text spans. A span crossing lines is split at
/// line boundaries; a span cutting through a multi-code-unit display cluster uses the same
/// proportional source split as `get_bounding_boxes`.
pub fn positioned_rich_text_segments(
    result: &LayoutResult,
    spans: &[RichTextSpan],
) -> Vec<RichTextLineSegment> {
    if spans.is_empty() || result.lines.is_empty() {
        return Vec::new();
    }
    let clusters = positioned_clusters(result);
    let text_length = result.input.content.text.scalar_len();
    let mut out = Vec::new();
    for span in spans {
        let start = span.range.start().min(text_length);
        let end = span.range.end().min(text_length).max(start);
        if start == end {
            continue;
        }
        // One normalized span instance for ALL of this span's slices — allocated once
        // (not per overlapping cluster) so the merge check compares by identity.
        let normalized = Arc::new(RichTextSpan {
            range: TextRange::new(start, end),
            role: span.role.clone(),
            paint: span.paint.clone(),
        });
        let mut pending: Option<RichTextLineSegment> = None;
        // Clusters are source-ordered: binary-search the first cluster that reaches
        // the span, and stop past its end — each span scans only its own window.
        let mut lo = 0_usize;
        let mut hi = clusters.len();
        while lo < hi {
            let mid = (lo + hi) >> 1;
            if clusters[mid].range.end() <= start {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        for cluster in &clusters[lo..] {
            if cluster.range.start() >= end {
                break;
            }
            let slice_start = start.max(cluster.range.start());
            let slice_end = end.min(cluster.range.end());
            if slice_start >= slice_end {
                continue;
            }
            let rect = slice_rect(cluster, slice_start, slice_end);
            let next = RichTextLineSegment::new(
                Arc::clone(&normalized),
                cluster.line_index,
                TextRange::new(slice_start, slice_end),
                rect.left,
                rect.top,
                rect.right,
                rect.bottom,
                cluster.baseline,
            );
            if let Some(current) = pending.take() {
                if current.line_index == next.line_index
                    && Arc::ptr_eq(&current.span, &next.span)
                    && current.range.end() == next.range.start()
                {
                    // Source-contiguous occupied slices merge into one continuous rect, so a decoration
                    // or background never acquires an internal sliver (涂). Outer punctuation glue is
                    // removed afterwards by trimmed_rich_text_decoration_segments, not here.
                    pending = Some(RichTextLineSegment {
                        span: current.span,
                        line_index: current.line_index,
                        range: TextRange::new(current.range.start(), next.range.end()),
                        left: current.left,
                        top: current.top.min(next.top),
                        right: next.right,
                        bottom: current.bottom.max(next.bottom),
                        baseline: current.baseline,
                    });
                } else {
                    out.push(current);
                    pending = Some(next);
                }
            } else {
                pending = Some(next);
            }
        }
        if let Some(segment) = pending {
            out.push(segment);
        }
    }
    out
}

/// Returns underline/strike-through segments with punctuation glue removed at the decoration's outer
/// source edges. `RichTextDecorationPunctuationGlueTrim` removes punctuation glue, which lives in
/// the recorded occupied geometry rather than the glyph. It keeps the occupied geometry unchanged for
/// backgrounds, links, selection and hit testing; glue between two decorated clusters also remains
/// covered so a continuous decoration does not acquire an internal gap.
pub fn trimmed_rich_text_decoration_segments(
    result: &LayoutResult,
    occupied_segments: &[RichTextLineSegment],
) -> Vec<RichTextLineSegment> {
    if occupied_segments.is_empty() {
        return Vec::new();
    }
    let decoration_segments: Vec<_> = occupied_segments
        .iter()
        .filter(|segment| {
            matches!(
                segment.span.role,
                RichTextRole::Underline | RichTextRole::LineThrough
            )
        })
        .cloned()
        .collect();
    if decoration_segments.is_empty() {
        return Vec::new();
    }
    with_adjacent_same_style_clearance(trim_outer_punctuation_glue(result, &decoration_segments))
}

/// Returns one continuous paint box per visual line for background roles. The horizontal box keeps
/// every authored/internal gap between the first and last marked clusters, while its two outer edges
/// exclude autospace, justification and punctuation glue owned by neighbouring text. Vertically it
/// uses the marked clusters' typographic faces rather than the complete line box, so paragraph
/// leading does not inflate a short highlight.
pub fn rich_text_background_segments(
    result: &LayoutResult,
    occupied_segments: &[RichTextLineSegment],
) -> Vec<RichTextLineSegment> {
    if occupied_segments.is_empty() {
        return Vec::new();
    }
    let backgrounds: Vec<_> = occupied_segments
        .iter()
        .filter(|segment| {
            matches!(
                segment.span.role,
                RichTextRole::Background | RichTextRole::InlineCode
            )
        })
        .cloned()
        .collect();
    if backgrounds.is_empty() {
        return Vec::new();
    }
    let positioned = positioned_clusters(result);
    let trimmed = trim_outer_punctuation_glue(result, &backgrounds);
    let segments = trimmed
        .into_iter()
        .map(|segment| {
            let covered: Vec<_> = positioned
                .iter()
                .filter(|cluster| {
                    cluster.line_index == segment.line_index
                        && cluster.range.end() > segment.range.start()
                        && cluster.range.start() < segment.range.end()
                })
                .collect();
            if covered.is_empty() {
                return segment;
            }

            let first = covered[0];
            let last = covered[covered.len() - 1];
            let horizontal_padding = segment.span.paint.background.horizontal_padding;
            let leading_padding = if segment.range.start() == segment.span.range.start() {
                horizontal_padding
            } else {
                0.0
            };
            let trailing_padding = if segment.range.end() == segment.span.range.end() {
                horizontal_padding
            } else {
                0.0
            };
            let left = segment
                .left
                .max(first.draw_x - leading_padding)
                .min(segment.right);
            let natural_last_right = result
                .glyph_runs
                .iter()
                .flat_map(|run| run.glyphs.iter())
                .filter(|glyph| glyph.cluster_range == last.range)
                .map(|glyph| last.draw_x + glyph.x + glyph.advance)
                .max_by(|a, b| a.total_cmp(b))
                .unwrap_or(last.right);
            let right = segment
                .right
                .min(natural_last_right + trailing_padding)
                .max(left);

            let (face_top, face_bottom) = match segment.span.paint.background.metric_policy {
                RichTextBackgroundMetricPolicy::MarkedFaces => {
                    marked_face_vertical_bounds(result, &covered, &result.debug.metric_decisions)
                }
                RichTextBackgroundMetricPolicy::UniformTextStyle => {
                    uniform_text_style_vertical_bounds(
                        result,
                        &segment,
                        &result.debug.metric_decisions,
                        &resolved_text_style_at(result, segment.range.start()),
                    )
                }
                RichTextBackgroundMetricPolicy::UniformParagraphStyle => {
                    uniform_text_style_vertical_bounds(
                        result,
                        &segment,
                        &result.debug.metric_decisions,
                        &result.input.text_style,
                    )
                }
            };
            let vertical_padding = segment.span.paint.background.vertical_padding;
            RichTextLineSegment {
                top: (face_top - vertical_padding).max(segment.top),
                bottom: (face_bottom + vertical_padding).min(segment.bottom),
                left,
                right,
                ..segment
            }
        })
        .collect();
    with_adjacent_same_style_clearance(segments)
}

/// `AdjacentSameStyleRichTextClearance`: two source-adjacent runs with the same role and visible
/// paint share one explicit gap. Each side yields half of the configured clearance. Different
/// roles or paints remain untouched; in particular, a highlight beside an underline is not a
/// cross-style avoidance case.
fn with_adjacent_same_style_clearance(
    segments: Vec<RichTextLineSegment>,
) -> Vec<RichTextLineSegment> {
    if segments.len() < 2 {
        return segments;
    }
    segments
        .iter()
        .cloned()
        .map(|segment| {
            let leading_neighbour = segments.iter().find(|other| {
                other.line_index == segment.line_index
                    && other.range.end() == segment.range.start()
                    && same_visible_style(&segment, other)
            });
            let trailing_neighbour = segments.iter().find(|other| {
                other.line_index == segment.line_index
                    && other.range.start() == segment.range.end()
                    && same_visible_style(&segment, other)
            });
            let shared_clearance = |other: Option<&RichTextLineSegment>| {
                other
                    .map(|candidate| {
                        segment
                            .span
                            .paint
                            .adjacent_same_style_clearance
                            .min(candidate.span.paint.adjacent_same_style_clearance)
                    })
                    .unwrap_or(0.0)
            };
            let left =
                (segment.left + shared_clearance(leading_neighbour) / 2.0).min(segment.right);
            RichTextLineSegment {
                left,
                right: (segment.right - shared_clearance(trailing_neighbour) / 2.0).max(left),
                ..segment
            }
        })
        .collect()
}

fn same_visible_style(left: &RichTextLineSegment, right: &RichTextLineSegment) -> bool {
    left.span.role == right.span.role
        && visible_paint(&left.span.paint) == visible_paint(&right.span.paint)
}

fn visible_paint(paint: &RichTextPaint) -> RichTextPaint {
    RichTextPaint {
        argb: paint.argb,
        line_pattern: paint.line_pattern.clone(),
        background: paint.background.clone(),
        adjacent_same_style_clearance: 0.0,
    }
}

fn marked_face_vertical_bounds(
    result: &LayoutResult,
    covered: &[&PositionedCluster],
    metrics: &[MetricDecisionInfo],
) -> (f32, f32) {
    let mut top = f32::INFINITY;
    let mut bottom = f32::NEG_INFINITY;
    for cluster in covered {
        let metric = metrics.iter().rev().find(|decision| {
            cluster.range.start() >= decision.range.start()
                && cluster.range.end() <= decision.range.end()
        });
        if let Some(metric) = metric {
            top = top.min(cluster.baseline - metric.layout_ascent);
            bottom = bottom.max(cluster.baseline + metric.layout_descent);
        } else {
            let style = resolved_text_style_at(result, cluster.range.start());
            top = top.min(cluster.baseline - style.font_size * BACKGROUND_FALLBACK_ASCENT_EM);
            bottom =
                bottom.max(cluster.baseline + style.font_size * BACKGROUND_FALLBACK_DESCENT_EM);
        }
    }
    (top, bottom)
}

/// `UniformTextStyleBackgroundMetricBox`: fallback faces inside one highlighted run must not change
/// its height. Prefer an ideographic metric for the run's resolved style, then any face carrying the
/// same style, and finally the named em-box fallback used by generic backgrounds.
fn uniform_text_style_vertical_bounds(
    result: &LayoutResult,
    segment: &RichTextLineSegment,
    metrics: &[MetricDecisionInfo],
    style: &TextStyle,
) -> (f32, f32) {
    let matching_style: Vec<_> = metrics
        .iter()
        .filter(|decision| {
            same_font_metric_style(
                &resolved_text_style_at(result, decision.range.start()),
                style,
            )
        })
        .collect();
    let reference = matching_style
        .iter()
        .find(|decision| decision.metric_box == IDEOGRAPHIC_EM_BOX_NAME)
        .copied()
        .or_else(|| matching_style.first().copied());
    let ascent = reference
        .map(|decision| decision.layout_ascent)
        .unwrap_or(style.font_size * BACKGROUND_FALLBACK_ASCENT_EM);
    let descent = reference
        .map(|decision| decision.layout_descent)
        .unwrap_or(style.font_size * BACKGROUND_FALLBACK_DESCENT_EM);
    (segment.baseline - ascent, segment.baseline + descent)
}

fn resolved_text_style_at(result: &LayoutResult, offset: ScalarOffset) -> TextStyle {
    result
        .input
        .content
        .spans
        .iter()
        .rev()
        .find(|span| offset >= span.range.start() && offset < span.range.end())
        .map(|span| span.style.clone())
        .unwrap_or_else(|| result.input.text_style.clone())
}

fn same_font_metric_style(left: &TextStyle, right: &TextStyle) -> bool {
    left.font_families == right.font_families
        && left.font_size == right.font_size
        && left.locale == right.locale
        && left.font_weight == right.font_weight
        && left.italic == right.italic
        && left.baseline_shift == right.baseline_shift
}

const IDEOGRAPHIC_EM_BOX_NAME: &str = "IdeographicEmBox";
const BACKGROUND_FALLBACK_ASCENT_EM: f32 = 0.88;
const BACKGROUND_FALLBACK_DESCENT_EM: f32 = 0.12;

fn trim_outer_punctuation_glue(
    result: &LayoutResult,
    segments: &[RichTextLineSegment],
) -> Vec<RichTextLineSegment> {
    segments
        .iter()
        .cloned()
        .map(|segment| {
            let Some(line) = result.lines.get(segment.line_index as usize) else {
                return segment;
            };
            let line_clusters: Vec<_> = line
                .cluster_range
                .into_iter()
                .filter_map(|index| result.clusters.get(index as usize))
                .collect();
            let first_cluster = line_clusters
                .iter()
                .find(|cluster| cluster.range.end() > segment.range.start());
            let last_cluster = line_clusters
                .iter()
                .rev()
                .find(|cluster| cluster.range.start() < segment.range.end());
            let leading_glue = first_cluster
                .filter(|cluster| segment.range.start() == cluster.range.start())
                .and_then(|cluster| {
                    result
                        .debug
                        .geometry_decisions
                        .iter()
                        .find(|decision| decision.range == cluster.range)
                })
                .map(|decision| {
                    (decision.leading_glue_natural - decision.leading_glue_consumed).max(0.0)
                })
                .unwrap_or(0.0);
            let trailing_glue = last_cluster
                .filter(|cluster| segment.range.end() == cluster.range.end())
                .and_then(|cluster| {
                    result
                        .debug
                        .geometry_decisions
                        .iter()
                        .find(|decision| decision.range == cluster.range)
                })
                .map(|decision| {
                    (decision.trailing_glue_natural - decision.trailing_glue_consumed).max(0.0)
                })
                .unwrap_or(0.0);
            let left = (segment.left + leading_glue).min(segment.right);
            RichTextLineSegment {
                left,
                right: (segment.right - trailing_glue).max(left),
                ..segment
            }
        })
        .collect()
}

/// Resolves the physical center line used by Tiqian's underline/strike-through renderers.
/// Consumers drawing a custom stroke style reuse this query instead of guessing from a line box.
pub fn rich_text_decoration_line_y(
    result: &LayoutResult,
    segment: &RichTextLineSegment,
    stroke_width: f32,
) -> f32 {
    assert!(
        stroke_width.is_finite() && stroke_width >= 0.0,
        "strokeWidth must be finite and non-negative"
    );
    let role = &segment.span.role;
    assert!(
        matches!(role, RichTextRole::Underline | RichTextRole::LineThrough),
        "richTextDecorationLineY only supports underline and line-through segments"
    );
    let style = resolved_text_style_at(result, segment.range.start());
    let raw_line_y = if matches!(role, RichTextRole::Underline) {
        segment.baseline + style.font_size * INTERLINEAR_UNDERLINE_OFFSET_EM
    } else {
        // `IdeographicMetricBoxLineThroughCenter`: a Chinese strike-through bisects the
        // resolved style's declared 字身框. Prefer its real ideographic metric decision; the
        // shared 0.88/0.12 em fallback is used only when the platform supplied no metrics.
        let (face_top, face_bottom) = uniform_text_style_vertical_bounds(
            result,
            segment,
            &result.debug.metric_decisions,
            &style,
        );
        (face_top + face_bottom) / 2.0
    };
    raw_line_y.clamp(
        segment.top + stroke_width / 2.0,
        segment.bottom - stroke_width / 2.0,
    )
}

/// Returns a caret rectangle for `offset`. The x position is derived from Tiqian's cluster advances;
/// inside a multi-code-unit cluster, `SourceRangeLinearClusterSplit` places the caret proportionally.
pub fn get_cursor_rect(result: &LayoutResult, offset: ScalarOffset) -> Rect {
    if result.lines.is_empty() {
        return Rect {
            left: 0.0,
            top: 0.0,
            right: 0.0,
            bottom: 0.0,
        };
    }
    let text_length = result.input.content.text.scalar_len();
    let clamped = nearest_interaction_offset(result, offset.min(text_length));
    let line_index = get_line_for_offset(result, clamped).max(0) as usize;
    let line = &result.lines[line_index];
    let clusters = positioned_clusters_for_line(result, line_index as i32, line);
    let caret_width = 1.0;
    let x = if clusters.is_empty() {
        line.indent
    } else if clamped <= clusters[0].range.start() {
        clusters[0].left
    } else if clamped >= clusters[clusters.len() - 1].range.end() {
        clusters[clusters.len() - 1].right
    } else {
        x_for_offset(
            clusters
                .iter()
                .find(|cluster| clamped >= cluster.range.start() && clamped <= cluster.range.end())
                .expect("source offset must belong to a positioned cluster"),
            clamped,
        )
    };
    Rect {
        left: x,
        top: line.top,
        right: x + caret_width,
        bottom: line.bottom,
    }
}

/// Hit-tests a point against Tiqian line/cluster geometry. `ClusterAdvanceLinearHitTest` chooses the
/// nearest source offset inside a cluster using its occupied advance until glyph-level source maps
/// are available.
pub fn get_offset_for_position(result: &LayoutResult, x: f32, y: f32) -> ScalarOffset {
    if result.lines.is_empty() {
        return ScalarOffset::ZERO;
    }
    let line_index = nearest_line_for_position(result, y);
    let clusters =
        positioned_clusters_for_line(result, line_index as i32, &result.lines[line_index]);
    if clusters.is_empty() {
        return result.lines[line_index].range.start();
    }
    if x <= clusters[0].left {
        return clusters[0].range.start();
    }
    if x >= clusters[clusters.len() - 1].right {
        return clusters[clusters.len() - 1].range.end();
    }
    let cluster = clusters
        .iter()
        .find(|cluster| x >= cluster.left && x <= cluster.right)
        .unwrap_or_else(|| {
            clusters
                .iter()
                .min_by(|left, right| {
                    (x - left.left)
                        .abs()
                        .min((x - left.right).abs())
                        .total_cmp(&(x - right.left).abs().min((x - right.right).abs()))
                })
                .expect("non-empty positioned clusters")
        });
    coerce_selection_offset(
        result,
        offset_for_x(cluster, x),
        SourceBoundaryBias::Nearest,
    )
}

/// Hit-tests a static-text selection endpoint and snaps it to a safe source interaction boundary.
/// This retains per-code-point Latin selection inside word layout atoms without ever returning the
/// middle of a surrogate, combining sequence, emoji ZWJ sequence, or other grouped source unit.
pub fn get_selection_offset_for_position(result: &LayoutResult, x: f32, y: f32) -> ScalarOffset {
    if result.lines.is_empty() {
        return ScalarOffset::ZERO;
    }
    let line_index = nearest_line_for_position(result, y);
    let positioned =
        positioned_clusters_for_line(result, line_index as i32, &result.lines[line_index]);
    if positioned.is_empty() {
        return coerce_selection_offset(
            result,
            result.lines[line_index].range.start(),
            SourceBoundaryBias::Nearest,
        );
    }
    if x <= positioned[0].left {
        return coerce_selection_offset(
            result,
            positioned[0].range.start(),
            SourceBoundaryBias::Nearest,
        );
    }
    if x >= positioned[positioned.len() - 1].right {
        return coerce_selection_offset(
            result,
            positioned[positioned.len() - 1].range.end(),
            SourceBoundaryBias::Nearest,
        );
    }
    let cluster = positioned
        .iter()
        .find(|cluster| x >= cluster.left && x <= cluster.right)
        .unwrap_or_else(|| {
            positioned
                .iter()
                .min_by(|left, right| {
                    (x - left.left)
                        .abs()
                        .min((x - left.right).abs())
                        .total_cmp(&(x - right.left).abs().min((x - right.right).abs()))
                })
                .expect("non-empty positioned clusters")
        });
    let raw_offset = offset_for_x(cluster, x);
    let backward = coerce_selection_offset(result, raw_offset, SourceBoundaryBias::Backward);
    let forward = coerce_selection_offset(result, raw_offset, SourceBoundaryBias::Forward);
    if backward == forward {
        return backward;
    }
    let backward_distance = (get_cursor_rect(result, backward).left - x).abs();
    let forward_distance = (get_cursor_rect(result, forward).left - x).abs();
    if backward_distance < forward_distance {
        backward
    } else {
        forward
    }
}

/// Coerces an external scalar offset to a safe selection/caret boundary.
pub fn coerce_selection_offset(
    result: &LayoutResult,
    offset: ScalarOffset,
    bias: SourceBoundaryBias,
) -> ScalarOffset {
    let text = &result.input.content.text;
    let text_length = text.scalar_len();
    let clamped = offset.min(text_length);
    if let Some(inline_object) = result.input.inline_objects.iter().find(|inline_object| {
        clamped > inline_object.range.start() && clamped < inline_object.range.end()
    }) {
        return match bias {
            SourceBoundaryBias::Backward => inline_object.range.start(),
            SourceBoundaryBias::Forward => inline_object.range.end(),
            SourceBoundaryBias::Nearest => {
                if clamped - inline_object.range.start() < inline_object.range.end() - clamped {
                    inline_object.range.start()
                } else {
                    inline_object.range.end()
                }
            }
        };
    }
    coerce_to_interaction_boundary(
        text,
        clamped,
        TextRange::new(ScalarOffset::ZERO, text_length),
        bias,
    )
}

/// Expands a safe source offset to the word selected by a static-text double click.
/// `SourceInteractionWordExpansion` joins adjacent letter/digit/connector graphemes, keeps Han
/// ideographs individually selectable, groups adjacent whitespace, and otherwise returns exactly
/// one source interaction unit. It does not depend on shaping-cluster width or line-break atoms.
pub fn get_selection_word_boundary(result: &LayoutResult, offset: ScalarOffset) -> TextRange {
    let text = &result.input.content.text;
    if text.is_empty() {
        return TextRange::new(ScalarOffset::ZERO, ScalarOffset::ZERO);
    }
    let text_length = text.scalar_len();
    let clamped =
        coerce_selection_offset(result, offset.min(text_length), SourceBoundaryBias::Nearest);
    if let Some(inline_object) = result.input.inline_objects.iter().find(|inline_object| {
        clamped >= inline_object.range.start() && clamped < inline_object.range.end()
    }) {
        return inline_object.range;
    }
    let boundaries = interaction_boundaries(text, TextRange::new(ScalarOffset::ZERO, text_length));
    let exact_index = boundaries.binary_search(&clamped);
    let unit_index = if clamped == text_length {
        boundaries.len() - 2
    } else if let Ok(index) = exact_index {
        index
    } else {
        let insertion_index = exact_index.unwrap_err();
        insertion_index.saturating_sub(1)
    };
    let kind = selection_word_kind(text, boundaries[unit_index], boundaries[unit_index + 1]);
    if kind == SelectionWordKind::Single {
        return TextRange::new(boundaries[unit_index], boundaries[unit_index + 1]);
    }
    let mut first = unit_index;
    let mut last = unit_index;
    while first > 0 && selection_word_kind(text, boundaries[first - 1], boundaries[first]) == kind {
        first -= 1;
    }
    while last + 2 < boundaries.len()
        && selection_word_kind(text, boundaries[last + 1], boundaries[last + 2]) == kind
    {
        last += 1;
    }
    TextRange::new(boundaries[first], boundaries[last + 1])
}

/// Returns the word/source unit visually under a point. Unlike caret hit testing, a point in the
/// right half of one Han box still belongs to that ideograph instead of the following insertion
/// boundary.
pub fn get_selection_word_boundary_for_position(
    result: &LayoutResult,
    x: f32,
    y: f32,
) -> Option<TextRange> {
    if result.lines.is_empty() || result.input.content.text.is_empty() {
        return None;
    }
    let line_index = nearest_line_for_position(result, y);
    let positioned =
        positioned_clusters_for_line(result, line_index as i32, &result.lines[line_index]);
    if positioned.is_empty() {
        return None;
    }
    let cluster = positioned
        .iter()
        .find(|cluster| x >= cluster.left && x <= cluster.right)
        .unwrap_or_else(|| {
            positioned
                .iter()
                .min_by(|left, right| {
                    (x - left.left)
                        .abs()
                        .min((x - left.right).abs())
                        .total_cmp(&(x - right.left).abs().min((x - right.right).abs()))
                })
                .expect("non-empty positioned clusters")
        });
    if cluster.range.is_empty() {
        return None;
    }
    let source_unit_offset =
        offset_for_x(cluster, x).clamp(cluster.range.start(), cluster.range.end() - 1);
    Some(get_selection_word_boundary(result, source_unit_offset))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SelectionWordKind {
    Word,
    Whitespace,
    Single,
}

fn selection_word_kind(text: &Text, start: ScalarOffset, _end: ScalarOffset) -> SelectionWordKind {
    let Some(code_point) = text.code_point_at_or_none(start) else {
        return SelectionWordKind::Single;
    };
    if SELECTION_MANDATORY_BREAKS.contains(&code_point) {
        return SelectionWordKind::Single;
    }
    let Some(character) = char::from_u32(code_point as u32) else {
        return SelectionWordKind::Single;
    };
    if character.is_whitespace() {
        return SelectionWordKind::Whitespace;
    }
    if is_han_ideograph(code_point) {
        return SelectionWordKind::Single;
    }
    if code_point <= 0xFFFF
        && (character.is_alphabetic()
            || character.is_numeric()
            || SELECTION_WORD_CONNECTORS.contains(&character))
    {
        return SelectionWordKind::Word;
    }
    SelectionWordKind::Single
}

fn is_han_ideograph(code_point: i32) -> bool {
    CodePointSetData::new::<UnifiedIdeograph>().contains32(code_point as u32)
}

const SELECTION_WORD_CONNECTORS: [char; 3] = ['_', '\'', '\u{2019}'];
const SELECTION_MANDATORY_BREAKS: [i32; 5] = [0x000A, 0x000D, 0x0085, 0x2028, 0x2029];

fn positioned_clusters_for_line(
    result: &LayoutResult,
    line_index: i32,
    line: &LineBox,
) -> Vec<PositionedCluster> {
    let mut x = line.indent;
    let mut positioned = Vec::new();
    for (index_in_line, cluster_index) in line.cluster_range.into_iter().enumerate() {
        let cluster = &result.clusters[cluster_index as usize];
        let leading_consumed = result
            .debug
            .geometry_decisions
            .iter()
            .find(|decision| {
                decision.range == cluster.range && decision.leading_glue_consumed > 0.0
            })
            .map(|decision| decision.leading_glue_consumed)
            .unwrap_or(0.0);
        // The applied autospace width is recorded on the decision (an Insert gap is a
        // negative reduction, `AutoSpacePolicy.gapEm` at apply time) — geometry reads
        // the recorded value instead of re-deriving a constant (ADR 0009 amendment).
        let leading_gap = if index_in_line == 0 {
            0.0
        } else {
            result
                .debug
                .auto_space_decisions
                .iter()
                .find(|decision| {
                    decision.side == "leading" && decision.cluster_range == cluster.range
                })
                .map(|decision| -decision.total_reduction)
                .unwrap_or(0.0)
        };
        let draw_x = x + cluster.leading_layout_advance + cluster.glyph_inline_shift + leading_gap
            - leading_consumed;
        // Interior source-offset boundaries from the shaped glyph origins, so a caret/selection
        // endpoint inside a proportional Latin word lands on the real letter edge. Only when the run
        // emitted one glyph per source unit (typical Latin); ligatures/complex runs stay None and
        // callers interpolate linearly. The two ends are always the occupied box edges — a
        // full-width punctuation glyph advancing past its compressed cluster box must not overshoot.
        let right = x + cluster.advance;
        let glyphs: Vec<_> = result
            .glyph_runs
            .iter()
            .flat_map(|run| run.glyphs.iter())
            .filter(|glyph| glyph.cluster_range == cluster.range)
            .collect();
        let source_stops =
            if cluster.range.length() > 1 && glyphs.len() == cluster.range.length() as usize {
                let mut stops = Vec::with_capacity(cluster.range.length() as usize + 1);
                stops.push(x);
                for glyph in glyphs.iter().skip(1) {
                    stops.push((draw_x + glyph.x).clamp(x, right));
                }
                stops.push(right);
                Some(stops)
            } else {
                None
            };
        positioned.push(
            PositionedCluster::builder(
                line_index,
                cluster_index,
                cluster.range,
                x,
                line.top,
                right,
                line.bottom,
                line.baseline + cluster.baseline_shift,
            )
            .draw_x(draw_x)
            .source_stops(source_stops)
            .build(),
        );
        x += cluster.advance;
    }
    with_ruby_selection_geometry(result, positioned, line_index)
}

fn nearest_interaction_offset(result: &LayoutResult, offset: ScalarOffset) -> ScalarOffset {
    coerce_selection_offset(result, offset, SourceBoundaryBias::Nearest)
}

fn nearest_line_for_offset(result: &LayoutResult, offset: ScalarOffset) -> i32 {
    result
        .lines
        .iter()
        .enumerate()
        .min_by_key(|(_, line)| {
            if offset < line.range.start() {
                line.range.start() - offset
            } else if offset > line.range.end() {
                offset - line.range.end()
            } else {
                0
            }
        })
        .map(|(index, _)| index as i32)
        .expect("nearest line requires a non-empty LayoutResult")
}

fn nearest_line_for_position(result: &LayoutResult, y: f32) -> usize {
    result
        .lines
        .iter()
        .enumerate()
        .min_by(|(_, left), (_, right)| {
            let distance = |line: &LineBox| {
                if y < line.top {
                    line.top - y
                } else if y > line.bottom {
                    y - line.bottom
                } else {
                    0.0
                }
            };
            distance(left).total_cmp(&distance(right))
        })
        .map(|(index, _)| index)
        .expect("nearest line requires a non-empty LayoutResult")
}

fn x_for_offset(cluster: &PositionedCluster, offset: ScalarOffset) -> f32 {
    if cluster.range.length() <= 0 {
        return cluster.left;
    }
    let index = (offset - cluster.range.start()).clamp(0, cluster.range.length()) as usize;
    if let Some(stops) = &cluster.source_stops {
        return stops[index];
    }
    cluster.left + cluster.width() * index as f32 / cluster.range.length() as f32
}

fn offset_for_x(cluster: &PositionedCluster, x: f32) -> ScalarOffset {
    if cluster.range.length() <= 0 {
        return cluster.range.start();
    }
    if let Some(stops) = &cluster.source_stops {
        let best = stops
            .iter()
            .enumerate()
            .min_by(|(_, left), (_, right)| (x - **left).abs().total_cmp(&(x - **right).abs()))
            .map(|(index, _)| index as i32)
            .expect("sourceStops is non-empty");
        return (cluster.range.start() + best).clamp(cluster.range.start(), cluster.range.end());
    }
    if cluster.width() <= 0.0 {
        return cluster.range.start();
    }
    let ratio = ((x - cluster.left) / cluster.width()).clamp(0.0, 1.0);
    (cluster.range.start() + (ratio * cluster.range.length() as f32).round() as i32)
        .clamp(cluster.range.start(), cluster.range.end())
}

fn slice_rect(cluster: &PositionedCluster, start: ScalarOffset, end: ScalarOffset) -> Rect {
    if cluster.range.length() <= 0 || cluster.width() <= 0.0 {
        return cluster.rect();
    }
    Rect {
        left: x_for_offset(cluster, start),
        top: cluster.top,
        right: x_for_offset(cluster, end),
        bottom: cluster.bottom,
    }
}

fn natural_advance_by_range(result: &LayoutResult) -> Vec<(TextRange, f32)> {
    let mut out: Vec<(TextRange, f32)> = Vec::new();
    for glyph in result.glyph_runs.iter().flat_map(|run| run.glyphs.iter()) {
        if let Some((_, advance)) = out
            .iter_mut()
            .find(|(range, _)| *range == glyph.cluster_range)
        {
            *advance += glyph.advance;
        } else {
            out.push((glyph.cluster_range, glyph.advance));
        }
    }
    out
}

fn ruby_spread_by_range(result: &LayoutResult) -> Vec<(TextRange, f32)> {
    result
        .debug
        .geometry_decisions
        .iter()
        .filter(|decision| decision.ruby_spread != 0.0)
        .map(|decision| (decision.range, decision.ruby_spread))
        .collect()
}

#[derive(Clone, Copy, Debug)]
struct SelectionBounds {
    left: f32,
    right: f32,
}

fn with_ruby_selection_geometry(
    result: &LayoutResult,
    positioned: Vec<PositionedCluster>,
    line_index: i32,
) -> Vec<PositionedCluster> {
    let rubies: Vec<_> = result
        .debug
        .ruby_decisions
        .iter()
        .filter(|ruby| ruby.line_index == line_index && ruby.width > 0.0)
        .collect();
    if rubies.is_empty() {
        return positioned;
    }

    let natural_advances = natural_advance_by_range(result);
    let ruby_spreads = ruby_spread_by_range(result);
    let mut bounds: Vec<_> = positioned
        .iter()
        .map(|cluster| {
            let ruby_spread = ruby_spreads
                .iter()
                .find(|(range, _)| *range == cluster.range)
                .map(|(_, spread)| *spread)
                .unwrap_or(0.0);
            SelectionBounds {
                left: cluster.left,
                right: (cluster.right - ruby_spread).max(cluster.left),
            }
        })
        .collect();
    let center_of = |cluster: &PositionedCluster| {
        let natural = natural_advances
            .iter()
            .find(|(range, _)| *range == cluster.range)
            .map(|(_, advance)| *advance)
            .unwrap_or_else(|| {
                cluster.width()
                    - ruby_spreads
                        .iter()
                        .find(|(range, _)| *range == cluster.range)
                        .map(|(_, spread)| *spread)
                        .unwrap_or(0.0)
            });
        cluster.draw_x + natural.max(0.0) / 2.0
    };

    for ruby in rubies {
        let base_indices: Vec<_> = positioned
            .iter()
            .enumerate()
            .filter(|(_, cluster)| {
                cluster.range.start() >= ruby.base_range.start()
                    && cluster.range.end() <= ruby.base_range.end()
            })
            .map(|(index, _)| index)
            .collect();
        if base_indices.is_empty() {
            continue;
        }
        let centers: Vec<_> = base_indices
            .iter()
            .map(|index| center_of(&positioned[*index]))
            .collect();
        let ruby_left = ruby.center_x - ruby.width / 2.0;
        let ruby_right = ruby.center_x + ruby.width / 2.0;
        for (index, base_index) in base_indices.iter().enumerate() {
            let segment_left = if index == 0 {
                ruby_left
            } else {
                ruby_left.max((centers[index - 1] + centers[index]) / 2.0)
            };
            let segment_right = if index == base_indices.len() - 1 {
                ruby_right
            } else {
                ruby_right.min((centers[index] + centers[index + 1]) / 2.0)
            };
            let bound = &mut bounds[*base_index];
            bound.left = bound.left.min(segment_left);
            bound.right = bound.right.max(segment_right);
        }
    }

    for index in 0..bounds.len().saturating_sub(1) {
        let (left_bounds, right_bounds) = bounds.split_at_mut(index + 1);
        let left = &mut left_bounds[index];
        let right = &mut right_bounds[0];
        if left.right <= right.left {
            continue;
        }
        let boundary = ((center_of(&positioned[index]) + center_of(&positioned[index + 1])) / 2.0)
            .clamp(left.left.min(right.left), left.right.max(right.right));
        left.right = left.right.min(boundary).max(left.left);
        right.left = right.left.max(boundary).min(right.right);
    }

    positioned
        .into_iter()
        .enumerate()
        .map(|(index, cluster)| {
            let bound = bounds[index];
            // Ruby redistributes occupied boxes to fill the annotation width, so absolute per-source
            // glyph stops no longer align; selection and bounding boxes interpolate linearly over the
            // redistributed box on ruby lines.
            PositionedCluster {
                left: bound.left,
                right: bound.right,
                source_stops: None,
                ..cluster
            }
        })
        .collect()
}
