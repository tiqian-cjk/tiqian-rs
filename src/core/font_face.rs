use std::fmt::{Display, Formatter};
use std::sync::Arc;

/// 由受控字体目录解析出的物理字体面的稳定标识。
///
/// 该标识由字体 backend 分配并由字体目录解析。布局层只传递和比较该标识，因此不同字体资源、字体集合成员
/// 和 variation instance 不会共享同一个 key。
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct FontFaceId(Arc<FontFaceIdData>);

#[derive(Debug, PartialEq, Eq, Hash)]
struct FontFaceIdData {
    resource_id: String,
    collection_index: u32,
    variation_instance: FontVariationInstance,
    synthesis: FontSynthesisInstance,
}

impl FontFaceId {
    pub fn new(
        resource_id: String,
        collection_index: u32,
        variation_instance: FontVariationInstance,
    ) -> Self {
        Self(Arc::new(FontFaceIdData {
            resource_id,
            collection_index,
            variation_instance,
            synthesis: FontSynthesisInstance::default(),
        }))
    }

    pub fn with_resource_id(resource_id: &str) -> Self {
        FontFaceId::new(resource_id.to_owned(), 0, FontVariationInstance::default())
    }

    pub fn resource_id(&self) -> &str {
        &self.0.resource_id
    }

    pub fn collection_index(&self) -> u32 {
        self.0.collection_index
    }

    pub fn variation_instance(&self) -> &FontVariationInstance {
        &self.0.variation_instance
    }

    pub fn with_synthesis(&self, synthesis: FontSynthesisInstance) -> Self {
        Self(Arc::new(FontFaceIdData {
            resource_id: self.0.resource_id.clone(),
            collection_index: self.0.collection_index,
            variation_instance: self.0.variation_instance.clone(),
            synthesis,
        }))
    }

    pub fn synthesis(&self) -> &FontSynthesisInstance {
        &self.0.synthesis
    }
}

impl Display for FontFaceId {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{}#{}@{}+{}",
            self.resource_id(),
            self.collection_index(),
            self.variation_instance(),
            self.synthesis()
        )
    }
}

/// 最终字体实例实际采用的软件合成参数。
///
/// 该描述不表示调用方允许的 fallback 策略；它只记录 backend 在真实静态 face 与标准 variation axis
/// 无法满足请求时实际采用的结果。
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct FontSynthesisInstance {
    embolden_em_bits: Option<u32>,
    oblique_degrees_bits: Option<u32>,
}

impl FontSynthesisInstance {
    pub fn new(embolden_em: Option<f32>, oblique_degrees: Option<f32>) -> Self {
        Self {
            embolden_em_bits: embolden_em.map(f32::to_bits),
            oblique_degrees_bits: oblique_degrees.map(f32::to_bits),
        }
    }

    pub fn embolden_em(&self) -> Option<f32> {
        self.embolden_em_bits.map(f32::from_bits)
    }

    pub fn oblique_degrees(&self) -> Option<f32> {
        self.oblique_degrees_bits.map(f32::from_bits)
    }
}

impl Display for FontSynthesisInstance {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match (self.embolden_em(), self.oblique_degrees()) {
            (None, None) => formatter.write_str("none"),
            (Some(embolden_em), None) => write!(formatter, "embolden={embolden_em}em"),
            (None, Some(oblique_degrees)) => write!(formatter, "oblique={oblique_degrees}deg"),
            (Some(embolden_em), Some(oblique_degrees)) => write!(
                formatter,
                "embolden={embolden_em}em,oblique={oblique_degrees}deg"
            ),
        }
    }
}

/// 属于一个物理字体实例的规范化 OpenType variation 设置。
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct FontVariationInstance(Vec<FontVariationSetting>);

impl FontVariationInstance {
    pub fn new(mut settings: Vec<FontVariationSetting>) -> Self {
        settings.retain(|setting| {
            if setting.tag.trim().is_empty() {
                log::warn!("blank font variation axis tag; ignoring axis");
                false
            } else if !setting.value().is_finite() {
                log::warn!("non-finite font variation axis value; ignoring axis");
                false
            } else {
                true
            }
        });
        settings.sort_by(|left, right| left.tag.cmp(&right.tag));
        settings.dedup_by(|later, first| {
            if later.tag == first.tag {
                log::warn!("duplicate font variation axis tag; ignoring later axis");
                true
            } else {
                false
            }
        });
        Self(settings)
    }

    pub fn settings(&self) -> &[FontVariationSetting] {
        &self.0
    }
}

impl Display for FontVariationInstance {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        if self.0.is_empty() {
            return formatter.write_str("default");
        }
        for (index, setting) in self.0.iter().enumerate() {
            if index > 0 {
                formatter.write_str(",")?;
            }
            write!(formatter, "{}={}", setting.tag, setting.value())?;
        }
        Ok(())
    }
}

/// 使用精确 f32 位模式存储的一项规范化 OpenType variation 设置。
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct FontVariationSetting {
    tag: String,
    value_bits: u32,
}

impl FontVariationSetting {
    pub fn new(tag: String, value: f32) -> Self {
        Self {
            tag,
            value_bits: value.to_bits(),
        }
    }

    pub fn tag(&self) -> &str {
        &self.tag
    }

    pub fn value(&self) -> f32 {
        f32::from_bits(self.value_bits)
    }
}

#[cfg(test)]
mod tests {
    use super::{FontFaceId, FontSynthesisInstance, FontVariationInstance};

    #[test]
    fn synthetic_parameters_distinguish_final_font_instances() {
        let physical = FontFaceId::new(
            "test-resource".to_owned(),
            0,
            FontVariationInstance::default(),
        );
        let emboldened = physical.with_synthesis(FontSynthesisInstance::new(Some(0.03), None));
        let oblique = physical.with_synthesis(FontSynthesisInstance::new(None, Some(14.0)));

        assert_ne!(physical, emboldened);
        assert_ne!(emboldened, oblique);
        assert_eq!(emboldened.synthesis().embolden_em(), Some(0.03));
        assert_eq!(oblique.synthesis().oblique_degrees(), Some(14.0));
        assert_eq!(physical.synthesis(), &FontSynthesisInstance::default());
        assert_eq!(
            emboldened.to_string(),
            "test-resource#0@default+embolden=0.03em"
        );
    }
}
