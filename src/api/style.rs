use crate::core::text::Text;
use crate::core::text_model::{
    InlineAttachment, InlineBoxOuterSpacing, InlineObjectBoundaryAdjustment, RubyKind, TextStyle,
};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TextStyleOverride {
    pub font_families: Option<Vec<String>>,
    pub font_size: Option<f32>,
    pub locale: Option<String>,
    pub font_weight: Option<i32>,
    pub italic: Option<bool>,
    pub baseline_shift: Option<f32>,
    pub inline_attachment: Option<InlineAttachment>,
}

impl TextStyleOverride {
    pub fn builder() -> TextStyleOverrideBuilder {
        TextStyleOverrideBuilder {
            override_style: Self::default(),
        }
    }

    pub(super) fn apply_to(&self, base: &TextStyle) -> TextStyle {
        TextStyle {
            font_families: self
                .font_families
                .clone()
                .unwrap_or_else(|| base.font_families.clone()),
            font_size: self.font_size.unwrap_or(base.font_size),
            locale: self.locale.clone().unwrap_or_else(|| base.locale.clone()),
            font_weight: self.font_weight.unwrap_or(base.font_weight),
            italic: self.italic.unwrap_or(base.italic),
            baseline_shift: self.baseline_shift.unwrap_or(base.baseline_shift),
            inline_attachment: self.inline_attachment.unwrap_or(base.inline_attachment),
        }
    }
}

pub struct TextStyleOverrideBuilder {
    override_style: TextStyleOverride,
}

impl TextStyleOverrideBuilder {
    pub fn font_families(mut self, value: Vec<String>) -> Self {
        self.override_style.font_families = Some(value);
        self
    }

    pub fn font_size(mut self, value: f32) -> Self {
        self.override_style.font_size = Some(value);
        self
    }

    pub fn locale(mut self, value: String) -> Self {
        self.override_style.locale = Some(value);
        self
    }

    pub fn font_weight(mut self, value: i32) -> Self {
        self.override_style.font_weight = Some(value);
        self
    }

    pub fn italic(mut self, value: bool) -> Self {
        self.override_style.italic = Some(value);
        self
    }

    pub fn baseline_shift(mut self, value: f32) -> Self {
        self.override_style.baseline_shift = Some(value);
        self
    }

    pub fn inline_attachment(mut self, value: InlineAttachment) -> Self {
        self.override_style.inline_attachment = Some(value);
        self
    }

    pub fn build(self) -> TextStyleOverride {
        self.override_style
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RubyAnnotation {
    pub(super) text: Text,
    pub(super) font_families: Vec<String>,
    pub(super) kind: RubyKind,
    pub(super) locale: Option<String>,
}

impl RubyAnnotation {
    pub fn pinyin(text: &str) -> Self {
        Self::builder(text).build()
    }

    pub fn bopomofo(text: &str) -> Self {
        Self::builder(text).kind(RubyKind::Bopomofo).build()
    }

    pub fn builder(text: &str) -> RubyAnnotationBuilder {
        RubyAnnotationBuilder {
            annotation: Self {
                text: Text::from(text),
                font_families: Vec::new(),
                kind: RubyKind::Pinyin,
                locale: None,
            },
            locale_was_set: false,
        }
    }
}

pub struct RubyAnnotationBuilder {
    annotation: RubyAnnotation,
    locale_was_set: bool,
}

impl RubyAnnotationBuilder {
    pub fn font_families(mut self, value: Vec<String>) -> Self {
        self.annotation.font_families = value;
        self
    }

    pub fn kind(mut self, value: RubyKind) -> Self {
        self.annotation.kind = value;
        self
    }

    pub fn locale(mut self, value: Option<String>) -> Self {
        self.annotation.locale = value;
        self.locale_was_set = true;
        self
    }

    pub fn build(mut self) -> RubyAnnotation {
        if !self.locale_was_set && self.annotation.kind == RubyKind::Bopomofo {
            self.annotation.locale = Some("zh-TW".to_owned());
        }
        self.annotation
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InlineBoxStyle {
    pub inline_start: f32,
    pub inline_end: f32,
    pub outer_spacing: InlineBoxOuterSpacing,
}

impl InlineBoxStyle {
    pub fn new() -> Self {
        Self {
            inline_start: 0.0,
            inline_end: 0.0,
            outer_spacing: InlineBoxOuterSpacing::Narrow,
        }
    }

    pub fn with_edges(inline_start: f32, inline_end: f32) -> Self {
        Self {
            inline_start,
            inline_end,
            outer_spacing: InlineBoxOuterSpacing::Narrow,
        }
    }

    pub fn with_all(
        inline_start: f32,
        inline_end: f32,
        outer_spacing: InlineBoxOuterSpacing,
    ) -> Self {
        Self {
            inline_start,
            inline_end,
            outer_spacing,
        }
    }

    pub fn builder() -> InlineBoxStyleBuilder {
        InlineBoxStyleBuilder { style: Self::new() }
    }
}

impl Default for InlineBoxStyle {
    fn default() -> Self {
        Self::new()
    }
}

pub struct InlineBoxStyleBuilder {
    style: InlineBoxStyle,
}

impl InlineBoxStyleBuilder {
    pub fn inline_start(mut self, value: f32) -> Self {
        self.style.inline_start = value;
        self
    }

    pub fn inline_end(mut self, value: f32) -> Self {
        self.style.inline_end = value;
        self
    }

    pub fn outer_spacing(mut self, value: InlineBoxOuterSpacing) -> Self {
        self.style.outer_spacing = value;
        self
    }

    pub fn build(self) -> InlineBoxStyle {
        self.style
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct InlineObjectMetrics {
    pub advance: f32,
    pub ascent: f32,
    pub descent: f32,
    pub leading_boundary: InlineObjectBoundaryAdjustment,
    pub trailing_boundary: InlineObjectBoundaryAdjustment,
}

impl InlineObjectMetrics {
    pub fn new(advance: f32, ascent: f32, descent: f32) -> Self {
        Self {
            advance,
            ascent,
            descent,
            leading_boundary: InlineObjectBoundaryAdjustment::FIXED,
            trailing_boundary: InlineObjectBoundaryAdjustment::FIXED,
        }
    }

    pub fn builder(advance: f32, ascent: f32, descent: f32) -> InlineObjectMetricsBuilder {
        InlineObjectMetricsBuilder {
            metrics: Self::new(advance, ascent, descent),
        }
    }
}

pub struct InlineObjectMetricsBuilder {
    metrics: InlineObjectMetrics,
}

impl InlineObjectMetricsBuilder {
    pub fn leading_boundary(mut self, value: InlineObjectBoundaryAdjustment) -> Self {
        self.metrics.leading_boundary = value;
        self
    }

    pub fn trailing_boundary(mut self, value: InlineObjectBoundaryAdjustment) -> Self {
        self.metrics.trailing_boundary = value;
        self
    }

    pub fn build(self) -> InlineObjectMetrics {
        self.metrics
    }
}

#[cfg(test)]
mod tests {
    use crate::api::{RubyAnnotation, TextStyleOverride};
    use crate::core::text_model::TextStyle;

    #[test]
    fn text_style_override_preserves_unspecified_fields() {
        let base = TextStyle::builder()
            .font_families(vec!["Source Han Sans SC".to_owned()])
            .font_size(15.0)
            .font_weight(400)
            .italic(false)
            .build();
        let override_style = TextStyleOverride::builder()
            .font_weight(700)
            .italic(true)
            .build();

        assert_eq!(
            TextStyle::builder()
                .font_families(vec!["Source Han Sans SC".to_owned()])
                .font_size(15.0)
                .font_weight(700)
                .italic(true)
                .build(),
            override_style.apply_to(&base)
        );
    }

    #[test]
    fn ruby_annotation_applies_bopomofo_locale_default() {
        assert_eq!(
            Some("zh-TW".to_owned()),
            RubyAnnotation::bopomofo("ㄊㄧˊ ㄑㄧㄢˋ").locale
        );
        assert_eq!(None, RubyAnnotation::pinyin("tíqiàn").locale);
    }
}
