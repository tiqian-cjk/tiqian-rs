use tiqian::layout::Justifier::Justifier;
use tiqian::layout::ProgressiveBreakDecisions::{ShrinkChannel, ShrinkOpportunity};

fn opportunity(cluster_index: i32, tier: i32, capacity: f32) -> ShrinkOpportunity {
    ShrinkOpportunity::new(cluster_index, tier, capacity, ShrinkChannel::TrailingGlue)
}

#[test]
fn consumes_tiers_in_ascending_order() {
    let plan = Justifier::default().compress(
        3.0,
        &[
            opportunity(0, 1, 2.0),
            opportunity(1, 2, 5.0),
            opportunity(2, 3, 5.0),
        ],
    );

    assert_eq!(0.0, plan.unfilled_surplus);
    assert_eq!(
        2.0,
        plan.allocations
            .iter()
            .find(|allocation| allocation.cluster_index == 0)
            .unwrap()
            .shrink
    );
    assert_eq!(
        1.0,
        plan.allocations
            .iter()
            .find(|allocation| allocation.cluster_index == 1)
            .unwrap()
            .shrink
    );
    assert!(
        plan.allocations
            .iter()
            .all(|allocation| allocation.cluster_index != 2)
    );
}

#[test]
fn shares_equal_fraction_within_a_tier() {
    let plan =
        Justifier::default().compress(4.0, &[opportunity(0, 2, 2.0), opportunity(1, 2, 6.0)]);

    assert_eq!(
        1.0,
        plan.allocations
            .iter()
            .find(|allocation| allocation.cluster_index == 0)
            .unwrap()
            .shrink
    );
    assert_eq!(
        3.0,
        plan.allocations
            .iter()
            .find(|allocation| allocation.cluster_index == 1)
            .unwrap()
            .shrink
    );
    assert_eq!(0.0, plan.unfilled_surplus);
}

#[test]
fn reports_unfilled_when_capacity_is_exhausted() {
    let plan =
        Justifier::default().compress(5.0, &[opportunity(0, 1, 1.0), opportunity(1, 2, 1.0)]);

    assert_eq!(3.0, plan.unfilled_surplus);
    assert_eq!(2, plan.allocations.len());
}

#[test]
fn zero_surplus_is_no_op() {
    let plan = Justifier::default().compress(0.0, &[opportunity(0, 1, 5.0)]);
    assert!(plan.allocations.is_empty());
    assert_eq!(0.0, plan.unfilled_surplus);
}
