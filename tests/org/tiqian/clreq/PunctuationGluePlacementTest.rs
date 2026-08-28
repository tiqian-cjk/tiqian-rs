use tiqian::org::tiqian::clreq::ClreqProfile::{
    ClreqProfile, ClreqRegion, GlueSide, PunctuationClass, PunctuationGluePlacement,
};

#[test]
fn mainland_anchors_closing_and_pause_stop_to_trailing() {
    let placement = PunctuationGluePlacement::MainlandSimplified;
    assert_eq!(
        GlueSide::TrailingOnly,
        placement.glue_side_for(PunctuationClass::Closing)
    );
    assert_eq!(
        GlueSide::TrailingOnly,
        placement.glue_side_for(PunctuationClass::PauseOrStop)
    );
}

#[test]
fn mainland_anchors_opening_to_leading() {
    assert_eq!(
        GlueSide::LeadingOnly,
        PunctuationGluePlacement::MainlandSimplified.glue_side_for(PunctuationClass::Opening)
    );
}

#[test]
fn mainland_splits_symmetric_punctuation_on_both_sides() {
    let placement = PunctuationGluePlacement::MainlandSimplified;
    for class in [
        PunctuationClass::MiddleDot,
        PunctuationClass::Ellipsis,
        PunctuationClass::Dash,
        PunctuationClass::Quote,
    ] {
        assert_eq!(GlueSide::BothSides, placement.glue_side_for(class));
    }
}

#[test]
fn traditional_centres_opening_closing_and_pause_stop() {
    let placement = PunctuationGluePlacement::Traditional;
    for class in [
        PunctuationClass::Opening,
        PunctuationClass::Closing,
        PunctuationClass::PauseOrStop,
    ] {
        assert_eq!(GlueSide::BothSides, placement.glue_side_for(class));
    }
}

#[test]
fn region_and_built_in_profiles_use_expected_placement() {
    assert_eq!(
        PunctuationGluePlacement::MainlandSimplified,
        PunctuationGluePlacement::for_region(ClreqRegion::Mainland)
    );
    assert_eq!(
        PunctuationGluePlacement::Traditional,
        PunctuationGluePlacement::for_region(ClreqRegion::Taiwan)
    );
    assert_eq!(
        PunctuationGluePlacement::Traditional,
        PunctuationGluePlacement::for_region(ClreqRegion::HongKong)
    );
    assert_eq!(
        PunctuationGluePlacement::MainlandSimplified,
        PunctuationGluePlacement::for_region(ClreqRegion::Custom)
    );
    assert_eq!(
        PunctuationGluePlacement::Traditional,
        ClreqProfile::taiwan_horizontal().glue_placement
    );
    assert_eq!(
        PunctuationGluePlacement::Traditional,
        ClreqProfile::hong_kong_horizontal().glue_placement
    );
    assert_eq!(
        PunctuationGluePlacement::MainlandSimplified,
        ClreqProfile::mainland_horizontal().glue_placement
    );
}
