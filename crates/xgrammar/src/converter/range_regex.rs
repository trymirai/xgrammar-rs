//! Generates regular expressions for JSON Schema integer and number ranges.
//!
//! This is a direct Rust port of upstream XGrammar's `NumberGenerator`.

use std::cmp::Ordering;

/// Default fractional precision used by the JSON Schema converter.
const FLOAT_PRECISION: usize = 6;

fn digit_class(
    lo: u8,
    hi: u8,
) -> String {
    if lo == hi {
        return char::from(lo).to_string();
    }
    if lo == b'0' && hi == b'9' {
        return r"\d".to_owned();
    }
    format!("[{}-{}]", char::from(lo), char::from(hi))
}

fn exact_digits(count: usize) -> String {
    match count {
        0 => String::new(),
        1 => r"\d".to_owned(),
        _ => format!(r"\d{{{count}}}"),
    }
}

fn all_char(
    value: &str,
    expected: u8,
) -> bool {
    value.bytes().all(|byte| byte == expected)
}

fn int_same_len(
    start: &str,
    end: &str,
) -> Vec<String> {
    debug_assert_eq!(start.len(), end.len());
    debug_assert!(start <= end);

    let len = start.len();
    if start == end {
        return vec![start.to_owned()];
    }
    if len == 1 {
        return vec![digit_class(start.as_bytes()[0], end.as_bytes()[0])];
    }

    let start_first = start.as_bytes()[0];
    let end_first = end.as_bytes()[0];
    if start_first == end_first {
        return int_same_len(&start[1..], &end[1..])
            .into_iter()
            .map(|pattern| format!("{}{pattern}", char::from(start_first)))
            .collect();
    }

    let start_suffix = &start[1..];
    let end_suffix = &end[1..];
    if all_char(start_suffix, b'0') && all_char(end_suffix, b'9') {
        if start_first == b'0' && end_first == b'9' {
            return vec![exact_digits(len)];
        }
        return vec![format!(
            "{}{}",
            digit_class(start_first, end_first),
            exact_digits(len - 1)
        )];
    }

    let nines = "9".repeat(len - 1);
    let zeros = "0".repeat(len - 1);
    let mut result = int_same_len(start_suffix, &nines)
        .into_iter()
        .map(|pattern| format!("{}{pattern}", char::from(start_first)))
        .collect::<Vec<_>>();
    if end_first - start_first >= 2 {
        result.push(format!(
            "{}{}",
            digit_class(start_first + 1, end_first - 1),
            exact_digits(len - 1)
        ));
    }
    result.extend(
        int_same_len(&zeros, end_suffix)
            .into_iter()
            .map(|pattern| format!("{}{pattern}", char::from(end_first))),
    );
    result
}

fn compare_digit_str(
    left: &str,
    right: &str,
) -> Ordering {
    left.len().cmp(&right.len()).then_with(|| left.cmp(right))
}

fn number_patterns_str(
    start: &str,
    end: &str,
) -> Vec<String> {
    if compare_digit_str(start, end).is_gt() {
        return Vec::new();
    }

    let mut patterns = Vec::new();
    for len in start.len()..=end.len() {
        let segment_start = if len == start.len() {
            start.to_owned()
        } else {
            format!("1{}", "0".repeat(len - 1))
        };
        let segment_end = if len == end.len() {
            end.to_owned()
        } else {
            "9".repeat(len)
        };
        patterns.extend(int_same_len(&segment_start, &segment_end));
    }
    patterns
}

fn sub_range_regex_str(
    start: &str,
    end: &str,
) -> String {
    format!("({})", number_patterns_str(start, end).join("|"))
}

fn at_least_positive_patterns_str(start: &str) -> Vec<String> {
    let len = start.len();
    let mut result = int_same_len(start, &"9".repeat(len));
    result.push(format!(r"[1-9]\d{{{len},}}"));
    result
}

fn abs_digits(value: i64) -> String {
    value.to_string().trim_start_matches('-').to_owned()
}

/// Generates a regular expression matching all integers in `[start, end]`.
///
/// Either bound may be omitted. An empty range produces `^()$`.
#[must_use]
pub fn generate_range_regex(
    start: Option<i64>,
    end: Option<i64>,
) -> String {
    let mut parts = Vec::new();

    match (start, end) {
        (None, None) => return r"^-?\d+$".to_owned(),
        (Some(start), None) if start <= 0 => {
            if start < 0 {
                parts.push(format!(
                    "-{}",
                    sub_range_regex_str("1", &abs_digits(start))
                ));
            }
            parts.push("0".to_owned());
            parts.push(r"[1-9]\d*".to_owned());
        },
        (Some(start), None) => {
            parts.extend(at_least_positive_patterns_str(&start.to_string()));
        },
        (None, Some(end)) if end >= 0 => {
            parts.push(r"-[1-9]\d*".to_owned());
            parts.push("0".to_owned());
            if end > 0 {
                parts.push(sub_range_regex_str("1", &end.to_string()));
            }
        },
        (None, Some(end)) => {
            parts.extend(
                at_least_positive_patterns_str(&abs_digits(end))
                    .into_iter()
                    .map(|pattern| format!("-{pattern}")),
            );
        },
        (Some(start), Some(end)) => {
            if start > end {
                return "^()$".to_owned();
            }
            if start < 0 {
                let negative_end = end.min(-1);
                parts.push(format!(
                    "-{}",
                    sub_range_regex_str(
                        &abs_digits(negative_end),
                        &abs_digits(start)
                    )
                ));
            }
            if start <= 0 && end >= 0 {
                parts.push("0".to_owned());
            }
            if end > 0 {
                parts.push(sub_range_regex_str(
                    &start.max(1).to_string(),
                    &end.to_string(),
                ));
            }
        },
    }

    format!("^({})$", parts.join("|"))
}

fn format_float(
    value: f64,
    precision: usize,
) -> String {
    if (-9_223_372_036_854_775_808.0..9_223_372_036_854_775_808.0)
        .contains(&value)
        && value == (value as i64) as f64
    {
        return (value as i64).to_string();
    }

    let mut result = format!("{value:.precision$}");
    if let Some(decimal_position) = result.find('.') {
        match result.rfind(|character| character != '0') {
            Some(last_non_zero) if last_non_zero > decimal_position => {
                result.truncate(last_non_zero + 1);
            },
            Some(last_non_zero) if last_non_zero == decimal_position => {
                result.truncate(decimal_position);
            },
            _ => {},
        }
    }
    result
}

fn split_decimal(value: &str) -> (&str, &str) {
    value
        .split_once('.')
        .map_or((value, ""), |(integer, fraction)| (integer, fraction))
}

fn adjust_grid(
    value: &str,
    precision: usize,
    increment: bool,
) -> String {
    let (integer_part, fraction_part) = split_decimal(value);
    let mut number = format!(
        "{integer_part}{fraction_part}{}",
        "0".repeat(precision.saturating_sub(fraction_part.len()))
    )
    .into_bytes();

    if increment {
        let mut index = number.len();
        while index > 0 && number[index - 1] == b'9' {
            number[index - 1] = b'0';
            index -= 1;
        }
        if index == 0 {
            number.insert(0, b'1');
        } else {
            number[index - 1] += 1;
        }
    } else {
        let mut index = number.len();
        while index > 0 && number[index - 1] == b'0' {
            number[index - 1] = b'9';
            index -= 1;
        }
        if index == 0 {
            number.fill(b'0');
        } else {
            number[index - 1] -= 1;
        }
    }

    while number.len() <= precision {
        number.insert(0, b'0');
    }
    let split = number.len() - precision;
    let mut new_integer =
        String::from_utf8(number[..split].to_vec()).expect("digits are UTF-8");
    let mut new_fraction =
        String::from_utf8(number[split..].to_vec()).expect("digits are UTF-8");
    let first_non_zero = new_integer.find(|character| character != '0');
    new_integer = first_non_zero.map_or_else(
        || "0".to_owned(),
        |index| new_integer[index..].to_owned(),
    );
    new_fraction.truncate(new_fraction.trim_end_matches('0').len());
    if new_fraction.is_empty() {
        new_integer
    } else {
        format!("{new_integer}.{new_fraction}")
    }
}

fn round_bound_to_grid(
    value: f64,
    precision: usize,
    is_lower: bool,
    strict_in: bool,
) -> (String, bool) {
    let mut rounded = format_float(value, precision);
    let rounded_value = rounded.parse::<f64>().expect("formatted float parses");
    if rounded_value == value {
        return (rounded, strict_in);
    }

    if is_lower && rounded_value < value {
        rounded = adjust_grid(&rounded, precision, true);
    } else if !is_lower && rounded_value > value {
        rounded = adjust_grid(&rounded, precision, false);
    }
    (rounded, false)
}

fn free_digits(max_count: usize) -> String {
    if max_count == 0 {
        String::new()
    } else {
        format!(r"\d{{0,{max_count}}}")
    }
}

fn optional_zeros(max_count: usize) -> String {
    if max_count == 0 {
        String::new()
    } else {
        format!("0{{0,{max_count}}}")
    }
}

fn some_zeros(max_count: usize) -> String {
    format!("0{{1,{max_count}}}")
}

#[derive(Default)]
struct FractionPatterns {
    parts: Vec<String>,
    include_empty: bool,
}

fn fraction_greater_patterns(
    start: &str,
    strict: bool,
    max_len: usize,
) -> FractionPatterns {
    let mut result = FractionPatterns::default();
    let len = start.len();
    for (index, digit) in start.bytes().enumerate() {
        if digit < b'9' {
            result.parts.push(format!(
                "{}{}{}",
                &start[..index],
                digit_class(digit + 1, b'9'),
                free_digits(max_len - index - 1)
            ));
        }
    }
    for zero_count in 0.. {
        if len + zero_count + 1 > max_len {
            break;
        }
        result.parts.push(format!(
            "{start}{}[1-9]{}",
            "0".repeat(zero_count),
            free_digits(max_len - len - zero_count - 1)
        ));
    }
    if !strict {
        if len > 0 {
            result
                .parts
                .push(format!("{start}{}", optional_zeros(max_len - len)));
        } else {
            result.include_empty = true;
            if max_len >= 1 {
                result.parts.push(some_zeros(max_len));
            }
        }
    }
    result
}

fn fraction_less_patterns(
    end: &str,
    strict: bool,
    max_len: usize,
) -> FractionPatterns {
    let mut result = FractionPatterns::default();
    let len = end.len();
    for (index, digit) in end.bytes().enumerate() {
        if digit > b'0' {
            result.parts.push(format!(
                "{}{}{}",
                &end[..index],
                digit_class(b'0', digit - 1),
                free_digits(max_len - index - 1)
            ));
        }
    }
    for index in 0..len {
        if index == 0 {
            if max_len >= 1 {
                result.parts.push(some_zeros(max_len));
            }
        } else {
            result.parts.push(format!(
                "{}{}",
                &end[..index],
                optional_zeros(max_len - index)
            ));
        }
    }
    if !strict {
        if len > 0 {
            result
                .parts
                .push(format!("{end}{}", optional_zeros(max_len - len)));
        } else if max_len >= 1 {
            result.parts.push(some_zeros(max_len));
        }
    }
    result.include_empty = len > 0 || !strict;
    result
}

fn fraction_between_patterns(
    start: &str,
    strict_start: bool,
    end: &str,
    strict_end: bool,
    max_len: usize,
) -> FractionPatterns {
    let mut result = FractionPatterns::default();
    let mut common_len = 0;
    while common_len < end.len()
        && start.as_bytes().get(common_len).copied().unwrap_or(b'0')
            == end.as_bytes()[common_len]
    {
        common_len += 1;
    }
    let common = &end[..common_len];
    let start_digit = start.as_bytes().get(common_len).copied().unwrap_or(b'0');
    let end_digit = end.as_bytes()[common_len];

    if end_digit - start_digit >= 2 {
        result.parts.push(format!(
            "{common}{}{}",
            digit_class(start_digit + 1, end_digit - 1),
            free_digits(max_len - common_len - 1)
        ));
    }
    if common_len < start.len() {
        let lower = fraction_greater_patterns(
            &start[common_len + 1..],
            strict_start,
            max_len - common_len - 1,
        );
        result.parts.extend(
            lower.parts.into_iter().map(|part| {
                format!("{common}{}{part}", char::from(start_digit))
            }),
        );
        if lower.include_empty {
            result.parts.push(format!("{common}{}", char::from(start_digit)));
        }
    } else {
        let lower =
            fraction_greater_patterns("", true, max_len - common_len - 1);
        result.parts.extend(
            lower.parts.into_iter().map(|part| {
                format!("{common}{}{part}", char::from(start_digit))
            }),
        );
        if !strict_start {
            if !start.is_empty() {
                result.parts.push(format!(
                    "{start}{}",
                    optional_zeros(max_len - start.len())
                ));
            } else {
                result.include_empty = true;
                if max_len >= 1 {
                    result.parts.push(some_zeros(max_len));
                }
            }
        }
    }

    let upper = fraction_less_patterns(
        &end[common_len + 1..],
        strict_end,
        max_len - common_len - 1,
    );
    result.parts.extend(
        upper
            .parts
            .into_iter()
            .map(|part| format!("{common}{}{part}", char::from(end_digit))),
    );
    if upper.include_empty {
        result.parts.push(format!("{common}{}", char::from(end_digit)));
    }
    result
}

fn compare_decimal(
    left_integer: &str,
    left_fraction: &str,
    right_integer: &str,
    right_fraction: &str,
) -> Ordering {
    let integer_order = compare_digit_str(left_integer, right_integer);
    if !integer_order.is_eq() {
        return integer_order;
    }
    let max_fraction = left_fraction.len().max(right_fraction.len());
    for index in 0..max_fraction {
        let left = left_fraction.as_bytes().get(index).copied().unwrap_or(b'0');
        let right =
            right_fraction.as_bytes().get(index).copied().unwrap_or(b'0');
        if left != right {
            return left.cmp(&right);
        }
    }
    Ordering::Equal
}

fn strip_anchors(regex: &str) -> &str {
    &regex[1..regex.len() - 1]
}

fn parse_int_capped(digits: &str) -> i64 {
    digits.parse::<i64>().unwrap_or(i64::MAX)
}

fn add_with_integer_part(
    parts: &mut Vec<String>,
    integer: &str,
    fraction_set: FractionPatterns,
) {
    parts.extend(
        fraction_set
            .parts
            .into_iter()
            .map(|fraction| format!(r"{integer}\.{fraction}")),
    );
    if fraction_set.include_empty {
        parts.push(integer.to_owned());
    }
}

fn positive_range_parts(
    low: &str,
    mut strict_low: bool,
    high: Option<&str>,
    strict_high: bool,
    precision: usize,
) -> Vec<String> {
    let mut parts = Vec::new();
    let (integer_low, fraction_low) = split_decimal(low);
    if integer_low == "0" && fraction_low.is_empty() {
        strict_low = true;
    }
    let integer_low_value = parse_int_capped(integer_low);
    let optional_any_fraction = format!(r"(\.\d{{1,{precision}}})?");

    let Some(high) = high else {
        add_with_integer_part(
            &mut parts,
            integer_low,
            fraction_greater_patterns(fraction_low, strict_low, precision),
        );
        if integer_low_value < i64::MAX {
            parts.push(format!(
                "{}{}",
                strip_anchors(&generate_range_regex(
                    Some(integer_low_value + 1),
                    None
                )),
                optional_any_fraction
            ));
        }
        return parts;
    };

    let (integer_high, fraction_high) = split_decimal(high);
    let integer_high_value = parse_int_capped(integer_high);
    let comparison =
        compare_decimal(integer_low, fraction_low, integer_high, fraction_high);
    if comparison.is_gt() || (comparison.is_eq() && (strict_low || strict_high))
    {
        return parts;
    }
    if comparison.is_eq() {
        if fraction_low.is_empty() {
            parts.push(format!(r"{integer_low}(\.{})?", some_zeros(precision)));
        } else {
            parts.push(format!(
                r"{integer_low}\.{fraction_low}{}",
                optional_zeros(precision - fraction_low.len())
            ));
        }
        return parts;
    }
    if integer_low == integer_high {
        add_with_integer_part(
            &mut parts,
            integer_low,
            fraction_between_patterns(
                fraction_low,
                strict_low,
                fraction_high,
                strict_high,
                precision,
            ),
        );
    } else {
        add_with_integer_part(
            &mut parts,
            integer_low,
            fraction_greater_patterns(fraction_low, strict_low, precision),
        );
        if integer_high_value - integer_low_value >= 2 {
            parts.push(format!(
                "{}{}",
                strip_anchors(&generate_range_regex(
                    Some(integer_low_value + 1),
                    Some(integer_high_value - 1)
                )),
                optional_any_fraction
            ));
        }
        add_with_integer_part(
            &mut parts,
            integer_high,
            fraction_less_patterns(fraction_high, strict_high, precision),
        );
    }
    parts
}

/// Generates a regular expression matching numbers in `[start, end]` with up to six
/// fractional digits. The boolean flags make either bound exclusive.
#[must_use]
pub fn generate_float_range_regex_with_options(
    start: Option<f64>,
    end: Option<f64>,
    exclusive_start: bool,
    exclusive_end: bool,
) -> String {
    let precision = FLOAT_PRECISION;
    if let (Some(start), Some(end)) = (start, end) {
        if start > end || (start == end && (exclusive_start || exclusive_end)) {
            return "^()$".to_owned();
        }
    }
    if start.is_none() && end.is_none() {
        return format!(r"^-?\d+(\.\d{{1,{precision}}})?$");
    }

    let mut parts = Vec::new();
    let negatives_in_range = start.is_none_or(|start| start < 0.0);
    if negatives_in_range {
        let (low, strict_low) = match end {
            Some(end) if end < 0.0 => {
                round_bound_to_grid(-end, precision, true, exclusive_end)
            },
            _ => ("0".to_owned(), true),
        };
        let (high, strict_high) = start.map_or((None, false), |start| {
            let (high, strict) =
                round_bound_to_grid(-start, precision, false, exclusive_start);
            (Some(high), strict)
        });
        parts.extend(
            positive_range_parts(
                &low,
                strict_low,
                high.as_deref(),
                strict_high,
                precision,
            )
            .into_iter()
            .map(|part| format!("-{part}")),
        );
    }

    let zero_allowed = start
        .is_none_or(|start| start < 0.0 || (start == 0.0 && !exclusive_start))
        && end.is_none_or(|end| end > 0.0 || (end == 0.0 && !exclusive_end));
    if zero_allowed {
        parts.push(format!(r"0(\.{})?", some_zeros(precision)));
        if negatives_in_range {
            parts.push(format!(r"-0(\.{})", some_zeros(precision)));
        }
    }

    if end.is_none_or(|end| end > 0.0) {
        let (low, strict_low) = match start {
            Some(start) if start > 0.0 => {
                round_bound_to_grid(start, precision, true, exclusive_start)
            },
            _ => ("0".to_owned(), true),
        };
        let (high, strict_high) = end.map_or((None, false), |end| {
            let (high, strict) =
                round_bound_to_grid(end, precision, false, exclusive_end);
            (Some(high), strict)
        });
        parts.extend(positive_range_parts(
            &low,
            strict_low,
            high.as_deref(),
            strict_high,
            precision,
        ));
    }

    format!("^({})$", parts.join("|"))
}

/// Generates a regular expression matching numbers in the inclusive range `[start, end]`.
#[must_use]
pub fn generate_float_range_regex(
    start: Option<f64>,
    end: Option<f64>,
) -> String {
    generate_float_range_regex_with_options(start, end, false, false)
}
