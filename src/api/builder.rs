use crate::common::HashSet;
use crate::core::geometry::{LayoutConstraints, ScalarOffset, TextRange};
use crate::core::text::Text;
use crate::core::text_model::{
    built_in_layout_profiles, ColorSpan, DecorationSpan, InlineBoxSpan, InlineObjectSpan,
    LayoutInput, LayoutProfileId, LineBreakSpan, ParagraphStyle, RichTextSpan, RubySpan,
    TextSpan, TextStyle, TiqianTextContent,
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
    pub(super) source_boundaries: HashSet<ScalarOffset>,
    pub(super) colors: Vec<(u64, ColorSpan)>,
    pub(super) rich_text: Vec<(u64, RichTextSpan)>,
    pub(super) open_scopes: Vec<super::scopes::OpenScope>,
    pub(super) next_scope_sequence: u64,
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
            source_boundaries: HashSet::new(),
            colors: Vec::new(),
            rich_text: Vec::new(),
            open_scopes: Vec::new(),
            next_scope_sequence: 0,
            first_error: None,
        }
    }

    pub fn text_style(&mut self, style: TextStyle) -> &mut Self {
        self.assert_paragraph_configuration_is_mutable();
        self.text_style = style;
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
    }

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
        let content = TiqianTextContent::builder(Text::from(self.source))
            .spans(self.spans)
            .source_boundaries(self.source_boundaries)
            .line_break_spans(Self::ordered_values(self.line_break_spans))
            .auto_space_suppressed_ranges(Self::ordered_values(self.auto_space_suppressed_ranges))
            .build();
        Ok(ParagraphBuildOutput {
            input: LayoutInput::builder(content, self.constraints)
                .text_style(self.text_style)
                .paragraph_style(self.paragraph_style)
                .profile_id(self.profile_id)
                .decorations(Self::ordered_values(self.decorations))
                .ruby_spans(Self::ordered_values(self.ruby_spans))
                .inline_boxes(Self::ordered_values(self.inline_boxes))
                .inline_objects(self.inline_objects)
                .build(),
            colors: Self::ordered_values(self.colors),
            rich_text: Self::ordered_values(self.rich_text),
        })
    }

    pub(super) fn ordered_values<T>(mut values: Vec<(u64, T)>) -> Vec<T> {
        values.sort_by_key(|(sequence, _)| *sequence);
        values.into_iter().map(|(_, value)| value).collect()
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

    pub(super) fn add_source_boundaries(&mut self, range: TextRange) {
        self.source_boundaries.insert(range.start());
        self.source_boundaries.insert(range.end());
    }

}
