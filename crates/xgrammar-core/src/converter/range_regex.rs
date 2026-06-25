//! Generates regexes matching integer / floating-point ranges — a port of the
//! `GenerateRangeRegex` / `GenerateFloatRangeRegex` family in `cpp/json_schema_converter.cc`.
//!
//! These back the `minimum`/`maximum` constraints of JSON-schema number types.

use std::fmt::Write as _;

/// Default fractional precision for float range regexes (matches the converter).
const FLOAT_PRECISION: i32 = 6;

/// Builds a regex fragment matching a single digit position in `[start, end]`, optionally
/// followed by `remaining_digits` free digits. `start`/`end` are ASCII digit bytes.
fn make_pattern_for_digit_range(
    start: u8,
    end: u8,
    remaining_digits: i32,
) -> String {
    let mut out = String::new();
    if start == end {
        out.push(start as char);
    } else {
        out.push('[');
        out.push(start as char);
        out.push('-');
        out.push(end as char);
        out.push(']');
    }
    if remaining_digits > 0 {
        let _ = write!(out, "\\d{{{remaining_digits}}}");
    }
    out
}

/// Returns regex alternatives covering the positive integer range `[lower, upper]`.
fn generate_number_patterns(
    lower: i64,
    upper: i64,
) -> Vec<String> {
    let mut patterns = Vec::new();
    let lower_len = lower.to_string().len() as i32;
    let upper_len = upper.to_string().len() as i32;

    for len in lower_len..=upper_len {
        let digit_min = 10i64.pow((len - 1) as u32);
        let digit_max = 10i64.pow(len as u32) - 1;
        let start = if len == lower_len {
            lower
        } else {
            digit_min
        };
        let end = if len == upper_len {
            upper
        } else {
            digit_max
        };
        let start_str = start.to_string();
        let end_str = end.to_string();
        let sb = start_str.as_bytes();
        let eb = end_str.as_bytes();
        let len_us = len as usize;

        if len == 1 {
            patterns.push(make_pattern_for_digit_range(sb[0], eb[0], 0));
            continue;
        }

        let mut prefix = 0usize;
        while prefix < len_us && sb[prefix] == eb[prefix] {
            prefix += 1;
        }

        if prefix == len_us {
            patterns.push(start_str.clone());
            continue;
        }

        if prefix > 0 && prefix >= len_us - 2 {
            let common = &start_str[0..prefix];
            patterns.push(format!(
                "{common}{}",
                make_pattern_for_digit_range(
                    sb[prefix],
                    eb[prefix],
                    len - prefix as i32 - 1
                )
            ));
            continue;
        }

        if len == lower_len && len == upper_len {
            if start == digit_max {
                patterns.push(start_str.clone());
            } else if start == digit_min {
                if end == digit_max {
                    patterns.push(format!("[1-9]\\d{{{}}}", len - 1));
                } else {
                    for i in 0..eb.len() {
                        if i == 0 {
                            if eb[0] > b'1' {
                                patterns.push(make_pattern_for_digit_range(
                                    b'1',
                                    eb[0] - 1,
                                    len - 1,
                                ));
                            }
                        } else if eb[i] > b'0' {
                            let pref = &end_str[0..i];
                            patterns.push(format!(
                                "{pref}{}",
                                make_pattern_for_digit_range(
                                    b'0',
                                    eb[i] - 1,
                                    len - i as i32 - 1
                                )
                            ));
                        }
                    }
                    patterns.push(end_str.clone());
                }
            } else if end == digit_max {
                for i in 0..sb.len() {
                    if i == 0 {
                        if sb[0] < b'9' {
                            patterns.push(make_pattern_for_digit_range(
                                sb[0] + 1,
                                b'9',
                                len - 1,
                            ));
                        }
                    } else if sb[i] < b'9' {
                        let pref = &start_str[0..i];
                        patterns.push(format!(
                            "{pref}{}",
                            make_pattern_for_digit_range(
                                sb[i] + 1,
                                b'9',
                                len - i as i32 - 1
                            )
                        ));
                    }
                }
                patterns.push(start_str.clone());
            } else {
                let start_first = sb[0];
                let end_first = eb[0];
                if end_first as i32 - start_first as i32 > 1 {
                    patterns.push(make_pattern_for_digit_range(
                        start_first + 1,
                        end_first - 1,
                        len - 1,
                    ));
                }
                for i in 0..sb.len() {
                    if i == 0 {
                        let pref = &start_str[0..1];
                        if sb[1] < b'9' {
                            patterns.push(format!(
                                "{pref}{}",
                                make_pattern_for_digit_range(
                                    sb[1] + 1,
                                    b'9',
                                    len - 2
                                )
                            ));
                        }
                    } else if sb[i] < b'9' {
                        let pref = &start_str[0..i];
                        patterns.push(format!(
                            "{pref}{}",
                            make_pattern_for_digit_range(
                                sb[i] + 1,
                                b'9',
                                len - i as i32 - 1
                            )
                        ));
                    }
                }
                patterns.push(start_str.clone());
                for i in 0..eb.len() {
                    if i == 0 {
                        let pref = &end_str[0..1];
                        if eb[1] > b'0' {
                            patterns.push(format!(
                                "{pref}{}",
                                make_pattern_for_digit_range(
                                    b'0',
                                    eb[1] - 1,
                                    len - 2
                                )
                            ));
                        }
                    } else if eb[i] > b'0' {
                        let pref = &end_str[0..i];
                        patterns.push(format!(
                            "{pref}{}",
                            make_pattern_for_digit_range(
                                b'0',
                                eb[i] - 1,
                                len - i as i32 - 1
                            )
                        ));
                    }
                }
                patterns.push(end_str.clone());
            }
        } else if len == lower_len {
            if start == digit_min {
                patterns.push(format!("[1-9]\\d{{{}}}", len - 1));
            } else {
                for i in 0..sb.len() {
                    if i == 0 {
                        if sb[0] < b'9' {
                            patterns.push(make_pattern_for_digit_range(
                                sb[0] + 1,
                                b'9',
                                len - 1,
                            ));
                        }
                    } else if sb[i] < b'9' {
                        let pref = &start_str[0..i];
                        patterns.push(format!(
                            "{pref}{}",
                            make_pattern_for_digit_range(
                                sb[i] + 1,
                                b'9',
                                len - i as i32 - 1
                            )
                        ));
                    }
                }
                patterns.push(start_str.clone());
            }
        } else if len == upper_len {
            if end == digit_max {
                patterns.push(format!("[1-9]\\d{{{}}}", len - 1));
            } else {
                for i in 0..eb.len() {
                    if i == 0 {
                        if eb[0] > b'1' {
                            patterns.push(make_pattern_for_digit_range(
                                b'1',
                                eb[0] - 1,
                                len - 1,
                            ));
                        }
                    } else if eb[i] > b'0' {
                        let pref = &end_str[0..i];
                        patterns.push(format!(
                            "{pref}{}",
                            make_pattern_for_digit_range(
                                b'0',
                                eb[i] - 1,
                                len - i as i32 - 1
                            )
                        ));
                    }
                }
                patterns.push(end_str.clone());
            }
        } else {
            patterns.push(format!("[1-9]\\d{{{}}}", len - 1));
        }
    }

    patterns
}

fn generate_sub_range_regex(
    lower: i64,
    upper: i64,
) -> String {
    let patterns = generate_number_patterns(lower, upper);
    format!("({})", patterns.join("|"))
}

/// Generates a regex matching integers in `[start, end]` (either bound optional).
#[must_use]
pub fn generate_range_regex(
    start: Option<i64>,
    end: Option<i64>,
) -> String {
    let mut parts: Vec<String> = Vec::new();

    if start.is_none() && end.is_none() {
        return "^-?\\d+$".to_owned();
    }

    if let (Some(start), None) = (start, end) {
        if start <= 0 {
            if start < 0 {
                parts.push(format!("-{}", generate_sub_range_regex(start, 1)));
            }
            parts.push("0".to_owned());
            parts.push("[1-9]\\d*".to_owned());
        } else {
            let start_str = start.to_string();
            let sb = start_str.as_bytes();
            let len = start_str.len() as i32;
            if len == 1 {
                parts.push(make_pattern_for_digit_range(sb[0], b'9', 0));
                parts.push("[1-9]\\d*".to_owned());
            } else {
                parts.push(start_str.clone());
                for i in 0..sb.len() {
                    if i == 0 {
                        if sb[0] < b'9' {
                            parts.push(make_pattern_for_digit_range(
                                sb[0] + 1,
                                b'9',
                                len - 1,
                            ));
                        }
                    } else if sb[i] < b'9' {
                        let pref = &start_str[0..i];
                        parts.push(format!(
                            "{pref}{}",
                            make_pattern_for_digit_range(
                                sb[i] + 1,
                                b'9',
                                len - i as i32 - 1
                            )
                        ));
                    }
                }
                parts.push(format!("[1-9]\\d{{{len},}}"));
            }
        }
    }

    if let (None, Some(end)) = (start, end) {
        if end >= 0 {
            parts.push("-[1-9]\\d*".to_owned());
            parts.push("0".to_owned());
            if end > 0 {
                parts.push(generate_sub_range_regex(1, end));
            }
        } else {
            let end_str = (-end).to_string();
            let eb = end_str.as_bytes();
            let len = end_str.len() as i32;
            if len == 1 {
                parts.push(format!(
                    "-{}",
                    make_pattern_for_digit_range(eb[0], b'9', 0)
                ));
                parts.push("-[1-9]\\d*".to_owned());
            } else {
                parts.push(end.to_string());
                for i in 0..eb.len() {
                    if i == 0 {
                        if eb[0] > b'1' {
                            parts.push(format!(
                                "-{}",
                                make_pattern_for_digit_range(
                                    b'1',
                                    eb[0] - 1,
                                    len - 1
                                )
                            ));
                        }
                    } else if eb[i] > b'0' {
                        let pref = &end_str[0..i];
                        parts.push(format!(
                            "-{pref}{}",
                            make_pattern_for_digit_range(
                                b'0',
                                eb[i] - 1,
                                len - i as i32 - 1
                            )
                        ));
                    }
                }
                parts.push(format!("-[1-9]\\d{{{len},}}"));
            }
        }
    }

    if let (Some(range_start), Some(range_end)) = (start, end) {
        if range_start > range_end {
            return "^()$".to_owned();
        }
        if range_start < 0 {
            let neg_end = range_end.min(-1);
            parts.push(format!(
                "-{}",
                generate_sub_range_regex(-neg_end, -range_start)
            ));
        }
        if range_start <= 0 && range_end >= 0 {
            parts.push("0".to_owned());
        }
        if range_end > 0 {
            let pos_start = range_start.max(1);
            parts.push(generate_sub_range_regex(pos_start, range_end));
        }
    }

    format!("^({})$", parts.join("|"))
}

fn escape_dot_for_regex(s: &str) -> String {
    s.replace('.', "\\.")
}

fn format_float(
    value: f64,
    precision: i32,
) -> String {
    if value == (value as i64) as f64 {
        return (value as i64).to_string();
    }
    let mut result = format!("{:.*}", precision as usize, value);
    if let Some(dot) = result.find('.') {
        match result.rfind(|c| c != '0') {
            Some(idx) if idx > dot => result.truncate(idx + 1),
            Some(idx) if idx == dot => result.truncate(dot),
            _ => {},
        }
    }
    result
}

/// Generates a regex matching floating-point numbers in `[start, end]` (either bound
/// optional) with up to six fractional digits.
#[must_use]
pub fn generate_float_range_regex(
    start: Option<f64>,
    end: Option<f64>,
) -> String {
    let precision = FLOAT_PRECISION;
    if let (Some(s), Some(e)) = (start, end) {
        if s > e {
            return "^()$".to_owned();
        }
    }
    if start.is_none() && end.is_none() {
        return format!("^-?\\d+(\\.\\d{{1,{precision}}})?$");
    }

    let mut parts: Vec<String> = Vec::new();
    let start_int = start.map_or(0, |v| v.floor() as i64);
    let start_frac = start.map_or(0.0, |v| v - start_int as f64);
    let is_start_negative = start.is_some_and(|v| v < 0.0);
    let end_int = end.map_or(0, |v| v.floor() as i64);
    let end_frac = end.map_or(0.0, |v| v - end_int as f64);
    let is_end_negative = end.is_some_and(|v| v < 0.0);

    // Emits the fractional "fan-out" patterns for a formatted number string.
    let frac_patterns = |num_str: &str, negative: bool| -> Vec<String> {
        let mut out = Vec::new();
        let Some(dot) = num_str.find('.') else {
            return out;
        };
        let int_part = &num_str[0..dot];
        let frac_part = &num_str.as_bytes()[dot + 1..];
        for i in 0..frac_part.len() {
            let rem = precision - i as i32 - 1;
            if i == 0 {
                if negative {
                    for d in b'0'..frac_part[0] {
                        out.push(format!(
                            "{int_part}\\.{}\\d{{0,{}}}",
                            d as char,
                            precision - 1
                        ));
                    }
                } else {
                    for d in (frac_part[0] + 1)..=b'9' {
                        out.push(format!(
                            "{int_part}\\.{}\\d{{0,{}}}",
                            d as char,
                            precision - 1
                        ));
                    }
                }
            } else {
                let pref = std::str::from_utf8(&frac_part[0..i]).unwrap();
                if negative {
                    if frac_part[i] > b'0' {
                        for d in b'0'..frac_part[i] {
                            out.push(format!(
                                "{int_part}\\.{pref}{}\\d{{0,{rem}}}",
                                d as char
                            ));
                        }
                    }
                } else {
                    for d in (frac_part[i] + 1)..=b'9' {
                        out.push(format!(
                            "{int_part}\\.{pref}{}\\d{{0,{rem}}}",
                            d as char
                        ));
                    }
                }
            }
        }
        out
    };

    if let (Some(start), None) = (start, end) {
        let start_str = format_float(start, precision);
        parts.push(escape_dot_for_regex(&start_str));
        if start_frac > 0.0 {
            parts.extend(frac_patterns(&start_str, is_start_negative));
        }
        if start_int < i64::MAX - 1 {
            let inner = generate_range_regex(Some(start_int + 1), None);
            let inner = &inner[1..inner.len() - 1];
            parts.push(format!("{inner}(\\.\\d{{1,{precision}}})?"));
        }
    } else if let (None, Some(end)) = (start, end) {
        let end_str = format_float(end, precision);
        parts.push(escape_dot_for_regex(&end_str));
        if end_frac > 0.0 {
            parts.extend(frac_patterns(&end_str, !is_end_negative));
        }
        if end_int > i64::MIN + 1 {
            let inner = generate_range_regex(None, Some(end_int - 1));
            let inner = &inner[1..inner.len() - 1];
            parts.push(format!("{inner}(\\.\\d{{1,{precision}}})?"));
        }
    } else if let (Some(start), Some(end)) = (start, end) {
        if start_int == end_int {
            if start_frac == 0.0 && end_frac == 0.0 {
                parts.push(start_int.to_string());
            } else {
                let start_str = format_float(start, precision);
                parts.push(escape_dot_for_regex(&start_str));
                let end_str = format_float(end, precision);
                if start_str != end_str {
                    parts.push(escape_dot_for_regex(&end_str));
                }
            }
        } else {
            let start_str = format_float(start, precision);
            parts.push(escape_dot_for_regex(&start_str));
            let end_str = format_float(end, precision);
            if start_str != end_str {
                parts.push(escape_dot_for_regex(&end_str));
            }
            if end_int > start_int + 1 {
                let inner = generate_range_regex(
                    Some(start_int + 1),
                    Some(end_int - 1),
                );
                let inner = &inner[1..inner.len() - 1];
                parts.push(format!("{inner}(\\.\\d{{1,{precision}}})?"));
            }
            if start_frac > 0.0 {
                parts.extend(frac_patterns(&start_str, is_start_negative));
            } else {
                parts.push(format!("{start_int}\\.\\d{{1,{precision}}}"));
            }
            if end_frac > 0.0 {
                parts.extend(frac_patterns(&end_str, !is_end_negative));
            } else {
                parts.push(format!("{end_int}\\.\\d{{1,{precision}}}"));
            }
        }
    }

    format!("^({})$", parts.join("|"))
}
