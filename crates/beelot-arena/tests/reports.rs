//! The arena reports' number formatting and summation.

use beelot_arena::report_format::{compensated_sum, fixed};

#[test]
fn fixed_pads_and_signs_to_the_decimals() {
    assert_eq!(fixed(1.0 / 3.0, 3, false), "0.333");
    assert_eq!(fixed(3.0, 2, true), "+3.00");
    assert_eq!(fixed(-0.001, 2, true), "-0.00");
    assert_eq!(fixed(0.0, 2, true), "+0.00");
    assert_eq!(fixed(9.96, 1, false), "10.0");
    assert_eq!(fixed(123456.0, 1, false), "123456.0");
    assert_eq!(fixed(-39.3, 2, true), "-39.30");
}

#[test]
fn compensated_sum_compensates() {
    assert_eq!(compensated_sum(std::iter::repeat_n(0.1, 10)), 1.0);
}

#[test]
fn compensated_sum_keeps_an_infinite_simple_sum() {
    assert_eq!(compensated_sum([f64::INFINITY, 1.0]), f64::INFINITY);
    assert_eq!(compensated_sum([]), 0.0);
}
