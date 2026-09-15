use std::sync::{Arc, Mutex};

use tiqian::core::font_face::FontFaceId;
use tiqian::core::geometry::{text_range, LayoutConstraints};
use tiqian::core::text::Text;
use tiqian::core::text_model::{LayoutInput, RubySpan, TextSpan, TextStyle, TiqianTextContent};
use tiqian::font::font_metrics::FontMetricsRequest;
use tiqian::font::font_policy::{FontRole, RawFontMetrics};
use tiqian::layout::paragraph_layout_engine::{
    ExplainableStubParagraphLayoutEngine, ParagraphLayoutEngine,
};
use tiqian::shaping::font_backend::{
    FontBackend, FontBackendRequest, FontBackendShapingResult,
};
use tiqian::shaping::replayable_font_backend::{
    FontBackendCapabilityReport, ReplayableFontCatalog, ReplayableFontFaceDescriptor,
};
use tiqian::shaping::stub_font_backend::DeterministicStubFontBackend;

#[derive(Clone, Debug)]
struct ShapingRecord {
    request: FontBackendRequest,
    face: FontFaceId,
}

struct RecordingFontBackend {
    fallback: DeterministicStubFontBackend,
    shaping: Arc<Mutex<Vec<ShapingRecord>>>,
    metrics: Arc<Mutex<Vec<FontMetricsRequest>>>,
}

impl ReplayableFontCatalog for RecordingFontBackend {
    fn faces(&self) -> &[ReplayableFontFaceDescriptor] {
        self.fallback.faces()
    }

    fn capability_report(&self) -> &FontBackendCapabilityReport {
        self.fallback.capability_report()
    }

    fn face(&self, id: &FontFaceId) -> Option<&ReplayableFontFaceDescriptor> {
        self.fallback.face(id)
    }
}

impl FontBackend for RecordingFontBackend {
    fn shape(&self, request: &FontBackendRequest) -> FontBackendShapingResult {
        let result = self.fallback.shape(request);
        self.shaping.lock().unwrap().push(ShapingRecord {
            request: request.clone(),
            face: result.face.clone(),
        });
        result
    }

    fn metrics(&self, request: &FontMetricsRequest) -> RawFontMetrics {
        self.metrics.lock().unwrap().push(request.clone());
        self.fallback.metrics(request)
    }
}

fn engine_with_requests(
    shaping: Arc<Mutex<Vec<ShapingRecord>>>,
    metrics: Arc<Mutex<Vec<FontMetricsRequest>>>,
) -> ExplainableStubParagraphLayoutEngine {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.font_backend = Box::new(RecordingFontBackend {
        fallback: DeterministicStubFontBackend::default(),
        shaping,
        metrics,
    });
    engine
}

#[test]
fn per_span_weight_and_italic_reach_the_font_backend_before_metrics() {
    let shaping = Arc::new(Mutex::new(Vec::new()));
    let metrics = Arc::new(Mutex::new(Vec::new()));
    let mut engine = engine_with_requests(shaping.clone(), metrics.clone());
    let base = TextStyle::builder()
        .font_families(vec!["Fixture Sans".to_owned()])
        .font_size(18.0)
        .font_weight(400)
        .italic(false)
        .build();
    engine.layout(
        LayoutInput::builder(
            TiqianTextContent::builder(Text::from("中A"))
                .spans(vec![TextSpan {
                    range: text_range(1, 2),
                    style: TextStyle::builder()
                        .font_families(vec!["Fixture Sans".to_owned()])
                        .font_size(18.0)
                        .font_weight(700)
                        .italic(true)
                        .build(),
                }])
                .build(),
            LayoutConstraints::with_defaults(180.0),
        )
        .text_style(base)
        .build(),
    );

    let shaping = shaping.lock().unwrap();
    assert!(
        shaping
            .iter()
            .any(|record| record.request.role == FontRole::CjkText
                && record.request.style.font_weight == 400
                && !record.request.style.italic
                && record.request.display_text == "中")
    );
    assert!(
        shaping
            .iter()
            .any(|record| record.request.role == FontRole::LatinText
                && record.request.style.font_weight == 700
                && record.request.style.italic
                && record.request.display_text == "A")
    );
    let selected_faces: Vec<_> = shaping.iter().map(|record| record.face.clone()).collect();
    assert!(metrics
        .lock()
        .unwrap()
        .iter()
        .all(|request| selected_faces.contains(&request.face)));
}

#[test]
fn display_substitution_selects_before_metrics_are_resolved() {
    let shaping = Arc::new(Mutex::new(Vec::new()));
    let metrics = Arc::new(Mutex::new(Vec::new()));
    let mut engine = engine_with_requests(shaping.clone(), metrics.clone());
    engine.layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from("——")),
            LayoutConstraints::with_defaults(180.0),
        )
        .text_style(
            TextStyle::builder()
                .font_families(vec!["Fixture Sans".to_owned()])
                .font_size(18.0)
                .build(),
        )
        .build(),
    );

    let shaping = shaping.lock().unwrap();
    assert!(
        shaping
            .iter()
            .any(|record| record.request.display_text == "⸺")
    );
    let selected_faces: Vec<_> = shaping.iter().map(|record| record.face.clone()).collect();
    assert!(metrics
        .lock()
        .unwrap()
        .iter()
        .all(|request| selected_faces.contains(&request.face)));
}

#[test]
fn ruby_metrics_use_the_same_final_face_as_ruby_shaping() {
    let shaping = Arc::new(Mutex::new(Vec::new()));
    let metrics = Arc::new(Mutex::new(Vec::new()));
    let mut engine = engine_with_requests(shaping.clone(), metrics.clone());
    engine.layout(
        LayoutInput::builder(
            TiqianTextContent::new(Text::from("中")),
            LayoutConstraints::with_defaults(180.0),
        )
        .text_style(
            TextStyle::builder()
                .font_families(vec!["Fixture Sans".to_owned()])
                .font_size(18.0)
                .italic(true)
                .build(),
        )
        .ruby_spans(vec![RubySpan::new(
            text_range(0, 1),
            Text::from("zhōng"),
        )])
        .build(),
    );

    let shaping = shaping.lock().unwrap();
    let ruby_shaping = shaping
        .iter()
        .find(|record| record.request.role == FontRole::LatinText && record.request.display_text == "zhōng")
        .expect("ruby text must be shaped by the font backend");
    assert!(
        metrics
            .lock()
            .unwrap()
            .iter()
            .any(|request| request.role == FontRole::LatinText
                && request.face == ruby_shaping.face)
    );
}
