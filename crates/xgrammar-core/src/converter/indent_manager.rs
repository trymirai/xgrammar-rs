//! Manages indentation and separators while emitting JSON-shaped EBNF — a port of
//! `IndentManager` in `cpp/json_schema_converter.*`.
//!
//! Depending on the whitespace mode it emits either flexible whitespace char-classes
//! (`any_whitespace`), fixed separators, or newline-plus-indent strings.

/// Produces the separator/whitespace fragments for array and object generation.
#[derive(Debug, Clone)]
pub(crate) struct IndentManager {
    any_whitespace: bool,
    enable_newline: bool,
    indent: i32,
    separator: String,
    total_indent: i32,
    is_first: Vec<bool>,
    max_whitespace_cnt: Option<i32>,
}

impl IndentManager {
    /// Creates an indent manager. `indent` enables newline+indent output; `separator` is
    /// the item separator (e.g. `,` or `, `).
    ///
    /// # Panics
    /// Panics if `max_whitespace_cnt` is `Some(n)` with `n <= 0`.
    pub fn new(
        indent: Option<i32>,
        separator: &str,
        any_whitespace: bool,
        max_whitespace_cnt: Option<i32>,
    ) -> Self {
        if let Some(n) = max_whitespace_cnt {
            assert!(n > 0, "max_whitespace_cnt must be positive.");
        }
        Self {
            any_whitespace,
            enable_newline: indent.is_some(),
            indent: indent.unwrap_or(0),
            separator: separator.to_owned(),
            total_indent: 0,
            is_first: vec![true],
            max_whitespace_cnt,
        }
    }

    fn whitespace_part(&self) -> String {
        match self.max_whitespace_cnt {
            None => "[ \\n\\t]*".to_owned(),
            Some(n) => format!("[ \\n\\t]{{0,{n}}}"),
        }
    }

    pub fn start_indent(&mut self) {
        self.total_indent += self.indent;
        self.is_first.push(true);
    }

    pub fn end_indent(&mut self) {
        self.total_indent -= self.indent;
        self.is_first.pop();
    }

    pub fn start_separator(&self) -> String {
        if self.any_whitespace {
            return self.whitespace_part();
        }
        if !self.enable_newline {
            return "\"\"".to_owned();
        }
        format!("\"\\n{}\"", " ".repeat(self.total_indent.max(0) as usize))
    }

    pub fn middle_separator(&self) -> String {
        if self.any_whitespace {
            let ws = self.whitespace_part();
            return format!("{ws} \"{}\" {ws}", self.separator);
        }
        if !self.enable_newline {
            return format!("\"{}\"", self.separator);
        }
        format!(
            "\"{}\\n{}\"",
            self.separator,
            " ".repeat(self.total_indent.max(0) as usize)
        )
    }

    pub fn end_separator(&self) -> String {
        if self.any_whitespace {
            return self.whitespace_part();
        }
        if !self.enable_newline {
            return "\"\"".to_owned();
        }
        format!(
            "\"\\n{}\"",
            " ".repeat((self.total_indent - self.indent).max(0) as usize)
        )
    }

    pub fn empty_separator(&self) -> String {
        if self.any_whitespace {
            return self.whitespace_part();
        }
        "\"\"".to_owned()
    }

    pub fn next_separator(
        &mut self,
        is_end: bool,
    ) -> String {
        if self.any_whitespace {
            let was_first = *self.is_first.last().unwrap();
            if was_first || is_end {
                *self.is_first.last_mut().unwrap() = false;
                return self.whitespace_part();
            }
            let ws = self.whitespace_part();
            return format!("{ws} \"{}\" {ws}", self.separator);
        }

        let mut res = String::new();
        if !*self.is_first.last().unwrap() && !is_end {
            res.push_str(&self.separator);
        }
        *self.is_first.last_mut().unwrap() = false;

        if self.enable_newline {
            res.push_str("\\n");
        }

        let pad = if is_end {
            self.total_indent - self.indent
        } else {
            self.total_indent
        };
        res.push_str(&" ".repeat(pad.max(0) as usize));

        format!("\"{res}\"")
    }
}
