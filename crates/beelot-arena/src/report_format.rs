//! The number handling the arena reports depend on: fixed decimals, and compensated summation.

/// `x` with `decimals` decimals, preceded by its sign when it is positive and `plus` is set.
pub fn fixed(x: f64, decimals: usize, plus: bool) -> String {
    if plus {
        format!("{x:+.decimals$}")
    } else {
        format!("{x:.decimals$}")
    }
}

/// Kahan's compensated sum: the rounding error of each addition is carried into the next, so
/// summing many small values does not lose their digits. A sum that overflows to infinity stays
/// infinite instead of turning into NaN.
pub fn compensated_sum(values: impl IntoIterator<Item = f64>) -> f64 {
    let mut sum = 0.0_f64;
    // The compensation is kept negated, and subtracted at the end.
    let mut negated_compensation = 0.0_f64;
    let mut simple_sum = 0.0_f64;
    for value in values {
        let corrected = value - negated_compensation;
        let next = sum + corrected;
        negated_compensation = (next - sum) - corrected;
        sum = next;
        simple_sum += value;
    }
    let total = sum - negated_compensation;
    if total.is_nan() && simple_sum.is_infinite() {
        simple_sum
    } else {
        total
    }
}
