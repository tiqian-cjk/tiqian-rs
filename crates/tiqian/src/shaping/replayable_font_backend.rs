// 对应 Kotlin 源文件：engine/src/commonMain/kotlin/org/tiqian/shaping/ReplayableFontBackend.kt

use crate::common::HashSet;

use super::super::core::font_face::FontFaceId;
use super::super::font::font_policy::FontRole;

#[derive(Clone, Debug, PartialEq)]
pub struct ReplayableFontFaceDescriptor {
    pub id: FontFaceId,
    pub family_aliases: HashSet<String>,
    pub roles: HashSet<FontRole>,
    pub weight: i32,
    pub italic: bool,
    pub source_label: String,
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
            source_label,
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

    pub fn build(self) -> ReplayableFontFaceDescriptor {
        self.descriptor
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

    fn face(&self, id: &FontFaceId) -> Option<&ReplayableFontFaceDescriptor>;
}
