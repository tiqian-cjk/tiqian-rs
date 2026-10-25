use tiqian::common::HashSet;
use tiqian::core::font_face::{FontFaceId, FontVariationInstance};
use tiqian::core::layout_model::{Cluster, Glyph, GlyphRun, ShapingDecisionInfo};
use tiqian::font::font_metrics::{FontMetricSource, FontMetricsRequest};
use tiqian::font::font_policy::{FontRole, RawFontMetrics};
use tiqian::shaping::font_backend::{
    FontBackend, FontBackendRequest, FontBackendShapingResult, FontCandidateAttempt,
};
use tiqian::shaping::replayable_font_backend::{
    FontBackendCapabilityReport, ReplayableFontCatalog, ReplayableFontFaceDescriptor,
};

#[derive(Clone, Debug)]
pub struct DeterministicStubFontBackend {
    faces: Vec<ReplayableFontFaceDescriptor>,
    capability_report: FontBackendCapabilityReport,
}

impl Default for DeterministicStubFontBackend {
    fn default() -> Self {
        let faces = vec![
            stub_face("cjk-primary", [FontRole::CjkText, FontRole::CjkPunctuation]),
            stub_face("latin-primary", [FontRole::LatinText]),
            stub_face(
                "symbol-fallback",
                [FontRole::Symbol, FontRole::Emoji, FontRole::Unknown],
            ),
        ];
        let capability_report = FontBackendCapabilityReport::new(
            "DeterministicStubFontBackend".to_owned(),
            "deterministic-fixture".to_owned(),
            faces.clone(),
        );
        Self {
            faces,
            capability_report,
        }
    }
}

impl FontBackend for DeterministicStubFontBackend {
    fn shape(&self, request: &FontBackendRequest) -> FontBackendShapingResult {
        let face = self
            .faces
            .iter()
            .find(|descriptor| descriptor.roles.contains(&request.role))
            .expect("deterministic stub must contain a face for every FontRole");
        let source_text = request.text.slice_text(request.range);
        let glyph_count = request.display_text.chars().count().max(1) as i32;
        let advance =
            request.style.font_size * nominal_advance_em(&source_text, &request.display_text);
        let cluster = Cluster::with_display_text(
            request.range,
            source_text.clone(),
            request.display_text.clone(),
            face.id.clone(),
            advance,
        );
        let glyph_advance = advance / glyph_count as f32;
        let glyphs = (0..glyph_count)
            .map(|glyph_id| {
                Glyph::builder(glyph_id as u32, request.range, glyph_advance)
                    .x(glyph_advance * glyph_id as f32)
                    .render_font_face(Some(face.id.clone()))
                    .build()
            })
            .collect();
        let run = GlyphRun::new(request.range, face.id.clone(), glyphs, advance);
        let decision = ShapingDecisionInfo::builder(
            request.range,
            source_text,
            request.display_text.clone(),
            Some(face.id.clone()),
            glyph_count,
            advance,
            "Stub".to_owned(),
            "DeterministicStubFontBackend:nominal-em-advance".to_owned(),
        )
        .glyphs_without_ink_bounds(glyph_count)
        .build();
        FontBackendShapingResult::new(
            face.id.clone(),
            tiqian::shaping::text_shaper::ShapingResult::with_decisions(
                vec![cluster],
                vec![run],
                vec![decision],
            ),
            vec![FontCandidateAttempt::new(
                face.source_label.clone(),
                face.id.clone(),
                0,
            )],
        )
    }

    fn metrics(&self, request: &FontMetricsRequest) -> RawFontMetrics {
        raw_metrics_for_role(request.font_size, request.role)
    }
}

fn raw_metrics_for_role(font_size: f32, role: FontRole) -> RawFontMetrics {
    match role {
        FontRole::CjkText | FontRole::CjkPunctuation => RawFontMetrics {
            ascent: font_size * 1.16,
            descent: font_size * 0.288,
            leading: 0.0,
            source: FontMetricSource::RawTables,
            typo_ascent: Some(font_size * 0.88),
            typo_descent: Some(font_size * 0.12),
        },
        FontRole::LatinText => RawFontMetrics {
            ascent: font_size * 0.8,
            descent: font_size * 0.2,
            leading: 0.0,
            source: FontMetricSource::RawTables,
            typo_ascent: None,
            typo_descent: None,
        },
        FontRole::Symbol | FontRole::Emoji | FontRole::Unknown => RawFontMetrics {
            ascent: font_size * 0.9,
            descent: font_size * 0.25,
            leading: 0.0,
            source: FontMetricSource::RawTables,
            typo_ascent: None,
            typo_descent: None,
        },
    }
}

fn nominal_advance_em(
    source_text: &tiqian::core::text::Text,
    display_text: &tiqian::core::text::Text,
) -> f32 {
    if source_text == "⸺" || display_text == "⸺" {
        2.0
    } else if !source_text.is_empty() && source_text.chars().all(|character| character == ' ') {
        0.5 * source_text.scalar_len().value() as f32
    } else {
        source_text
            .chars()
            .count()
            .max(display_text.chars().count()) as f32
    }
}

impl ReplayableFontCatalog for DeterministicStubFontBackend {
    fn faces(&self) -> &[ReplayableFontFaceDescriptor] {
        &self.faces
    }

    fn capability_report(&self) -> &FontBackendCapabilityReport {
        &self.capability_report
    }

    fn face(&self, id: &FontFaceId) -> Option<&ReplayableFontFaceDescriptor> {
        self.faces.iter().find(|descriptor| descriptor.id == *id)
    }
}

fn stub_face<const N: usize>(
    resource_id: &str,
    roles: [FontRole; N],
) -> ReplayableFontFaceDescriptor {
    let id = FontFaceId::new(resource_id.to_owned(), 0, FontVariationInstance::default());
    ReplayableFontFaceDescriptor::new(
        id,
        HashSet::from([resource_id.to_owned()]),
        roles.into_iter().collect(),
        resource_id.to_owned(),
    )
}
