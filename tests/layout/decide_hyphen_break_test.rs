use tiqian::common::HashSet;

use tiqian::core::geometry::TextRange;
use tiqian::core::layout_model::Cluster;
use tiqian::core::text::Text;
use tiqian::layout::progressive_break_decisions::decide_hyphen_break;

fn cluster(start: i32, advance: f32) -> Cluster {
    Cluster::new(
        TextRange::new(start, start + 1),
        Text::from("x"),
        "k".to_owned(),
        advance,
    )
}

fn clusters() -> Vec<Cluster> {
    vec![
        cluster(0, 16.0),
        cluster(1, 16.0),
        cluster(2, 32.0),
        cluster(3, 32.0),
        cluster(4, 32.0),
    ]
}

#[test]
fn charges_all_deficit_to_cjk_when_no_sino_western_capacity_is_known() {
    assert_eq!(
        4,
        decide_hyphen_break(
            0,
            4,
            &clusters(),
            74.0,
            &HashSet::from([4]),
            &HashSet::from([1]),
            8.0,
            &HashSet::new(),
            0.0,
        ),
    );
}

#[test]
fn discounts_sino_western_capacity_before_charging_cjk_looseness() {
    assert_eq!(
        3,
        decide_hyphen_break(
            0,
            4,
            &clusters(),
            74.0,
            &HashSet::from([4]),
            &HashSet::from([1]),
            8.0,
            &HashSet::from([2]),
            4.0,
        ),
    );
}
