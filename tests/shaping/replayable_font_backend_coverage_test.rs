use std::any::Any;

use tiqian::common::{HashMap, HashSet};
use tiqian::core::text::Text;
use tiqian::font::font_policy::FontRole;
use tiqian::shaping::replayable_font_backend::{
    FontBackendCapabilityIssue, FontBackendCapabilityReport, FontFaceId, ReplayableFontCatalog,
    ReplayableFontFaceDescriptor, ReplayableFontFaceRequest,
};

fn descriptor(id: &str, family: &str, role: FontRole) -> ReplayableFontFaceDescriptor {
    ReplayableFontFaceDescriptor::new(
        FontFaceId::new(id.to_owned()),
        HashSet::from([family.to_owned()]),
        HashSet::from([role]),
        "bytes".to_owned(),
    )
}

fn request(role: FontRole, family: &str) -> ReplayableFontFaceRequest {
    ReplayableFontFaceRequest::new(
        role,
        vec![family.to_owned()],
        12.0,
        400,
        false,
        "zh-CN".to_owned(),
        Text::from("A"),
    )
}

fn panic_message(error: Box<dyn Any + Send>) -> String {
    if let Some(message) = error.downcast_ref::<String>() {
        message.clone()
    } else if let Some(message) = error.downcast_ref::<&str>() {
        (*message).to_owned()
    } else {
        "non-string panic payload".to_owned()
    }
}

#[test]
fn font_face_id_rejects_blank_and_keeps_value() {
    let id = FontFaceId::new("noto-cjk-1".to_owned());
    assert_eq!("noto-cjk-1", id.value());
    assert_eq!("noto-cjk-1", id.to_string());

    let blank = std::panic::catch_unwind(|| FontFaceId::new(" ".to_owned()))
        .expect_err("blank FontFaceId must panic");
    assert!(panic_message(blank).contains("blank"));
    assert!(std::panic::catch_unwind(|| FontFaceId::new(String::new())).is_err());
}

#[test]
fn face_descriptor_defaults_are_stable() {
    let descriptor = descriptor("face-a", "Serif", FontRole::CjkText);
    assert_eq!(400, descriptor.weight);
    assert!(!descriptor.italic);
    assert_eq!(0, descriptor.collection_index);
    assert!(descriptor.variation_axes.is_empty());
    assert_eq!(FontFaceId::new("face-a".to_owned()), descriptor.id);

    let varied = ReplayableFontFaceDescriptor::builder(
        descriptor.id.clone(),
        descriptor.family_aliases.clone(),
        descriptor.roles.clone(),
        descriptor.source_label.clone(),
    )
    .weight(700)
    .italic(true)
    .collection_index(2)
    .variation_axes(HashMap::from([("wght".to_owned(), 700.0)]))
    .build();
    assert_eq!(700, varied.weight);
    assert!(varied.italic);
    assert_eq!(2, varied.collection_index);
    assert_eq!(Some(&700.0), varied.variation_axes.get("wght"));
}

#[test]
fn face_request_rejects_non_positive_and_non_finite_font_size() {
    let valid = ReplayableFontFaceRequest::new(
        FontRole::LatinText,
        vec!["Plex".to_owned()],
        15.0,
        400,
        false,
        "zh-CN".to_owned(),
        Text::from("A"),
    );
    assert_eq!(FontRole::LatinText, valid.role);
    assert_eq!(15.0, valid.font_size);

    let invalid = |font_size| {
        std::panic::catch_unwind(|| {
            ReplayableFontFaceRequest::new(
                FontRole::LatinText,
                Vec::new(),
                font_size,
                400,
                false,
                String::new(),
                Text::from("A"),
            )
        })
    };
    let zero = invalid(0.0).expect_err("zero font size must panic");
    assert!(panic_message(zero).contains("positive and finite"));
    assert!(invalid(-1.0).is_err());
    assert!(invalid(f32::NAN).is_err());
    assert!(invalid(f32::INFINITY).is_err());
}

#[test]
fn capability_report_replay_flag_requires_faces_and_no_missing_face_issue() {
    let face = descriptor("face-a", "Serif", FontRole::CjkText);
    assert!(
        !FontBackendCapabilityReport::new("b".to_owned(), "k".to_owned(), Vec::new())
            .can_replay_from_controlled_bytes()
    );
    assert!(
        !FontBackendCapabilityReport::with_issues(
            "b".to_owned(),
            "k".to_owned(),
            vec![face.clone()],
            vec![FontBackendCapabilityIssue {
                code: "MissingControlledFontFace".to_owned(),
                detail: "gone".to_owned(),
            }],
        )
        .can_replay_from_controlled_bytes()
    );
    assert!(
        FontBackendCapabilityReport::with_issues(
            "b".to_owned(),
            "k".to_owned(),
            vec![face.clone()],
            vec![FontBackendCapabilityIssue {
                code: "Other".to_owned(),
                detail: "note".to_owned(),
            }],
        )
        .can_replay_from_controlled_bytes()
    );
    assert!(
        FontBackendCapabilityReport::new("b".to_owned(), "k".to_owned(), vec![face])
            .can_replay_from_controlled_bytes()
    );
}

struct TestCatalog {
    faces: Vec<ReplayableFontFaceDescriptor>,
    capability_report: FontBackendCapabilityReport,
}

impl ReplayableFontCatalog for TestCatalog {
    fn faces(&self) -> &[ReplayableFontFaceDescriptor] {
        &self.faces
    }

    fn capability_report(&self) -> &FontBackendCapabilityReport {
        &self.capability_report
    }

    fn resolve(&self, request: &ReplayableFontFaceRequest) -> Option<ReplayableFontFaceDescriptor> {
        self.faces
            .iter()
            .find(|face| {
                face.roles.contains(&request.role)
                    && request
                        .preferred_families
                        .iter()
                        .any(|family| face.family_aliases.contains(family))
            })
            .cloned()
    }
}

#[test]
fn catalog_contract_resolves_by_request() {
    let faces = vec![
        descriptor("face-cjk", "Noto Serif CJK", FontRole::CjkText),
        descriptor("face-latin", "Plex", FontRole::LatinText),
    ];
    let catalog = TestCatalog {
        capability_report: FontBackendCapabilityReport::new(
            "test".to_owned(),
            "bytes".to_owned(),
            faces.clone(),
        ),
        faces,
    };
    assert!(catalog.capability_report().can_replay_from_controlled_bytes());
    let hit = catalog
        .resolve(&request(FontRole::LatinText, "Plex"))
        .expect("matching catalog face");
    assert_eq!(FontFaceId::new("face-latin".to_owned()), hit.id);
    assert!(catalog.resolve(&request(FontRole::LatinText, "Missing")).is_none());
}