//! L4 contract: measured label width must be a safe **upper bound** on the
//! real rendered text advance, so a label never overflows its node box.
//!
//! Ground truth = Helvetica/Arial AFM advance widths (em = units/1000), the
//! widest of the common UI sans stack (Inter is narrower, so this bounds it
//! too). The measurement table must be >= this reference for every glyph and
//! for a battery of whole strings including CJK and emoji.

use layra_text::measure_line;

/// Reference advance (em) per char from Helvetica/Arial AFM metrics, with
/// full-width CJK at 1.0 and emoji at 1.2 (emoji glyphs render wider than a
/// nominal em advance). This is the *real* advance the engine must not
/// under-measure.
fn ref_em(c: char) -> f32 {
    match c {
        ' ' => 0.278,
        '!' => 0.278,
        '"' => 0.355,
        '#' => 0.556,
        '$' => 0.556,
        '%' => 0.889,
        '&' => 0.667,
        '\'' => 0.191,
        '(' | ')' => 0.333,
        '*' => 0.389,
        '+' | '<' | '=' | '>' | '~' => 0.584,
        ',' => 0.278,
        '-' => 0.333,
        '.' => 0.278,
        '/' => 0.278,
        '0'..='9' => 0.556,
        ':' | ';' => 0.278,
        '?' => 0.556,
        '@' => 1.015,
        'A' => 0.667,
        'B' => 0.667,
        'C' => 0.722,
        'D' => 0.722,
        'E' => 0.667,
        'F' => 0.611,
        'G' => 0.778,
        'H' => 0.722,
        'I' => 0.278,
        'J' => 0.5,
        'K' => 0.667,
        'L' => 0.556,
        'M' => 0.833,
        'N' => 0.722,
        'O' => 0.778,
        'P' => 0.667,
        'Q' => 0.778,
        'R' => 0.722,
        'S' => 0.667,
        'T' => 0.611,
        'U' => 0.722,
        'V' => 0.667,
        'W' => 0.944,
        'X' => 0.667,
        'Y' => 0.667,
        'Z' => 0.611,
        '[' | ']' => 0.278,
        '\\' => 0.278,
        '^' => 0.469,
        '_' => 0.556,
        '`' => 0.333,
        'a' => 0.556,
        'b' => 0.556,
        'c' => 0.5,
        'd' => 0.556,
        'e' => 0.556,
        'f' => 0.278,
        'g' => 0.556,
        'h' => 0.556,
        'i' => 0.222,
        'j' => 0.222,
        'k' => 0.5,
        'l' => 0.222,
        'm' => 0.833,
        'n' => 0.556,
        'o' => 0.556,
        'p' => 0.556,
        'q' => 0.556,
        'r' => 0.333,
        's' => 0.5,
        't' => 0.278,
        'u' => 0.556,
        'v' => 0.5,
        'w' => 0.722,
        'x' => 0.5,
        'y' => 0.5,
        'z' => 0.5,
        '{' | '}' => 0.334,
        '|' => 0.26,
        // Emoji: render wider than a nominal em advance.
        c if is_emoji(c) => 1.2,
        // CJK / full-width ideographs and kana.
        c if (c as u32) >= 0x2E80 => 1.0,
        // Other Latin-1 / accented letters: ~base letter width.
        _ => 0.556,
    }
}

fn is_emoji(c: char) -> bool {
    let u = c as u32;
    (0x1F000..=0x1FAFF).contains(&u)
        || (0x2600..=0x27BF).contains(&u)
        || (0x1F300..=0x1F9FF).contains(&u)
}

fn ref_advance(s: &str, font_size: f32) -> f32 {
    s.chars().map(ref_em).sum::<f32>() * font_size
}

const BATTERY: &[&str] = &[
    "OK",
    "Cancel",
    "WWWWW",
    "MMMMM",
    "AWESOME GATEWAY",
    "Order Service",
    "PostgreSQL primary",
    "QUOTAS",
    "GROWTH",
    "Load Balancer",
    "API",
    "VPC",
    "WOW WHO",
    "中文标签",     // CJK
    "データベース", // Katakana
    "用户认证服务", // CJK phrase
    "Deploy 🚀",    // emoji + latin
    "🔥🔥🔥",       // emoji run
    "Cache ⚡ Layer",
    "OOOObviously WIDE",
];

#[test]
fn measured_width_is_upper_bound_for_battery() {
    let fs = 14.0;
    for &s in BATTERY {
        let measured = measure_line(s, fs);
        let rendered = ref_advance(s, fs);
        assert!(
            measured + 0.01 >= rendered,
            "label {s:?} under-measured: measured {measured:.2} < rendered {rendered:.2}"
        );
    }
}

#[test]
fn every_ascii_glyph_is_upper_bound() {
    let fs = 100.0; // big so rounding noise is negligible
    for code in 0x20u8..0x7F {
        let c = code as char;
        let measured = measure_line(&c.to_string(), fs);
        let rendered = ref_em(c) * fs;
        assert!(
            measured + 0.05 >= rendered,
            "glyph {c:?} under-measured: {measured:.2} < {rendered:.2}"
        );
    }
}

#[test]
fn cjk_and_emoji_are_upper_bound() {
    let fs = 14.0;
    for s in ["世界", "あ", "漢字テスト", "🚀", "😀😀", "🇺🇸"] {
        let measured = measure_line(s, fs);
        let rendered = ref_advance(s, fs);
        assert!(
            measured + 0.01 >= rendered,
            "wide label {s:?} under-measured: {measured:.2} < {rendered:.2}"
        );
    }
}
