use crate::core::geometry::TextRange;
use crate::core::text::Text;
use crate::core::text_model::{
    ColorSpan, DecorationKind, DecorationSpan, InlineBoxSpan, InlineObjectSpan, LineBreakPolicy,
    LineBreakSpan, RichTextPaint, RichTextRole, RichTextSpan, RubySpan,
};

use super::builder::ParagraphBuilder;
use super::style::{InlineBoxStyle, InlineObjectMetrics, RubyAnnotation, TextStyleOverride};
use super::types::{ParagraphBuildError, ParagraphPositionInsertionKind, ParagraphScopeKind};

pub(super) struct OpenScope {
    pub(super) start: crate::core::geometry::ScalarOffset,
    pub(super) sequence: u64,
    pub(super) kind: OpenScopeKind,
}

pub(super) enum OpenScopeKind {
    TextStyle(TextStyleOverride),
    Ruby(RubyAnnotation),
    InlineBox(InlineBoxStyle),
    Decoration(DecorationKind),
    Color(i32),
    RichText(RichTextRole, RichTextPaint),
    Link(String),
    Technical,
    InlineCode(TextStyleOverride, RichTextPaint),
    AutoSpaceSuppressed,
}

impl OpenScopeKind {
    pub(super) fn public_kind(&self) -> ParagraphScopeKind {
        match self {
            Self::TextStyle(_) => ParagraphScopeKind::TextStyle,
            Self::Ruby(_) => ParagraphScopeKind::Ruby,
            Self::InlineBox(_) => ParagraphScopeKind::InlineBox,
            Self::Decoration(_) => ParagraphScopeKind::Decoration,
            Self::Color(_) => ParagraphScopeKind::Color,
            Self::RichText(_, _) => ParagraphScopeKind::RichText,
            Self::Link(_) => ParagraphScopeKind::Link,
            Self::Technical => ParagraphScopeKind::Technical,
            Self::InlineCode(_, _) => ParagraphScopeKind::InlineCode,
            Self::AutoSpaceSuppressed => ParagraphScopeKind::AutoSpaceSuppressed,
        }
    }
}

impl ParagraphBuilder {
    pub fn push_text_style(
        &mut self,
        style: TextStyleOverride,
    ) -> Result<(), ParagraphBuildError> {
        self.push_scope(OpenScopeKind::TextStyle(style));
        Ok(())
    }

    pub fn push_ruby(&mut self, annotation: RubyAnnotation) -> Result<(), ParagraphBuildError> {
        self.push_scope(OpenScopeKind::Ruby(annotation));
        Ok(())
    }

    pub fn push_inline_box(&mut self, style: InlineBoxStyle) -> Result<(), ParagraphBuildError> {
        self.push_scope(OpenScopeKind::InlineBox(style));
        Ok(())
    }

    pub fn push_decoration(&mut self, kind: DecorationKind) -> Result<(), ParagraphBuildError> {
        self.push_scope(OpenScopeKind::Decoration(kind));
        Ok(())
    }

    pub fn push_color(&mut self, argb: i32) -> Result<(), ParagraphBuildError> {
        self.push_scope(OpenScopeKind::Color(argb));
        Ok(())
    }

    pub fn push_rich_text(
        &mut self,
        role: RichTextRole,
        paint: RichTextPaint,
    ) -> Result<(), ParagraphBuildError> {
        self.push_scope(OpenScopeKind::RichText(role, paint));
        Ok(())
    }

    pub fn push_link(&mut self, target: String) -> Result<(), ParagraphBuildError> {
        self.push_scope(OpenScopeKind::Link(target));
        Ok(())
    }

    pub fn push_technical(&mut self) -> Result<(), ParagraphBuildError> {
        self.push_scope(OpenScopeKind::Technical);
        Ok(())
    }

    pub fn push_inline_code(
        &mut self,
        style: TextStyleOverride,
        paint: RichTextPaint,
    ) -> Result<(), ParagraphBuildError> {
        self.push_scope(OpenScopeKind::InlineCode(style, paint));
        Ok(())
    }

    pub fn push_auto_space_suppressed(&mut self) -> Result<(), ParagraphBuildError> {
        self.push_scope(OpenScopeKind::AutoSpaceSuppressed);
        Ok(())
    }

    pub fn hard_break(&mut self) -> Result<(), ParagraphBuildError> {
        if let Err(error) =
            self.ensure_position_insertion_is_allowed(ParagraphPositionInsertionKind::HardBreak)
        {
            self.record_error(error.clone());
            return Err(error);
        }
        self.push("\n");
        Ok(())
    }

    pub fn inline_object(
        &mut self,
        replacement_text: &str,
        metrics: InlineObjectMetrics,
    ) -> Result<(), ParagraphBuildError> {
        if let Err(error) =
            self.ensure_position_insertion_is_allowed(ParagraphPositionInsertionKind::InlineObject)
        {
            self.record_error(error.clone());
            return Err(error);
        }
        if replacement_text.is_empty() {
            let error = ParagraphBuildError::EmptyInlineObjectReplacementText;
            self.record_error(error.clone());
            return Err(error);
        }
        let start = self.scalar_offset;
        self.push(replacement_text);
        self.inline_objects.push(InlineObjectSpan::new(
            TextRange::new(start, self.scalar_offset),
            metrics.advance,
            metrics.ascent,
            metrics.descent,
            metrics.leading_boundary,
            metrics.trailing_boundary,
        ));
        Ok(())
    }

    pub fn pop(&mut self) -> Result<(), ParagraphBuildError> {
        let Some(scope) = self.open_scopes.pop() else {
            let error = ParagraphBuildError::EmptyScopeStack;
            self.record_error(error.clone());
            return Err(error);
        };
        let result = self.finish_scope(scope);
        if let Err(error) = &result {
            self.record_error(error.clone());
        }
        result
    }

    pub fn with_text_style(
        &mut self,
        style: TextStyleOverride,
        content: impl FnOnce(&mut Self),
    ) {
        self.with_scope(OpenScopeKind::TextStyle(style), content);
    }

    pub fn try_with_text_style(
        &mut self,
        style: TextStyleOverride,
        content: impl FnOnce(&mut Self) -> Result<(), ParagraphBuildError>,
    ) -> Result<(), ParagraphBuildError> {
        self.try_with_scope(OpenScopeKind::TextStyle(style), content)
    }

    pub fn with_ruby(&mut self, annotation: RubyAnnotation, content: impl FnOnce(&mut Self)) {
        self.with_scope(OpenScopeKind::Ruby(annotation), content);
    }

    pub fn try_with_ruby(
        &mut self,
        annotation: RubyAnnotation,
        content: impl FnOnce(&mut Self) -> Result<(), ParagraphBuildError>,
    ) -> Result<(), ParagraphBuildError> {
        self.try_with_scope(OpenScopeKind::Ruby(annotation), content)
    }

    pub fn with_decoration(&mut self, kind: DecorationKind, content: impl FnOnce(&mut Self)) {
        self.with_scope(OpenScopeKind::Decoration(kind), content);
    }

    pub fn try_with_decoration(
        &mut self,
        kind: DecorationKind,
        content: impl FnOnce(&mut Self) -> Result<(), ParagraphBuildError>,
    ) -> Result<(), ParagraphBuildError> {
        self.try_with_scope(OpenScopeKind::Decoration(kind), content)
    }

    pub fn with_inline_box(&mut self, style: InlineBoxStyle, content: impl FnOnce(&mut Self)) {
        self.with_scope(OpenScopeKind::InlineBox(style), content);
    }

    pub fn try_with_inline_box(
        &mut self,
        style: InlineBoxStyle,
        content: impl FnOnce(&mut Self) -> Result<(), ParagraphBuildError>,
    ) -> Result<(), ParagraphBuildError> {
        self.try_with_scope(OpenScopeKind::InlineBox(style), content)
    }

    pub fn with_color(&mut self, argb: i32, content: impl FnOnce(&mut Self)) {
        self.with_scope(OpenScopeKind::Color(argb), content);
    }

    pub fn try_with_color(
        &mut self,
        argb: i32,
        content: impl FnOnce(&mut Self) -> Result<(), ParagraphBuildError>,
    ) -> Result<(), ParagraphBuildError> {
        self.try_with_scope(OpenScopeKind::Color(argb), content)
    }

    pub fn with_rich_text(
        &mut self,
        role: RichTextRole,
        paint: RichTextPaint,
        content: impl FnOnce(&mut Self),
    ) {
        self.with_scope(OpenScopeKind::RichText(role, paint), content);
    }

    pub fn try_with_rich_text(
        &mut self,
        role: RichTextRole,
        paint: RichTextPaint,
        content: impl FnOnce(&mut Self) -> Result<(), ParagraphBuildError>,
    ) -> Result<(), ParagraphBuildError> {
        self.try_with_scope(OpenScopeKind::RichText(role, paint), content)
    }

    pub fn with_link(&mut self, target: String, content: impl FnOnce(&mut Self)) {
        self.with_scope(OpenScopeKind::Link(target), content);
    }

    pub fn try_with_link(
        &mut self,
        target: String,
        content: impl FnOnce(&mut Self) -> Result<(), ParagraphBuildError>,
    ) -> Result<(), ParagraphBuildError> {
        self.try_with_scope(OpenScopeKind::Link(target), content)
    }

    pub fn with_technical(&mut self, content: impl FnOnce(&mut Self)) {
        self.with_scope(OpenScopeKind::Technical, content);
    }

    pub fn try_with_technical(
        &mut self,
        content: impl FnOnce(&mut Self) -> Result<(), ParagraphBuildError>,
    ) -> Result<(), ParagraphBuildError> {
        self.try_with_scope(OpenScopeKind::Technical, content)
    }

    pub fn with_inline_code(
        &mut self,
        style: TextStyleOverride,
        paint: RichTextPaint,
        content: impl FnOnce(&mut Self),
    ) {
        self.with_scope(OpenScopeKind::InlineCode(style, paint), content);
    }

    pub fn try_with_inline_code(
        &mut self,
        style: TextStyleOverride,
        paint: RichTextPaint,
        content: impl FnOnce(&mut Self) -> Result<(), ParagraphBuildError>,
    ) -> Result<(), ParagraphBuildError> {
        self.try_with_scope(OpenScopeKind::InlineCode(style, paint), content)
    }

    pub fn with_auto_space_suppressed(&mut self, content: impl FnOnce(&mut Self)) {
        self.with_scope(OpenScopeKind::AutoSpaceSuppressed, content);
    }

    pub fn try_with_auto_space_suppressed(
        &mut self,
        content: impl FnOnce(&mut Self) -> Result<(), ParagraphBuildError>,
    ) -> Result<(), ParagraphBuildError> {
        self.try_with_scope(OpenScopeKind::AutoSpaceSuppressed, content)
    }

    fn push_scope(&mut self, kind: OpenScopeKind) -> u64 {
        let sequence = self.next_scope_sequence;
        self.next_scope_sequence += 1;
        self.open_scopes.push(OpenScope {
            start: self.scalar_offset,
            sequence,
            kind,
        });
        sequence
    }

    fn finish_closure_scope(&mut self, sequence: u64, scope_kind: ParagraphScopeKind) {
        if self.open_scopes.last().is_some_and(|scope| scope.sequence == sequence) {
            let scope = self.open_scopes.pop().expect("scope stack was checked");
            if let Err(error) = self.finish_scope(scope) {
                self.record_error(error);
            }
        } else {
            self.record_error(ParagraphBuildError::ClosureScopeBoundary { scope: scope_kind });
        }
    }

    fn finish_scope(&mut self, scope: OpenScope) -> Result<(), ParagraphBuildError> {
        let range = TextRange::new(scope.start, self.scalar_offset);
        match scope.kind {
            OpenScopeKind::TextStyle(_) => Ok(()),
            OpenScopeKind::Ruby(annotation) => {
                if range.is_empty() {
                    return Err(ParagraphBuildError::EmptyScope {
                        scope: ParagraphScopeKind::Ruby,
                    });
                }
                self.ruby_spans.push((
                    scope.sequence,
                    RubySpan::builder(range, annotation.text)
                        .font_families(annotation.font_families)
                        .kind(annotation.kind)
                        .locale(annotation.locale)
                        .build(),
                ));
                Ok(())
            }
            OpenScopeKind::InlineBox(style) => {
                if range.is_empty() {
                    return Err(ParagraphBuildError::EmptyScope {
                        scope: ParagraphScopeKind::InlineBox,
                    });
                }
                self.inline_boxes.push((
                    scope.sequence,
                    InlineBoxSpan::with_all(
                        range,
                        style.inline_start,
                        style.inline_end,
                        style.outer_spacing,
                    ),
                ));
                Ok(())
            }
            OpenScopeKind::Decoration(kind) => {
                if !range.is_empty() {
                    self.decorations
                        .push((scope.sequence, DecorationSpan { range, kind }));
                }
                Ok(())
            }
            OpenScopeKind::Color(argb) => {
                if !range.is_empty() {
                    self.colors.push((
                        scope.sequence,
                        ColorSpan {
                            start: range.start(),
                            end: range.end(),
                            argb,
                        },
                    ));
                    self.add_source_boundaries(range);
                }
                Ok(())
            }
            OpenScopeKind::RichText(role, paint) => {
                if !range.is_empty() {
                    self.rich_text.push((
                        scope.sequence,
                        RichTextSpan::with_paint(range, role, paint),
                    ));
                    self.add_source_boundaries(range);
                }
                Ok(())
            }
            OpenScopeKind::Link(target) => {
                if !range.is_empty() {
                    self.rich_text.push((
                        scope.sequence,
                        RichTextSpan::new(range, RichTextRole::Link { target: target.clone() }),
                    ));
                    self.add_source_boundaries(range);
                    let visible_text = Text::from(self.source.as_str()).slice_text(range);
                    if crate::core::text_model::link_address_display::displays_address(
                        &visible_text,
                        &target,
                    ) {
                        self.line_break_spans.push((
                            scope.sequence,
                            LineBreakSpan {
                                range,
                                policy: LineBreakPolicy::ProgressiveTechnical,
                            },
                        ));
                    }
                }
                Ok(())
            }
            OpenScopeKind::Technical => {
                if !range.is_empty() {
                    self.line_break_spans.push((
                        scope.sequence,
                        LineBreakSpan {
                            range,
                            policy: LineBreakPolicy::ProgressiveTechnical,
                        },
                    ));
                    self.auto_space_suppressed_ranges.push((scope.sequence, range));
                }
                Ok(())
            }
            OpenScopeKind::InlineCode(_, paint) => {
                if !range.is_empty() {
                    self.rich_text.push((
                        scope.sequence,
                        RichTextSpan::with_paint(range, RichTextRole::InlineCode, paint),
                    ));
                    self.add_source_boundaries(range);
                }
                Ok(())
            }
            OpenScopeKind::AutoSpaceSuppressed => {
                if !range.is_empty() {
                    self.auto_space_suppressed_ranges.push((scope.sequence, range));
                }
                Ok(())
            }
        }
    }

    fn with_scope(&mut self, kind: OpenScopeKind, content: impl FnOnce(&mut Self)) {
        let scope_kind = kind.public_kind();
        let sequence = self.push_scope(kind);
        content(self);
        self.finish_closure_scope(sequence, scope_kind);
    }

    fn try_with_scope(
        &mut self,
        kind: OpenScopeKind,
        content: impl FnOnce(&mut Self) -> Result<(), ParagraphBuildError>,
    ) -> Result<(), ParagraphBuildError> {
        let scope_kind = kind.public_kind();
        let sequence = self.push_scope(kind);
        if let Err(error) = content(self) {
            self.record_error(error.clone());
            return Err(error);
        }
        self.finish_closure_scope(sequence, scope_kind);
        self.first_error.clone().map_or(Ok(()), Err)
    }

    pub(super) fn ensure_position_insertion_is_allowed(
        &self,
        insertion: ParagraphPositionInsertionKind,
    ) -> Result<(), ParagraphBuildError> {
        if self
            .open_scopes
            .iter()
            .any(|scope| matches!(scope.kind, OpenScopeKind::Ruby(_)))
        {
            return Err(ParagraphBuildError::ForbiddenPositionInsertion {
                scope: ParagraphScopeKind::Ruby,
                insertion,
            });
        }
        Ok(())
    }
}
