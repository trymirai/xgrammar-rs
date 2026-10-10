//! Error raised by the Lark converter.

use super::lark_location::LarkLocation;

/// A Lark conversion failure.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LarkError {
    /// An error at a location in a Lark source, rendered with the offending line and a caret.
    #[error("Lark error at line {line}, column {column}: {message}{}", render_excerpt(.source_line, *.column))]
    Located {
        /// 1-based line number.
        line: i32,
        /// 1-based byte column.
        column: i32,
        /// Human-readable description.
        message: String,
        /// The source line containing the error.
        source_line: String,
    },
    /// An invalid named-grammar argument.
    #[error("{0}")]
    NamedGrammar(String),
}

impl LarkError {
    /// Builds a located error, capturing the source line the location points into.
    #[must_use]
    pub fn at(
        source: &str,
        location: LarkLocation,
        message: impl Into<String>,
    ) -> Self {
        let bytes = source.as_bytes();
        let mut line_start = 0;
        let mut current_line = 1;
        while current_line < location.line && line_start < bytes.len() {
            match bytes[line_start..].iter().position(|&b| b == b'\n') {
                Some(offset) => {
                    line_start += offset + 1;
                    current_line += 1;
                },
                None => {
                    line_start = bytes.len();
                    break;
                },
            }
        }
        let line_end = bytes[line_start..].iter().position(|&b| b == b'\n').map_or(bytes.len(), |end| line_start + end);
        Self::Located {
            line: location.line,
            column: location.column,
            message: message.into(),
            source_line: String::from_utf8_lossy(&bytes[line_start..line_end]).into_owned(),
        }
    }
}

fn render_excerpt(
    source_line: &str,
    column: i32,
) -> String {
    if source_line.is_empty() {
        return String::new();
    }
    format!("\n{source_line}\n{}^", " ".repeat(column.saturating_sub(1).max(0) as usize))
}
