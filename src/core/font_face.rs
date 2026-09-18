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
}

impl Display for FontFaceId {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{}#{}@{}",
            self.resource_id(),
            self.collection_index(),
            self.variation_instance()
        )
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
