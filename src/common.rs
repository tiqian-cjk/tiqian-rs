#[cfg(all(feature = "collections-hashbrown", feature = "collections-std"))]
compile_error!("only one collection implementation may be enabled");

#[cfg(not(any(feature = "collections-hashbrown", feature = "collections-std")))]
compile_error!("a collection implementation must be enabled");

#[cfg(feature = "collections-hashbrown")]
pub use hashbrown::{HashMap, HashSet};

#[cfg(feature = "collections-std")]
pub use std::collections::{HashMap, HashSet};

#[cfg(all(feature = "no-std", feature = "runtime-std"))]
compile_error!("no-std cannot be combined with runtime-std or std components");

#[cfg(all(feature = "math-std", feature = "math-libm"))]
compile_error!("only one math implementation may be enabled");

#[cfg(not(any(feature = "math-std", feature = "math-libm")))]
compile_error!("a math implementation must be enabled");

#[cfg(all(feature = "shared-auto", feature = "shared-portable"))]
compile_error!("only one shared pointer implementation may be enabled");

#[cfg(not(any(feature = "shared-auto", feature = "shared-portable")))]
compile_error!("a shared pointer implementation must be enabled");

#[cfg(all(feature = "shared-auto", target_has_atomic = "ptr"))]
pub use alloc::sync::Arc;

#[cfg(any(
    feature = "shared-portable",
    all(feature = "shared-auto", not(target_has_atomic = "ptr"))
))]
pub use portable_atomic_util::Arc;

#[cfg(feature = "math-std")]
pub(crate) use f32::{floor, round};

#[cfg(all(feature = "math-libm", not(feature = "math-std")))]
pub(crate) use libm::{floorf as floor, roundf as round};

#[cfg(feature = "math-std")]
mod f32 {
    #[inline]
    pub(crate) fn floor(value: f32) -> f32 {
        value.floor()
    }

    #[inline]
    pub(crate) fn round(value: f32) -> f32 {
        value.round()
    }
}

#[cfg(test)]
mod tests {
    use super::{floor, round};

    #[test]
    fn rounding_preserves_boundary_semantics() {
        for (input, expected_floor, expected_round) in [
            (-1.5, -2.0, -2.0),
            (-0.5, -1.0, -1.0),
            (0.5, 0.0, 1.0),
            (1.5, 1.0, 2.0),
            (2.0, 2.0, 2.0),
            (f32::from_bits(2.0_f32.to_bits() - 1), 1.0, 2.0),
            (f32::from_bits(2.0_f32.to_bits() + 1), 2.0, 2.0),
        ] {
            assert_eq!(floor(input), expected_floor);
            assert_eq!(round(input), expected_round);
        }
        for value in [0.0_f32, -0.0, f32::INFINITY, f32::NEG_INFINITY] {
            assert_eq!(floor(value).to_bits(), value.to_bits());
            assert_eq!(round(value).to_bits(), value.to_bits());
        }
        assert_eq!(round(-0.25).to_bits(), (-0.0_f32).to_bits());
        assert!(floor(f32::NAN).is_nan());
        assert!(round(f32::NAN).is_nan());
    }
}
