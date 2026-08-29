// 对应 Kotlin 源文件：engine/src/commonMain/kotlin/org/tiqian/shaping/ReplayableFontBackend.kt

use crate::common::{HashMap, HashSet};
use std::fmt::{Display, Formatter};

use super::super::core::Text::Text;
use super::super::font::FontPolicy::FontRole;

/**
 * 一个 physical SFNT face、collection index 与 variation instance 的稳定 identity。
 *
 * [`FontFaceId`] 可安全存入 `Glyph.render_font_key`：它指向 font bytes，而不是进程内 platform object。
 * 平台 renderer 通过为 shaping 与 metrics 提供字体的同一 catalog 解析它。
 */
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct FontFaceId(String);

impl FontFaceId {
    pub fn new(value: String) -> Self {
        assert!(!value.trim().is_empty(), "FontFaceId must not be blank");
        Self(value)
    }

    pub fn value(&self) -> &str {
        &self.0
    }
}

impl Display for FontFaceId {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReplayableFontFaceDescriptor {
    pub id: FontFaceId,
    pub family_aliases: HashSet<String>,
    pub roles: HashSet<FontRole>,
    pub weight: i32,
    pub italic: bool,
    pub collection_index: i32,
    pub source_label: String,
    pub variation_axes: HashMap<String, f32>,
}

impl ReplayableFontFaceDescriptor {
    pub fn new(
        id: FontFaceId,
        family_aliases: HashSet<String>,
        roles: HashSet<FontRole>,
        source_label: String,
    ) -> Self {
        Self {
            id,
            family_aliases,
            roles,
            weight: 400,
            italic: false,
            collection_index: 0,
            source_label,
            variation_axes: HashMap::new(),
        }
    }

    pub fn builder(
        id: FontFaceId,
        family_aliases: HashSet<String>,
        roles: HashSet<FontRole>,
        source_label: String,
    ) -> ReplayableFontFaceDescriptorBuilder {
        ReplayableFontFaceDescriptorBuilder {
            descriptor: Self::new(id, family_aliases, roles, source_label),
        }
    }
}

pub struct ReplayableFontFaceDescriptorBuilder {
    descriptor: ReplayableFontFaceDescriptor,
}

impl ReplayableFontFaceDescriptorBuilder {
    pub fn weight(mut self, value: i32) -> Self {
        self.descriptor.weight = value;
        self
    }

    pub fn italic(mut self, value: bool) -> Self {
        self.descriptor.italic = value;
        self
    }

    pub fn collection_index(mut self, value: i32) -> Self {
        self.descriptor.collection_index = value;
        self
    }

    pub fn variation_axes(mut self, value: HashMap<String, f32>) -> Self {
        self.descriptor.variation_axes = value;
        self
    }

    pub fn build(self) -> ReplayableFontFaceDescriptor {
        self.descriptor
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReplayableFontFaceRequest {
    pub role: FontRole,
    pub preferred_families: Vec<String>,
    /// 请求的 em size；平台默认选择可以解析依赖 size 的 variation axis。
    pub font_size: f32,
    pub weight: i32,
    pub italic: bool,
    pub locale: String,
    /// 用于拒绝无法覆盖此具体 run 的 face 的文本。
    pub selection_text: Text,
}

impl ReplayableFontFaceRequest {
    pub fn new(
        role: FontRole,
        preferred_families: Vec<String>,
        font_size: f32,
        weight: i32,
        italic: bool,
        locale: String,
        selection_text: Text,
    ) -> Self {
        assert!(
            font_size > 0.0 && font_size.is_finite(),
            "fontSize must be positive and finite"
        );
        Self {
            role,
            preferred_families,
            font_size,
            weight,
            italic,
            locale,
            selection_text,
        }
    }
}

/// 一项具名的 evidence 或 coverage 损失。report 用于告知 host，绝不据此路由到另一个 renderer。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FontBackendCapabilityIssue {
    pub code: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FontBackendCapabilityReport {
    pub backend: String,
    pub source_kind: String,
    pub faces: Vec<ReplayableFontFaceDescriptor>,
    pub issues: Vec<FontBackendCapabilityIssue>,
}

impl FontBackendCapabilityReport {
    pub fn new(
        backend: String,
        source_kind: String,
        faces: Vec<ReplayableFontFaceDescriptor>,
    ) -> Self {
        Self {
            backend,
            source_kind,
            faces,
            issues: Vec::new(),
        }
    }

    pub fn with_issues(
        backend: String,
        source_kind: String,
        faces: Vec<ReplayableFontFaceDescriptor>,
        issues: Vec<FontBackendCapabilityIssue>,
    ) -> Self {
        Self {
            backend,
            source_kind,
            faces,
            issues,
        }
    }

    pub fn can_replay_from_controlled_bytes(&self) -> bool {
        !self.faces.is_empty()
            && self
                .issues
                .iter()
                .all(|issue| issue.code != "MissingControlledFontFace")
    }
}

/**
 * shaping、metrics 与 replay 共享的 platform-neutral catalog contract。具体 catalog 可以持有
 * file、byte array、asset 或公开 system-font handle，但调用方只能观察稳定的 face descriptor。
 */
pub trait ReplayableFontCatalog {
    fn faces(&self) -> &[ReplayableFontFaceDescriptor];

    fn capability_report(&self) -> &FontBackendCapabilityReport;

    fn resolve(&self, request: &ReplayableFontFaceRequest) -> Option<ReplayableFontFaceDescriptor>;
}
