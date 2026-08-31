use std::fs;

use serde::Deserialize;
use tiqian::core::geometry::Rect;
use tiqian::core::layout_model::{Cluster, Glyph, GlyphRun, ShapingDecisionInfo};
use tiqian::font::font_metrics::{FontMetricSource, FontMetricsRequest, FontMetricsResolver};
use tiqian::font::font_policy::RawFontMetrics;
use tiqian::shaping::text_shaper::{ShapingInput, ShapingResult, TextShaper};

#[derive(Clone)]
pub struct RecordedShapingEvidence {
    shaping: Vec<ShapingEvidenceEntry>,
    metrics: Vec<MetricsEvidenceEntry>,
}

impl RecordedShapingEvidence {
    pub fn load(path: &str) -> Self {
        let json = fs::read_to_string(path)
            .unwrap_or_else(|error| panic!("read recorded shaping evidence {path}: {error}"));
        let wire: ShapingEvidenceWire = serde_json::from_str(&json)
            .unwrap_or_else(|error| panic!("parse recorded shaping evidence {path}: {error}"));
        Self {
            shaping: wire.shaping,
            metrics: wire.metrics,
        }
    }
}

pub struct RecordedEvidenceTextShaper {
    evidence: RecordedShapingEvidence,
}

impl RecordedEvidenceTextShaper {
    pub fn new(evidence: RecordedShapingEvidence) -> Self {
        Self { evidence }
    }
}

impl TextShaper for RecordedEvidenceTextShaper {
    fn shape(&self, input: &ShapingInput) -> ShapingResult {
        let recorded = self
            .evidence
            .shaping
            .iter()
            .find(|entry| entry.key.matches(input))
            .unwrap_or_else(|| {
                panic!(
                    "No recorded shaping evidence for {} — re-record on the JVM with TIQIAN_RECORD_SHAPING=1 ./gradlew :engine:jvmTest --tests '*ShapingEvidenceRecorder*'",
                    ShapingEvidenceKey::from_input(input).describe(),
                )
            });
        let source_text = input.text.slice_text(input.range);
        let font_key = input.font_decision.candidate.key.clone();
        let cluster = Cluster::with_display_text(
            input.range,
            source_text.clone(),
            input.display_text.clone(),
            font_key.clone(),
            recorded.result.cluster_advance,
        );
        let glyphs = recorded
            .result
            .glyphs
            .iter()
            .map(|glyph| {
                Glyph::builder(glyph.id as u32, input.range, glyph.advance)
                    .x(glyph.x)
                    .y(glyph.y)
                    .bounds(glyph.bounds.map(|bounds| Rect {
                        left: bounds[0],
                        top: bounds[1],
                        right: bounds[2],
                        bottom: bounds[3],
                    }))
                    .halt_advance(glyph.halt_advance)
                    .halt_placement_x(glyph.halt_placement_x)
                    .build()
            })
            .collect();
        let run = GlyphRun::with_open_type_features(
            input.range,
            font_key.clone(),
            glyphs,
            recorded.result.run_advance,
            recorded.result.run_features.clone(),
        );
        let decisions = recorded
            .result
            .decisions
            .iter()
            .map(|decision| {
                ShapingDecisionInfo::builder(
                    input.range,
                    source_text.clone(),
                    input.display_text.clone(),
                    font_key.clone(),
                    decision.glyph_count,
                    decision.advance,
                    decision.source.clone(),
                    decision.reason.clone(),
                )
                .glyphs_without_ink_bounds(decision.glyphs_without_ink_bounds)
                .missing_glyphs(decision.missing_glyphs)
                .resolved_face(decision.resolved_face.clone())
                .script(decision.script.clone())
                .language(decision.language.clone())
                .strategy(decision.strategy.clone())
                .feature_evidence(decision.feature_evidence.clone())
                .capability_issue(decision.capability_issue.clone())
                .build()
            })
            .collect();
        ShapingResult::with_decisions(vec![cluster], vec![run], decisions)
    }
}

pub struct RecordedEvidenceFontMetricsResolver {
    evidence: RecordedShapingEvidence,
}

impl RecordedEvidenceFontMetricsResolver {
    pub fn new(evidence: RecordedShapingEvidence) -> Self {
        Self { evidence }
    }
}

impl FontMetricsResolver for RecordedEvidenceFontMetricsResolver {
    fn resolve(&self, request: &FontMetricsRequest) -> RawFontMetrics {
        let recorded = self
            .evidence
            .metrics
            .iter()
            .find(|entry| entry.key.matches(request))
            .unwrap_or_else(|| {
                panic!(
                    "No recorded font metrics evidence for {} — re-record on the JVM with TIQIAN_RECORD_SHAPING=1 ./gradlew :engine:jvmTest --tests '*ShapingEvidenceRecorder*'",
                    MetricsEvidenceKey::from_request(request).describe(),
                )
            });
        RawFontMetrics::builder(recorded.result.ascent, recorded.result.descent)
            .leading(recorded.result.leading)
            .source(font_metric_source(&recorded.result.source))
            .typo_ascent(recorded.result.typo_ascent)
            .typo_descent(recorded.result.typo_descent)
            .build()
    }
}

#[derive(Deserialize)]
struct ShapingEvidenceWire {
    shaping: Vec<ShapingEvidenceEntry>,
    metrics: Vec<MetricsEvidenceEntry>,
}

#[derive(Clone, Deserialize)]
struct ShapingEvidenceEntry {
    key: ShapingEvidenceKey,
    result: RecordedShapingResult,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ShapingEvidenceKey {
    display_text: String,
    font_key: String,
    font_family: String,
    role: String,
    style_font_families: Vec<String>,
    font_size: f32,
    font_weight: i32,
    italic: bool,
    locale: String,
    open_type_features: Vec<String>,
}

impl ShapingEvidenceKey {
    fn from_input(input: &ShapingInput) -> Self {
        Self {
            display_text: input.display_text.to_string(),
            font_key: input.font_decision.candidate.key.clone(),
            font_family: input.font_decision.candidate.family.clone(),
            role: format!("{:?}", input.font_decision.role),
            style_font_families: input.style.font_families.clone(),
            font_size: input.style.font_size,
            font_weight: input.style.font_weight,
            italic: input.style.italic,
            locale: input.style.locale.clone(),
            open_type_features: input.open_type_features.clone(),
        }
    }

    fn matches(&self, input: &ShapingInput) -> bool {
        let actual = Self::from_input(input);
        self.display_text == actual.display_text
            && self.font_key == actual.font_key
            && self.font_family == actual.font_family
            && self.role == actual.role
            && self.style_font_families == actual.style_font_families
            && self.font_size == actual.font_size
            && self.font_weight == actual.font_weight
            && self.italic == actual.italic
            && self.locale == actual.locale
            && self.open_type_features == actual.open_type_features
    }

    fn describe(&self) -> String {
        format!(
            "ShapingEvidenceKey(displayText={}, fontKey={}, fontFamily={}, role={}, styleFontFamilies={:?}, fontSize={}, fontWeight={}, italic={}, locale={}, openTypeFeatures={:?})",
            self.display_text,
            self.font_key,
            self.font_family,
            self.role,
            self.style_font_families,
            self.font_size,
            self.font_weight,
            self.italic,
            self.locale,
            self.open_type_features,
        )
    }
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RecordedShapingResult {
    cluster_advance: f32,
    run_advance: f32,
    run_features: Vec<String>,
    glyphs: Vec<RecordedGlyph>,
    decisions: Vec<RecordedShapingDecision>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RecordedGlyph {
    id: u64,
    advance: f32,
    x: f32,
    y: f32,
    bounds: Option<[f32; 4]>,
    halt_advance: Option<f32>,
    halt_placement_x: Option<f32>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RecordedShapingDecision {
    glyph_count: i32,
    advance: f32,
    source: String,
    reason: String,
    glyphs_without_ink_bounds: i32,
    missing_glyphs: i32,
    resolved_face: Option<String>,
    script: Option<String>,
    language: Option<String>,
    strategy: Option<String>,
    feature_evidence: Option<String>,
    capability_issue: Option<String>,
}

#[derive(Clone, Deserialize)]
struct MetricsEvidenceEntry {
    key: MetricsEvidenceKey,
    result: RecordedFontMetrics,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MetricsEvidenceKey {
    font_key: String,
    font_size: f32,
    role: String,
    locale: String,
    font_families: Vec<String>,
    font_weight: i32,
    italic: bool,
    face_selection_text: String,
}

impl MetricsEvidenceKey {
    fn from_request(request: &FontMetricsRequest) -> Self {
        Self {
            font_key: request.font_key.clone(),
            font_size: request.font_size,
            role: format!("{:?}", request.role),
            locale: request.locale.clone(),
            font_families: request.font_families.clone(),
            font_weight: request.font_weight,
            italic: request.italic,
            face_selection_text: request.face_selection_text.to_string(),
        }
    }

    fn matches(&self, request: &FontMetricsRequest) -> bool {
        let actual = Self::from_request(request);
        self.font_key == actual.font_key
            && self.font_size == actual.font_size
            && self.role == actual.role
            && self.locale == actual.locale
            && self.font_families == actual.font_families
            && self.font_weight == actual.font_weight
            && self.italic == actual.italic
            && self.face_selection_text == actual.face_selection_text
    }

    fn describe(&self) -> String {
        format!(
            "MetricsEvidenceKey(fontKey={}, fontSize={}, role={}, locale={}, fontFamilies={:?}, fontWeight={}, italic={}, faceSelectionText={})",
            self.font_key,
            self.font_size,
            self.role,
            self.locale,
            self.font_families,
            self.font_weight,
            self.italic,
            self.face_selection_text,
        )
    }
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RecordedFontMetrics {
    ascent: f32,
    descent: f32,
    leading: f32,
    source: String,
    typo_ascent: Option<f32>,
    typo_descent: Option<f32>,
}

fn font_metric_source(value: &str) -> FontMetricSource {
    match value {
        "RawTables" => FontMetricSource::RawTables,
        "OpenTypeBase" => FontMetricSource::OpenTypeBase,
        "GlyphSampling" => FontMetricSource::GlyphSampling,
        "ManualOverride" => FontMetricSource::ManualOverride,
        "SynthesizedIdeographicBox" => FontMetricSource::SynthesizedIdeographicBox,
        _ => panic!("Unknown recorded font metric source {value}"),
    }
}
