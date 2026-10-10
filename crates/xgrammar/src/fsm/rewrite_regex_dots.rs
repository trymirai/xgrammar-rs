//! Standard regex dot semantics on top of the regex engine, whose `.` matches every codepoint.

/// Rewrites every unescaped `.` outside character classes to `[^\n]` unless
/// `dot_matches_newline` is true (the dot-all `s` flag).
#[must_use]
pub fn rewrite_regex_dots(
    pattern: &[u8],
    dot_matches_newline: bool,
) -> Vec<u8> {
    if dot_matches_newline {
        return pattern.to_vec();
    }
    let mut result = Vec::with_capacity(pattern.len());
    let mut escaped = false;
    let mut in_character_class = false;
    for &c in pattern {
        if escaped {
            result.push(c);
            escaped = false;
            continue;
        }
        match c {
            b'\\' => {
                result.push(c);
                escaped = true;
            },
            b'[' => {
                result.push(c);
                in_character_class = true;
            },
            b']' if in_character_class => {
                result.push(c);
                in_character_class = false;
            },
            b'.' if !in_character_class => result.extend_from_slice(b"[^\\n]"),
            _ => result.push(c),
        }
    }
    result
}
