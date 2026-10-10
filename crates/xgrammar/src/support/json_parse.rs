//! Depth-guarded JSON parsing.

use serde::Deserialize;
use serde_json::Value;

use super::recursion_guard::{RecursionError, get_max_recursion_depth};

/// Parses `text` as JSON, rejecting input whose value nesting exceeds the maximum recursion
/// depth (the C++ `ParseJSON`, which guards every nested array or object item). The depth is
/// checked before parsing, so `serde_json`'s fixed 128-level limit is lifted.
///
/// # Errors
/// Returns the parse error message, or the recursion-depth error message.
pub fn parse_json(text: &str) -> Result<Value, String> {
    let max = get_max_recursion_depth();
    let mut depth = 0;
    let mut in_string = false;
    let mut escaped = false;
    for byte in text.bytes() {
        if in_string {
            match (escaped, byte) {
                (true, _) => escaped = false,
                (false, b'\\') => escaped = true,
                (false, b'"') => in_string = false,
                _ => {},
            }
            continue;
        }
        match byte {
            b'"' => in_string = true,
            b'[' | b'{' => depth += 1,
            b']' | b'}' => depth -= 1,
            b' ' | b'\t' | b'\n' | b'\r' | b',' | b':' => continue,
            _ => {},
        }
        // A value (or container item) at nesting `depth` is parsed at recursion depth `depth`.
        let value_depth = if matches!(byte, b'[' | b'{') {
            depth - 1
        } else {
            depth
        };
        if value_depth > max {
            return Err(RecursionError::DepthExceeded {
                current: value_depth,
                max,
            }
            .to_string());
        }
    }
    let mut deserializer = serde_json::Deserializer::from_str(text);
    deserializer.disable_recursion_limit();
    let value = Value::deserialize(&mut deserializer).map_err(|error| error.to_string())?;
    deserializer.end().map_err(|error| error.to_string())?;
    Ok(value)
}
