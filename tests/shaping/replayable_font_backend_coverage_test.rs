use std::any::Any;

use tiqian::common::HashSet;
use tiqian::core::font_face::{FontFaceId, FontVariationInstance, FontVariationSetting};
use tiqian::font::font_policy::FontRole;
use tiqian::shaping::replayable_font_backend::{
    FontBackendCapabilityIssue, FontBackendCapabilityReport, ReplayableFontCatalog,
    ReplayableFontFaceDescriptor,
};

fn face_id(resource_id: &str) -> FontFaceId {
    FontFaceId::new(
        resource_id.to_owned(),
        0,
        FontVariationInstance::default(),
    )
}

fn descriptor(id: FontFaceId, family: &str, role: FontRole) -> ReplayableFontFaceDescriptor {
    ReplayableFontFaceDescriptor::new(
        id,
        HashSet::from([family.to_owned()]),
        HashSet::from([role]),
        "bytes".to_owned(),
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
fn font_face_id_keeps_resource_collection_and_canonical_variations() {
    let id = FontFaceId::new(
        "noto-cjk".to_owned(),
        2,
        FontVariationInstance::new(vec![
            FontVariationSetting::new("wdth".to_owned(), 75.0),
            FontVariationSetting::new("wght".to_owned(), 700.0),
        ]),
    );

    assert_eq!("noto-cjk", id.resource_id());
    assert_eq!(2, id.collection_index());
    assert_eq!("wdth", id.variation_instance().settings()[0].tag());
    assert_eq!("wght", id.variation_instance().settings()[1].tag());
    assert_eq!("noto-cjk#2@wdth=75,wght=700", id.to_string());

    let blank = std::panic::catch_unwind(|| face_id(" "))
        .expect_err("blank FontFaceId resource must panic");
    assert!(panic_message(blank).contains("blank"));
    assert!(std::panic::catch_unwind(|| {
        FontVariationInstance::new(vec![
            FontVariationSetting::new("wght".to_owned(), 400.0),
            FontVariationSetting::new("wght".to_owned(), 700.0),
        ])
    })
    .is_err());
}

#[test]
fn face_descriptor_defaults_are_stable() {
    let descriptor = descriptor(face_id("face-a"), "Serif", FontRole::CjkText);
    assert_eq!(400, descriptor.weight);
    assert!(!descriptor.italic);
    assert_eq!("face-a", descriptor.id.resource_id());
    assert_eq!(0, descriptor.id.collection_index());
    assert!(descriptor.id.variation_instance().settings().is_empty());

    let varied = ReplayableFontFaceDescriptor::builder(
        FontFaceId::new(
            "face-a".to_owned(),
            2,
            FontVariationInstance::new(vec![FontVariationSetting::new(
                "wght".to_owned(),
                700.0,
            )]),
        ),
        descriptor.family_aliases.clone(),
        descriptor.roles.clone(),
        descriptor.source_label.clone(),
    )
    .weight(700)
    .italic(true)
    .build();
    assert_eq!(700, varied.weight);
    assert!(varied.italic);
    assert_eq!(2, varied.id.collection_index());
    assert_eq!("wght", varied.id.variation_instance().settings()[0].tag());
}

#[test]
fn capability_report_replay_flag_requires_faces_and_no_missing_face_issue() {
    let face = descriptor(face_id("face-a"), "Serif", FontRole::CjkText);
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

    fn face(&self, id: &FontFaceId) -> Option<&ReplayableFontFaceDescriptor> {
        self.faces.iter().find(|face| face.id == *id)
    }
}

#[test]
fn catalog_exposes_exact_replayable_face_identity() {
    let cjk = descriptor(face_id("face-cjk"), "Noto Serif CJK", FontRole::CjkText);
    let latin = descriptor(face_id("face-latin"), "Plex", FontRole::LatinText);
    let catalog = TestCatalog {
        capability_report: FontBackendCapabilityReport::new(
            "test".to_owned(),
            "bytes".to_owned(),
            vec![cjk.clone(), latin.clone()],
        ),
        faces: vec![cjk, latin.clone()],
    };

    assert!(catalog.capability_report().can_replay_from_controlled_bytes());
    assert_eq!(2, catalog.faces().len());
    assert_eq!(Some(&latin), catalog.face(&face_id("face-latin")));
    assert!(catalog.face(&face_id("missing")).is_none());
}
