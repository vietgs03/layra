//! Fast SVG number formatting.
//!
//! `write!(svg, "{:.1}", x)` goes through `core::fmt` float machinery —
//! measurably slow when a 5k-node diagram emits ~100k coordinates. This
//! module formats `f32` with one decimal digit using integer math only,
//! writing straight into the output buffer with zero allocations.

/// Append `value` formatted with exactly one decimal digit (like `{:.1}`).
/// Falls back to std formatting for values beyond i64 range / NaN (never
/// happens for pixel coordinates).
#[inline]
pub(crate) fn push_f1(out: &mut String, value: f32) {
    let scaled_f = (value as f64) * 10.0;
    if !(i64::MIN as f64..=i64::MAX as f64).contains(&scaled_f) || scaled_f.is_nan() {
        use std::fmt::Write;
        let _ = write!(out, "{value:.1}");
        return;
    }
    let scaled = scaled_f.round() as i64;
    // Note: values rounding to zero emit "0.0", not std's "-0.0" — better
    // for SVG and one branch cheaper.
    if scaled < 0 {
        out.push('-');
    }
    let abs = scaled.unsigned_abs();
    push_u64(out, abs / 10);
    out.push('.');
    out.push((b'0' + (abs % 10) as u8) as char);
}

/// Append `value` formatted with no decimals (like `{:.0}`).
#[inline]
#[allow(dead_code)] // kept alongside push_f1; used as renderers migrate
pub(crate) fn push_f0(out: &mut String, value: f32) {
    let v = value as f64;
    if !(i64::MIN as f64..=i64::MAX as f64).contains(&v) || v.is_nan() {
        use std::fmt::Write;
        let _ = write!(out, "{value:.0}");
        return;
    }
    let r = v.round() as i64;
    if r < 0 {
        out.push('-');
    }
    push_u64(out, r.unsigned_abs());
}

/// Append a u64 as decimal digits using a stack buffer (no allocation).
#[inline]
fn push_u64(out: &mut String, mut v: u64) {
    if v == 0 {
        out.push('0');
        return;
    }
    let mut buf = [0u8; 20];
    let mut i = buf.len();
    while v > 0 {
        i -= 1;
        buf[i] = b'0' + (v % 10) as u8;
        v /= 10;
    }
    // Digits are ASCII, always valid UTF-8.
    out.push_str(std::str::from_utf8(&buf[i..]).unwrap());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f1(v: f32) -> String {
        let mut s = String::new();
        push_f1(&mut s, v);
        s
    }

    fn f0(v: f32) -> String {
        let mut s = String::new();
        push_f0(&mut s, v);
        s
    }

    #[test]
    fn f1_matches_std_formatting() {
        for v in [
            0.0f32, 1.0, -1.0, 3.15159, -3.15, 1234.56, 0.05, 99999.99, -99999.99, 0.949,
        ] {
            assert_eq!(f1(v), format!("{v:.1}"), "mismatch for {v}");
        }
    }

    #[test]
    fn f1_negative_zero_normalizes() {
        // std prints "-0.0"; we emit "0.0" (valid SVG, simpler).
        assert_eq!(f1(-0.04), "0.0");
    }

    #[test]
    fn f0_matches_std_formatting() {
        for v in [0.0f32, 1.4, -1.4, 712.0, 409.5] {
            assert_eq!(f0(v), format!("{v:.0}"), "mismatch for {v}");
        }
        assert_eq!(f0(-0.4), "0"); // normalized negative zero
    }
}
