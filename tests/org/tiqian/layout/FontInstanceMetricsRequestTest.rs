use std::sync::{Arc, Mutex};

use tiqian::org::tiqian::core::Geometry::{LayoutConstraints, TextRange};
use tiqian::org::tiqian::core::TextModel::{
    LayoutInput, RubySpan, TextSpan, TextStyle, TiqianTextContent,
};
use tiqian::org::tiqian::font::FontMetrics::{
    FontMetricsRequest, FontMetricsResolver, StubFontMetricsResolver,
};
use tiqian::org::tiqian::font::FontPolicy::{FontRole, RawFontMetrics};
use tiqian::org::tiqian::layout::ParagraphLayoutEngine::{
    ExplainableStubParagraphLayoutEngine, ParagraphLayoutEngine,
};

struct RecordingMetricsResolver {
    requests: Arc<Mutex<Vec<FontMetricsRequest>>>,
}

impl FontMetricsResolver for RecordingMetricsResolver {
    fn resolve(&self, request: &FontMetricsRequest) -> RawFontMetrics {
        self.requests.lock().unwrap().push(request.clone());
        StubFontMetricsResolver.resolve(request)
    }
}

fn engine_with_requests(
    requests: Arc<Mutex<Vec<FontMetricsRequest>>>,
) -> ExplainableStubParagraphLayoutEngine {
    let mut engine = ExplainableStubParagraphLayoutEngine::default();
    engine.font_metrics_resolver = Box::new(RecordingMetricsResolver { requests });
    engine
}

#[test]
fn per_span_weight_and_italic_reach_metrics_resolver() {
    let requests = Arc::new(Mutex::new(Vec::new()));
    let mut engine = engine_with_requests(requests.clone());
    let base = TextStyle::builder()
        .font_families(vec!["Fixture Sans".to_owned()])
        .font_size(18.0)
        .font_weight(400)
        .italic(false)
        .build();
    engine.layout(
        LayoutInput::builder(
            TiqianTextContent::builder("中A".to_owned())
                .spans(vec![TextSpan {
                    range: TextRange::new(1, 2),
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

    let requests = requests.lock().unwrap();
    assert!(
        requests
            .iter()
            .any(|request| request.role == FontRole::CjkText
                && request.font_weight == 400
                && !request.italic
                && request.face_selection_text == "中")
    );
    assert!(
        requests
            .iter()
            .any(|request| request.role == FontRole::LatinText
                && request.font_weight == 700
                && request.italic
                && request.face_selection_text == "A")
    );
}

#[test]
fn display_substitution_and_ruby_use_the_actual_metric_face_instance() {
    let requests = Arc::new(Mutex::new(Vec::new()));
    let mut engine = engine_with_requests(requests.clone());
    engine.layout(
        LayoutInput::builder(
            TiqianTextContent::new("——中".to_owned()),
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
            TextRange::new(2, 3),
            "zhōng".to_owned(),
        )])
        .build(),
    );

    let requests = requests.lock().unwrap();
    assert!(
        requests
            .iter()
            .any(|request| request.face_selection_text == "⸺")
    );
    assert!(
        requests
            .iter()
            .any(|request| request.role == FontRole::LatinText
                && request.face_selection_text == "zhōng"
                && request.italic)
    );
}
