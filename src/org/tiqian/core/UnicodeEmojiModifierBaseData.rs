/*
 * GENERATED from Unicode 17.0.0 emoji-data.txt by the Tiqian Unicode emoji generator.
 * Source: https://www.unicode.org/Public/17.0.0/ucd/emoji/emoji-data.txt
 * SHA-256: 2cb2bb9455cda83e8481541ecf5b6dfda66a3bb89efa3fa7c5297eccf607b72b
 * Copyright © 2025 Unicode, Inc.
 * Terms of Use: https://www.unicode.org/terms_of_use.html
 */

pub(crate) fn contains(code_point: i32) -> bool {
    contains_in_ranges(code_point, RANGES)
}

fn contains_in_ranges(code_point: i32, ranges: &[i32]) -> bool {
    let mut low = 0_usize;
    let mut high = ranges.len() / 2;
    while low < high {
        let middle = (low + high) / 2;
        let base = middle * 2;
        if code_point < ranges[base] {
            high = middle;
        } else if code_point > ranges[base + 1] {
            low = middle + 1;
        } else {
            return true;
        }
    }
    false
}

const RANGES: &[i32] = &[
    0x261D, 0x261D,
    0x26F9, 0x26F9,
    0x270A, 0x270D,
    0x1F385, 0x1F385,
    0x1F3C2, 0x1F3C4,
    0x1F3C7, 0x1F3C7,
    0x1F3CA, 0x1F3CC,
    0x1F442, 0x1F443,
    0x1F446, 0x1F450,
    0x1F466, 0x1F478,
    0x1F47C, 0x1F47C,
    0x1F481, 0x1F483,
    0x1F485, 0x1F487,
    0x1F48F, 0x1F48F,
    0x1F491, 0x1F491,
    0x1F4AA, 0x1F4AA,
    0x1F574, 0x1F575,
    0x1F57A, 0x1F57A,
    0x1F590, 0x1F590,
    0x1F595, 0x1F596,
    0x1F645, 0x1F647,
    0x1F64B, 0x1F64F,
    0x1F6A3, 0x1F6A3,
    0x1F6B4, 0x1F6B6,
    0x1F6C0, 0x1F6C0,
    0x1F6CC, 0x1F6CC,
    0x1F90C, 0x1F90C,
    0x1F90F, 0x1F90F,
    0x1F918, 0x1F91F,
    0x1F926, 0x1F926,
    0x1F930, 0x1F939,
    0x1F93C, 0x1F93E,
    0x1F977, 0x1F977,
    0x1F9B5, 0x1F9B6,
    0x1F9B8, 0x1F9B9,
    0x1F9BB, 0x1F9BB,
    0x1F9CD, 0x1F9CF,
    0x1F9D1, 0x1F9DD,
    0x1FAC3, 0x1FAC5,
    0x1FAF0, 0x1FAF8,
];
