use std::cmp::min;

/// Computes end-anchored dot centers for a dotted line.
///
/// The first and last dots meet the span edges. `kept_left` and `kept_right` only clip complete
/// dots for skip-ink; they do not restart the fitted pattern.
pub fn fitted_dotted_line_centers(
    span_left: f32,
    span_right: f32,
    kept_left: f32,
    kept_right: f32,
    dot_diameter: f32,
    gap_length: f32,
) -> Vec<f32> {
    assert!(
        span_left.is_finite()
            && span_right.is_finite()
            && kept_left.is_finite()
            && kept_right.is_finite()
    );
    assert!(dot_diameter.is_finite() && dot_diameter > 0.0);
    assert!(gap_length.is_finite() && gap_length >= 0.0);
    if span_right <= span_left || kept_right <= kept_left {
        return Vec::new();
    }

    let radius = dot_diameter / 2.0;
    let span_width = span_right - span_left;
    let target_pitch = dot_diameter + gap_length;
    let fitted_count = ((span_width + gap_length) / target_pitch).round().max(1.0) as i32;
    let non_overlapping_count = (span_width / dot_diameter).floor().max(1.0) as i32;
    let count = min(fitted_count, non_overlapping_count);

    if count == 1 {
        let center = (span_left + span_right) / 2.0;
        let complete_dot_fits = center - radius >= kept_left - DOTTED_CENTER_EPSILON
            && center + radius <= kept_right + DOTTED_CENTER_EPSILON;
        let short_span_is_fully_kept = span_width < dot_diameter
            && kept_left <= span_left + DOTTED_CENTER_EPSILON
            && kept_right >= span_right - DOTTED_CENTER_EPSILON;
        return if complete_dot_fits || short_span_is_fully_kept {
            vec![center]
        } else {
            Vec::new()
        };
    }

    let first_center = span_left + radius;
    let fitted_pitch = (span_width - dot_diameter) / (count - 1) as f32;
    (0..count)
        .map(|index| first_center + index as f32 * fitted_pitch)
        .filter(|center| {
            *center - radius >= kept_left - DOTTED_CENTER_EPSILON
                && *center + radius <= kept_right + DOTTED_CENTER_EPSILON
        })
        .collect()
}

const DOTTED_CENTER_EPSILON: f32 = 0.001;

/// Computes end-anchored visible `[left, right]` pairs for a dashed line.
pub fn fitted_dashed_line_segments(
    span_left: f32,
    span_right: f32,
    dash_length: f32,
    gap_length: f32,
) -> Vec<f32> {
    assert!(span_left.is_finite() && span_right.is_finite());
    assert!(dash_length.is_finite() && dash_length > 0.0);
    assert!(gap_length.is_finite() && gap_length >= 0.0);
    if span_right <= span_left {
        return Vec::new();
    }

    let span_width = span_right - span_left;
    if span_width < dash_length * 2.0 {
        return vec![span_left, span_right];
    }

    let fitted_count = ((span_width + gap_length) / (dash_length + gap_length))
        .round()
        .max(2.0) as i32;
    let non_overlapping_count = (span_width / dash_length).floor().max(2.0) as i32;
    let count = min(fitted_count, non_overlapping_count);
    let fitted_gap = (span_width - count as f32 * dash_length) / (count - 1) as f32;

    (0..count * 2)
        .map(|coordinate_index| {
            let dash_index = coordinate_index / 2;
            let dash_left = span_left + dash_index as f32 * (dash_length + fitted_gap);
            if coordinate_index % 2 == 0 {
                dash_left
            } else {
                dash_left + dash_length
            }
        })
        .collect()
}