use super::super::core::font_face::FontFaceId;
use super::super::core::geometry::TextRange;
use super::super::core::text::Text;
use super::super::core::text_model::TextStyle;
use super::super::font::font_metrics::FontMetricsRequest;
use super::super::font::font_policy::{FontRole, RawFontMetrics};
use super::replayable_font_backend::ReplayableFontCatalog;
use super::text_shaper::ShapingResult;
use std::sync::Arc;

/// 一个段落 shaping 请求的字体选择与完整 shaping 原子范围。
#[derive(Clone, Debug, PartialEq)]
pub struct FontBackendRequest {
    pub text: Text,
    pub range: TextRange,
    pub display_text: Text,
    pub style: TextStyle,
    pub role: FontRole,
    pub open_type_features: Vec<String>,
}

impl FontBackendRequest {
    pub fn new(text: Text, range: TextRange, style: TextStyle, role: FontRole) -> Self {
        let display_text = text.slice_text(range);
        Self {
            text,
            range,
            display_text,
            style,
            role,
            open_type_features: Vec::new(),
        }
    }

    pub fn builder(
        text: Text,
        range: TextRange,
        style: TextStyle,
        role: FontRole,
    ) -> FontBackendRequestBuilder {
        FontBackendRequestBuilder {
            request: Self::new(text, range, style, role),
        }
    }
}

pub struct FontBackendRequestBuilder {
    request: FontBackendRequest,
}

impl FontBackendRequestBuilder {
    pub fn display_text(mut self, value: Text) -> Self {
        self.request.display_text = value;
        self
    }

    pub fn open_type_features(mut self, value: Vec<String>) -> Self {
        self.request.open_type_features = value;
        self
    }

    pub fn build(self) -> FontBackendRequest {
        self.request
    }
}

/// 对一个候选字体执行一次完整 shaping 的证据。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FontCandidateAttempt {
    pub candidate_key: String,
    pub face: FontFaceId,
    pub missing_glyphs: u32,
}

impl FontCandidateAttempt {
    pub fn new(candidate_key: String, face: FontFaceId, missing_glyphs: u32) -> Self {
        Self {
            candidate_key,
            face,
            missing_glyphs,
        }
    }

    pub fn has_missing_glyphs(&self) -> bool {
        self.missing_glyphs != 0
    }
}

/// 一个段落 shaping 请求的最终字体与候选尝试证据。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FontResolution {
    pub range: TextRange,
    pub role: FontRole,
    pub face: FontFaceId,
    pub attempts: Arc<[FontCandidateAttempt]>,
}

impl FontResolution {
    pub fn new(
        range: TextRange,
        role: FontRole,
        face: FontFaceId,
        attempts: Vec<FontCandidateAttempt>,
    ) -> Self {
        Self {
            range,
            role,
            face,
            attempts: Arc::from(attempts),
        }
    }

    pub fn selected_attempt(&self) -> &FontCandidateAttempt {
        self.attempts
            .iter()
            .find(|attempt| attempt.face == self.face)
            .expect("FontResolution face must appear in candidate evidence")
    }
}

/// 对选定受控字体面执行单次 shaping 后得到的输出与证据。
#[derive(Clone, Debug, PartialEq)]
pub struct FontBackendShapingResult {
    pub face: FontFaceId,
    pub shaping: ShapingResult,
    pub attempts: Vec<FontCandidateAttempt>,
}

impl FontBackendShapingResult {
    pub fn new(
        face: FontFaceId,
        shaping: ShapingResult,
        attempts: Vec<FontCandidateAttempt>,
    ) -> Self {
        assert!(
            !attempts.is_empty(),
            "FontBackendShapingResult must contain candidate evidence"
        );
        let selected_index = attempts
            .iter()
            .position(|attempt| attempt.face == face)
            .expect("FontBackendShapingResult face must appear in candidate evidence");
        let first_complete_index = attempts
            .iter()
            .position(|attempt| !attempt.has_missing_glyphs());
        if let Some(first_complete_index) = first_complete_index {
            assert_eq!(
                attempts.len(),
                first_complete_index + 1,
                "FontBackendShapingResult must stop after the first complete candidate"
            );
            assert_eq!(
                selected_index, first_complete_index,
                "FontBackendShapingResult must select the first complete candidate"
            );
        } else {
            assert_eq!(
                selected_index, 0,
                "FontBackendShapingResult must retain the preferred face when all candidates are missing glyphs"
            );
        }
        Self {
            face,
            shaping,
            attempts,
        }
    }

    pub fn selected_attempt(&self) -> &FontCandidateAttempt {
        self.attempts
            .iter()
            .find(|attempt| attempt.face == self.face)
            .expect("FontBackendShapingResult face must appear in candidate evidence")
    }

    pub fn resolution(&self, range: TextRange, role: FontRole) -> FontResolution {
        FontResolution::new(range, role, self.face.clone(), self.attempts.clone())
    }
}

/// 段落布局所需的字体选择、shaping、metrics 和回放标识。
pub trait FontBackend: ReplayableFontCatalog {
    fn shape(&self, request: &FontBackendRequest) -> FontBackendShapingResult;

    fn metrics(&self, request: &FontMetricsRequest) -> RawFontMetrics;
}
