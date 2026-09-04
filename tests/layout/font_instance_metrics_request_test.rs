use std::sync::{Arc, Mutex};

use tiqian::core::geometry::{text_range, LayoutConstraints};
use tiqian::core::text::Text;
use tiqian::core::text_model::{LayoutInput, RubySpan, TextSpan, TextStyle, TiqianTextContent};
use tiqian::font::font_metrics::{
    FontMetricsRequest, FontMetricsResolver, StubFontMetricsResolver,
};
use tiqian::font::font_policy::{FontRole, RawFontMetrics};
use tiqian::layout::paragraph_layout_engine::{
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
fn face_selection_uses_the_display_text_that_was_actually_shaped() {
    let requests = Arc::new(Mutex::new(Vec::new()));
    let mut engine = engine_with_requests(requests.clone());
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

    let requests = requests.lock().unwrap();
    assert!(
        requests
            .iter()
            .any(|request| request.face_selection_text == "⸺")
    );
}

#[test]
fn ruby_metrics_use_the_same_italic_instance_as_ruby_shaping() {
    let requests = Arc::new(Mutex::new(Vec::new()));
    let mut engine = engine_with_requests(requests.clone());
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

    let requests = requests.lock().unwrap();
    assert!(
        requests
            .iter()
            .any(|request| request.role == FontRole::LatinText
                && request.face_selection_text == "zhōng"
                && request.italic)
    );
}
