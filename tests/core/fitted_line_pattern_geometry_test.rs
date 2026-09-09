use tiqian::core::fitted_line_pattern_geometry::{
    fitted_dashed_line_segments, fitted_dotted_line_centers,
};

#[test]
fn dashed_remainder_is_shared_between_full_edge_anchored_dashes() {
    assert_eq!(
        vec![0.0, 2.0, 4.5, 6.5, 9.0, 11.0],
        fitted_dashed_line_segments(0.0, 11.0, 2.0, 2.0),
    );
}

#[test]
fn short_dashed_span_becomes_one_visible_dash() {
    assert_eq!(vec![0.0, 3.0], fitted_dashed_line_segments(0.0, 3.0, 2.0, 2.0));
}

#[test]
fn remainder_is_shared_so_both_span_edges_have_complete_dots() {
    assert_eq!(
        vec![1.0, 5.5, 10.0],
        fitted_dotted_line_centers(0.0, 11.0, 0.0, 11.0, 2.0, 2.0),
    );
}

#[test]
fn skip_ink_intervals_keep_the_fitted_span_pattern_without_cut_dots() {
    assert_eq!(
        vec![5.0],
        fitted_dotted_line_centers(0.0, 10.0, 2.0, 8.0, 2.0, 2.0),
    );
}

#[test]
fn a_span_shorter_than_one_dot_still_paints_one_centered_dot() {
    assert_eq!(
        vec![0.5],
        fitted_dotted_line_centers(0.0, 1.0, 0.0, 1.0, 2.0, 2.0),
    );
}