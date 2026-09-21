use crate::support::DeterministicStubFontBackend;
use tiqian::core::font_face::FontFaceId;
use tiqian::font::font_metrics::FontMetricsRequest;
use tiqian::font::font_policy::RawFontMetrics;
use tiqian::shaping::font_backend::{FontBackend, FontBackendRequest, FontBackendShapingResult};
use tiqian::shaping::replayable_font_backend::{
    FontBackendCapabilityReport, ReplayableFontCatalog, ReplayableFontFaceDescriptor,
};

pub struct StubResultTransformFontBackend<F> {
    fallback: DeterministicStubFontBackend,
    transform: F,
}

pub fn stub_backend_with_transform<F>(transform: F) -> StubResultTransformFontBackend<F> {
    StubResultTransformFontBackend {
        fallback: DeterministicStubFontBackend::default(),
        transform,
    }
}

impl<F> ReplayableFontCatalog for StubResultTransformFontBackend<F> {
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

impl<F> FontBackend for StubResultTransformFontBackend<F>
where
    F: Fn(&FontBackendRequest, FontBackendShapingResult) -> FontBackendShapingResult + Send + Sync,
{
    fn shape(&self, request: &FontBackendRequest) -> FontBackendShapingResult {
        (self.transform)(request, self.fallback.shape(request))
    }

    fn metrics(&self, request: &FontMetricsRequest) -> RawFontMetrics {
        self.fallback.metrics(request)
    }
}
