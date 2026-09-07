use crate::core::text_model::{
    DecorationKind, RichTextBackgroundPaint, RichTextLayer,
};

use super::builder::ParagraphBuilder;
use super::style::{InlineBoxStyle, RubyAnnotation, TextStyleOverride};
use super::types::ParagraphBuildError;

impl ParagraphBuilder {
    pub fn styled(&mut self, style: TextStyleOverride, text: &str) -> &mut Self {
        self.with_text_style(style, |builder| builder.push(text));
        self
    }

    pub fn ruby(
        &mut self,
        annotation: RubyAnnotation,
        text: &str,
    ) -> Result<&mut Self, ParagraphBuildError> {
        self.push_ruby(annotation)?;
        self.push(text);
        self.pop()?;
        Ok(self)
    }

    pub fn emphasis(&mut self, text: &str) -> &mut Self {
        self.decoration(DecorationKind::Emphasis, text)
    }

    pub fn mourning(&mut self, text: &str) -> &mut Self {
        self.decoration(DecorationKind::Mourning, text)
    }

    pub fn proper_noun(&mut self, text: &str) -> &mut Self {
        self.decoration(DecorationKind::ProperNoun, text)
    }

    pub fn book_title(&mut self, text: &str) -> &mut Self {
        self.decoration(DecorationKind::BookTitle, text)
    }

    pub fn color(&mut self, argb: i32, text: &str) -> &mut Self {
        self.with_color(argb, |builder| builder.push(text));
        self
    }

    pub fn rich_text(
        &mut self,
        layers: &[RichTextLayer],
        text: &str,
    ) -> &mut Self {
        self.with_rich_text(layers, |builder| builder.push(text));
        self
    }

    pub fn link(&mut self, target: String, text: &str) -> &mut Self {
        self.with_link(target, |builder| builder.push(text));
        self
    }

    pub fn technical(&mut self, text: &str) -> &mut Self {
        self.with_technical(|builder| builder.push(text));
        self
    }

    pub fn inline_code(
        &mut self,
        style: TextStyleOverride,
        background: RichTextBackgroundPaint,
        text: &str,
    ) -> &mut Self {
        self.with_inline_code(style, background, |builder| builder.push(text));
        self
    }

    pub fn inline_box(
        &mut self,
        style: InlineBoxStyle,
        text: &str,
    ) -> Result<&mut Self, ParagraphBuildError> {
        self.push_inline_box(style)?;
        self.push(text);
        self.pop()?;
        Ok(self)
    }

    pub fn auto_space_suppressed(&mut self, text: &str) -> &mut Self {
        self.with_auto_space_suppressed(|builder| builder.push(text));
        self
    }

    fn decoration(&mut self, kind: DecorationKind, text: &str) -> &mut Self {
        self.with_decoration(kind, |builder| builder.push(text));
        self
    }
}
