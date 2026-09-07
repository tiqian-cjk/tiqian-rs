use crate::common::{HashMap, HashSet};
use crate::core::geometry::{LayoutConstraints, ScalarOffset, TextRange};
use crate::core::text::Text;
use crate::core::text_model::{
    built_in_layout_profiles, DecorationSpan, InlineBoxOuterSpacing, InlineBoxSpan,
    InlineObjectSpan, LayoutInput, LayoutProfileId, LineBreakSpan, ParagraphStyle,
    RichTextLayer, RichTextLayerKind, RichTextPaint, RichTextSpan, RubySpan, TextSpan,
    TextStyle, TiqianTextContent,
};

use super::types::{ParagraphBuildError, ParagraphBuildOutput};

pub struct ParagraphBuilder {
    pub(super) source: String,
    pub(super) scalar_offset: ScalarOffset,
    pub(super) constraints: LayoutConstraints,
    pub(super) text_style: TextStyle,
    pub(super) paragraph_style: ParagraphStyle,
    pub(super) profile_id: LayoutProfileId,
    pub(super) spans: Vec<TextSpan>,
    pub(super) line_break_spans: Vec<(u64, LineBreakSpan)>,
    pub(super) auto_space_suppressed_ranges: Vec<(u64, TextRange)>,
    pub(super) decorations: Vec<(u64, DecorationSpan)>,
    pub(super) ruby_spans: Vec<(u64, RubySpan)>,
    pub(super) inline_boxes: Vec<(u64, InlineBoxSpan)>,
    pub(super) inline_objects: Vec<InlineObjectSpan>,
    pub(super) paints: Vec<RichTextPaint>,
    pub(super) rich_text: Vec<(u64, RichTextSpan)>,
    pub(super) open_scopes: Vec<super::scopes::OpenScope>,
    pub(super) next_scope_sequence: u64,
    pub(super) next_rich_text_sequence: u64,
    pub(super) first_error: Option<ParagraphBuildError>,
}

impl ParagraphBuilder {
    pub fn new(constraints: LayoutConstraints) -> Self {
        Self {
            source: String::new(),
            scalar_offset: ScalarOffset::ZERO,
            constraints,
            text_style: TextStyle::default(),
            paragraph_style: ParagraphStyle::default(),
            profile_id: built_in_layout_profiles::clreq_horizontal(),
            spans: Vec::new(),
            line_break_spans: Vec::new(),
            auto_space_suppressed_ranges: Vec::new(),
            decorations: Vec::new(),
            ruby_spans: Vec::new(),
            inline_boxes: Vec::new(),
            inline_objects: Vec::new(),
            paints: vec![RichTextPaint::default()],
            rich_text: Vec::new(),
            open_scopes: Vec::new(),
            next_scope_sequence: 0,
            next_rich_text_sequence: 0,
            first_error: None,
        }
    }

    pub fn text_style(&mut self, style: TextStyle) -> &mut Self {
        self.assert_paragraph_configuration_is_mutable();
        self.text_style = style;
        self
    }

    /// 设置正文默认使用的 paint；文本写入后不能再修改段落级默认值。
    pub fn paints(&mut self, paints: &[RichTextPaint]) -> &mut Self {
        self.assert_paragraph_configuration_is_mutable();
        self.paints = paints.to_vec();
        self
    }

    pub fn paragraph_style(&mut self, style: ParagraphStyle) -> &mut Self {
        self.assert_paragraph_configuration_is_mutable();
        self.paragraph_style = style;
        self
    }

    pub fn profile_id(&mut self, profile_id: LayoutProfileId) -> &mut Self {
        self.assert_paragraph_configuration_is_mutable();
        self.profile_id = profile_id;
        self
    }

    /// 追加 source text，并为这一段文本记录当前生效的样式、layer 和语义。
    pub fn push(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        let start = self.scalar_offset;
        self.source.push_str(text);
        self.scalar_offset += text.chars().count() as i32;
        let range = TextRange::new(start, self.scalar_offset);
        let style = self.current_text_style();
        if style != self.text_style {
            if let Some(previous) = self
                .spans
                .last_mut()
                .filter(|span| span.range.end() == range.start() && span.style == style)
            {
                previous.range = TextRange::new(previous.range.start(), range.end());
            } else {
                self.spans.push(TextSpan { range, style });
            }
        }
        self.record_text_layers(range);
        self.record_active_decoration_layers(range);
        self.record_active_annotation_layers(range);
    }

    /// 校验 builder 状态，并将 source、layout 输入和 rich-text side channel 分别组装完成。
    pub fn build(self) -> Result<ParagraphBuildOutput, ParagraphBuildError> {
        if let Some(error) = self.first_error {
            return Err(error);
        }
        if !self.open_scopes.is_empty() {
            return Err(ParagraphBuildError::UnclosedScopes {
                scopes: self
                    .open_scopes
                    .iter()
                    .map(|scope| scope.kind.public_kind())
                    .collect(),
            });
        }
        let line_break_spans = Self::ordered_line_break_spans(self.line_break_spans);
        let auto_space_suppressed_ranges =
            Self::ordered_auto_space_suppressed_ranges(self.auto_space_suppressed_ranges);
        let rich_text = Self::normalized_rich_text(self.rich_text);
        let mut source_boundaries = HashSet::new();
        let mut inline_boxes = self.inline_boxes;
        // rich-text 本身保留在 side channel 中供渲染使用；这里只提取会影响布局几何的
        // 范围边界和背景水平 padding，将后者转换为布局可处理的 inline box。
        for (sequence, span) in &rich_text {
            source_boundaries.insert(span.range.start());
            source_boundaries.insert(span.range.end());
            for layer in &span.layers {
                let RichTextLayerKind::Background { background } = &layer.kind else {
                    continue;
                };
                if background.horizontal_padding > 0.0 {
                    inline_boxes.push((
                        *sequence,
                        InlineBoxSpan::with_all(
                            span.range,
                            background.horizontal_padding,
                            background.horizontal_padding,
                            InlineBoxOuterSpacing::Narrow,
                        ),
                    ));
                }
            }
        }
        let content = TiqianTextContent::builder(Text::from(self.source))
            .spans(self.spans)
            .source_boundaries(source_boundaries)
            .line_break_spans(line_break_spans)
            .auto_space_suppressed_ranges(auto_space_suppressed_ranges)
            .build();
        Ok(ParagraphBuildOutput {
            input: LayoutInput::builder(content, self.constraints)
                .text_style(self.text_style)
                .paragraph_style(self.paragraph_style)
                .profile_id(self.profile_id)
                .decorations(Self::ordered_values(self.decorations))
                .ruby_spans(Self::ordered_values(self.ruby_spans))
                .inline_boxes(Self::ordered_values(inline_boxes))
                .inline_objects(self.inline_objects)
                .build(),
            rich_text: rich_text.into_iter().map(|(_, span)| span).collect(),
        })
    }

    pub(super) fn ordered_values<T>(mut values: Vec<(u64, T)>) -> Vec<T> {
        values.sort_by_key(|(sequence, _)| *sequence);
        values.into_iter().map(|(_, value)| value).collect()
    }

    fn ordered_line_break_spans(values: Vec<(u64, LineBreakSpan)>) -> Vec<LineBreakSpan> {
        let mut seen = HashSet::new();
        Self::ordered_values(values)
            .into_iter()
            .filter(|span| seen.insert((span.range, span.policy)))
            .collect()
    }

    fn ordered_auto_space_suppressed_ranges(values: Vec<(u64, TextRange)>) -> Vec<TextRange> {
        let mut seen = HashSet::new();
        Self::ordered_values(values)
            .into_iter()
            .filter(|range| seen.insert(*range))
            .collect()
    }

    pub(super) fn current_text_style(&self) -> TextStyle {
        self.open_scopes.iter().fold(self.text_style.clone(), |style, scope| {
            match &scope.kind {
                super::scopes::OpenScopeKind::TextStyle(override_style)
                | super::scopes::OpenScopeKind::InlineCode(override_style, _) => {
                    override_style.apply_to(&style)
                }
                _ => style,
            }
        })
    }

    /// 返回最近一个 paint scope 的 paint；没有局部 scope 时使用段落默认 paint。
    pub(super) fn current_paints(&self) -> Vec<RichTextPaint> {
        self.open_scopes
            .iter()
            .rev()
            .find_map(|scope| match &scope.kind {
                super::scopes::OpenScopeKind::Paints(paints) => Some(paints.clone()),
                _ => None,
            })
            .unwrap_or_else(|| self.paints.clone())
    }

    /// 从最近的 rich-text scope 中筛选指定类型的 layer。
    pub(super) fn current_layers(&self, kind: &RichTextLayerKind) -> Vec<RichTextLayer> {
        self.open_scopes
            .iter()
            .rev()
            .find_map(|scope| match &scope.kind {
                super::scopes::OpenScopeKind::RichText(layers) => {
                    let matching = layers
                        .iter()
                        .filter(|layer| Self::layer_kind_matches(&layer.kind, kind))
                        .cloned()
                        .collect::<Vec<_>>();
                    (!matching.is_empty()).then_some(matching)
                }
                _ => None,
            })
            .unwrap_or_default()
    }

    /// 记录一段 rich-text；空范围和没有内容的 span 不进入输出。
    pub(super) fn record_rich_text(
        &mut self,
        range: TextRange,
        layers: Vec<RichTextLayer>,
        semantics: Vec<crate::core::text_model::RichTextSemantic>,
    ) {
        if range.is_empty() || (layers.is_empty() && semantics.is_empty()) {
            return;
        }
        let sequence = self.next_rich_text_sequence;
        self.next_rich_text_sequence += 1;
        self.rich_text.push((
            sequence,
            RichTextSpan {
                range,
                layers,
                semantics,
            },
        ));
    }

    /// 为新写入的文本生成正文 layer，并附加当前 scope 声明的背景和线 layer。
    fn record_text_layers(&mut self, range: TextRange) {
        let mut layers = self.current_layers(&RichTextLayerKind::Text);
        if layers.is_empty() {
            layers.push(RichTextLayer {
                kind: RichTextLayerKind::Text,
                paints: self.current_paints(),
            });
        }
        for kind in [
            RichTextLayerKind::Background {
                background: crate::core::text_model::RichTextBackgroundPaint::default(),
            },
            RichTextLayerKind::Underline {
                line: crate::core::text_model::RichTextLinePaint::default(),
            },
            RichTextLayerKind::LineThrough {
                line: crate::core::text_model::RichTextLinePaint::default(),
            },
        ] {
            layers.extend(self.current_layers(&kind));
        }
        self.record_rich_text(range, layers, Vec::new());
    }

    /// 把当前打开的 decoration scope 记录为覆盖当前文本范围的 layer。
    fn record_active_decoration_layers(&mut self, range: TextRange) {
        let kinds = self
            .open_scopes
            .iter()
            .filter_map(|scope| match scope.kind {
                super::scopes::OpenScopeKind::Decoration(kind) => Some(kind),
                _ => None,
            })
            .collect::<Vec<_>>();
        for kind in kinds {
            self.record_object_layer(range, RichTextLayerKind::Decoration { kind });
        }
    }

    /// 把当前打开的 ruby scope 记录为覆盖当前文本范围的 annotation layer。
    fn record_active_annotation_layers(&mut self, range: TextRange) {
        let kinds = self
            .open_scopes
            .iter()
            .filter_map(|scope| match &scope.kind {
                super::scopes::OpenScopeKind::Ruby(annotation) => Some(annotation.kind),
                _ => None,
            })
            .collect::<Vec<_>>();
        for kind in kinds {
            self.record_object_layer(range, RichTextLayerKind::Annotation { kind });
        }
    }

    /// 按声明顺序整理 rich-text，并合并可连续合并的 span 或相同范围的 layer。
    fn normalized_rich_text(mut values: Vec<(u64, RichTextSpan)>) -> Vec<(u64, RichTextSpan)> {
        values.sort_by_key(|(sequence, _)| *sequence);
        let mut merged = Vec::<(u64, RichTextSpan)>::new();
        let mut index_by_range = HashMap::<TextRange, usize>::new();
        for (sequence, span) in values {
            let last_index = merged.len().saturating_sub(1);
            if let Some((_, previous)) = merged.last_mut()
                && previous.semantics.is_empty()
                && span.semantics.is_empty()
                && previous.layers == span.layers
                && previous.range.end() == span.range.start()
            {
                let previous_range = previous.range;
                previous.range = TextRange::new(previous.range.start(), span.range.end());
                index_by_range.remove(&previous_range);
                index_by_range.insert(previous.range, last_index);
                continue;
            }
            if let Some(&index) = index_by_range.get(&span.range) {
                let (_, previous) = &mut merged[index];
                previous.layers.extend(span.layers);
                previous.semantics.extend(span.semantics);
                continue;
            }
            index_by_range.insert(span.range, merged.len());
            merged.push((sequence, span));
        }
        merged
    }

    /// 判断两个 layer 是否属于同一类 scope；带 kind 的 layer 还需要匹配具体 kind。
    fn layer_kind_matches(left: &RichTextLayerKind, right: &RichTextLayerKind) -> bool {
        matches!(
            (left, right),
            (RichTextLayerKind::Text, RichTextLayerKind::Text)
                | (RichTextLayerKind::Background { .. }, RichTextLayerKind::Background { .. })
                | (RichTextLayerKind::Underline { .. }, RichTextLayerKind::Underline { .. })
                | (RichTextLayerKind::LineThrough { .. }, RichTextLayerKind::LineThrough { .. })
        ) || matches!(
            (left, right),
            (
                RichTextLayerKind::Decoration { kind: left_kind },
                RichTextLayerKind::Decoration { kind: right_kind },
            ) if left_kind == right_kind
        ) || matches!(
            (left, right),
            (
                RichTextLayerKind::Annotation { kind: left_kind },
                RichTextLayerKind::Annotation { kind: right_kind },
            ) if left_kind == right_kind
        )
    }

    /// 记录 decoration 或 annotation 等非正文 layer，并沿用当前 paint。
    fn record_object_layer(&mut self, range: TextRange, kind: RichTextLayerKind) {
        let mut layers = self.current_layers(&kind);
        if layers.is_empty() {
            layers.push(RichTextLayer {
                kind,
                paints: self.current_paints(),
            });
        }
        self.record_rich_text(range, layers, Vec::new());
    }

    pub(super) fn record_error(&mut self, error: ParagraphBuildError) {
        if self.first_error.is_none() {
            self.first_error = Some(error);
        }
    }

    pub(super) fn assert_paragraph_configuration_is_mutable(&self) {
        assert!(
            self.scalar_offset == ScalarOffset::ZERO,
            "paragraph defaults cannot change after source text has been appended"
        );
    }

}
