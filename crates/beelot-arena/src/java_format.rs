//! The parts of Java's number handling the arena report depends on, so that a report reads the
//! same, digit for digit, as the Java arena's.

/// Java's `String.format(Locale.ROOT, "%.Nf", x)`, or `"%+.Nf"` when `plus` is set.
///
/// Java rounds the shortest decimal form that reads back as `x`, half-up, rather than the exact
/// binary value: `1.005` is formatted as `1.01` with two decimals.
pub fn fixed(x: f64, decimals: usize, plus: bool) -> String {
    if x.is_nan() {
        return "NaN".to_string();
    }
    let sign = if x.is_sign_negative() {
        "-"
    } else if plus {
        "+"
    } else {
        ""
    };
    if x.is_infinite() {
        return format!("{sign}Infinity");
    }
    // The shortest round-trip digits and exponent: `1.005e0` is 1005 with the point after one digit.
    let scientific = format!("{:e}", x.abs());
    let (mantissa, exponent) = scientific
        .split_once('e')
        .expect("scientific notation has an exponent");
    let exponent: i64 = exponent.parse().expect("an integer exponent");
    let mut digits: Vec<u8> = mantissa
        .bytes()
        .filter(u8::is_ascii_digit)
        .map(|digit| digit - b'0')
        .collect();
    let mut point = exponent + 1;
    if point < 0 {
        let zeros = point.unsigned_abs() as usize;
        digits.splice(0..0, std::iter::repeat_n(0, zeros));
        point = 0;
    }
    let mut point = point as usize;
    let kept = point + decimals;
    let round_up = digits.get(kept).is_some_and(|&digit| digit >= 5);
    digits.resize(kept, 0);
    if round_up && increment(&mut digits) {
        digits.insert(0, 1);
        point += 1;
    }
    let (whole, fraction) = digits.split_at(point);
    let mut text = String::from(sign);
    let first_significant = whole
        .iter()
        .position(|&digit| digit != 0)
        .unwrap_or(whole.len());
    if first_significant == whole.len() {
        text.push('0');
    }
    text.extend(
        whole[first_significant..]
            .iter()
            .map(|&d| char::from(b'0' + d)),
    );
    if decimals > 0 {
        text.push('.');
        text.extend(fraction.iter().map(|&d| char::from(b'0' + d)));
    }
    text
}

/// Adds one to the last digit; returns whether the carry ran past the first digit.
fn increment(digits: &mut [u8]) -> bool {
    for digit in digits.iter_mut().rev() {
        if *digit == 9 {
            *digit = 0;
        } else {
            *digit += 1;
            return false;
        }
    }
    true
}

/// The sum `DoubleStream.sum()` returns: Kahan's compensated summation, as Java's
/// `Collectors.sumWithCompensation` and `Collectors.computeFinalSum` compute it.
/// `DoubleStream.average()` is this sum divided by the count.
pub fn java_sum(values: impl IntoIterator<Item = f64>) -> f64 {
    let mut sum = 0.0_f64;
    // Java keeps the compensation negated, and subtracts it at the end.
    let mut negated_compensation = 0.0_f64;
    let mut simple_sum = 0.0_f64;
    for value in values {
        let corrected = value - negated_compensation;
        let velvel = sum + corrected;
        negated_compensation = (velvel - sum) - corrected;
        sum = velvel;
        simple_sum += value;
    }
    let total = sum - negated_compensation;
    if total.is_nan() && simple_sum.is_infinite() {
        simple_sum
    } else {
        total
    }
}
