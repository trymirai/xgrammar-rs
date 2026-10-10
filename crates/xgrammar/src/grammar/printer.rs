//! EBNF printing for [`Grammar`].

use std::fmt::{self, Write as _};

use super::{grammar::Grammar, grammar_expr_type::GrammarExprType, rule::NO_EXPR};
use crate::support::{Codepoint, escape_bytes, escape_codepoint};

/// Custom escapes used inside character classes: `-` and `]` are backslash-escaped.
const CHAR_CLASS_ESCAPES: &[(Codepoint, &str)] = &[(0x2D, "\\-"), (0x5D, "\\]")];

impl Grammar {
    /// Renders the grammar back to its normalized EBNF text (one rule per line).
    #[must_use]
    pub fn to_string_ebnf(&self) -> String {
        let mut out = String::new();
        for rule_id in 0..self.num_rules() {
            out.push_str(&self.print_rule(rule_id));
            out.push('\n');
        }
        out
    }

    fn print_rule(
        &self,
        rule_id: i32,
    ) -> String {
        let rule = self.rule(rule_id);
        let suffix_stop_info = self.suffix_stop_info(rule_id);
        // Print the attributes as one comma-separated bracket group, re-parseable by the lexer.
        let mut attributes = Vec::new();
        if rule.max_tokens >= 0 {
            attributes.push(format!("max_tokens={}", rule.max_tokens));
        }
        if rule.max_chars >= 0 {
            attributes.push(format!("max_chars={}", rule.max_chars));
        }
        if !rule.capture_name.is_empty() {
            attributes.push(format!("capture=\"{}\"", rule.capture_name));
        }
        if let Some(info) = suffix_stop_info {
            if info.hidden_suffix_bytes > 0 {
                attributes.push(format!("capture_hidden_suffix_bytes={}", info.hidden_suffix_bytes));
            }
            if info.hidden_stop_bytes > 0 {
                attributes.push(format!("capture_hidden_stop_bytes={}", info.hidden_stop_bytes));
            }
            if info.body_rule_id >= 0 {
                attributes.push(format!("capture_hidden_body_rule_id={}", info.body_rule_id));
                attributes.push(format!("capture_hidden_marker_rule_id={}", info.marker_rule_id));
            }
            if !info.stop_capture_name.is_empty() {
                attributes.push(format!("stop_capture=\"{}\"", info.stop_capture_name));
            }
        }
        if rule.is_lazy {
            attributes.push("lazy".to_owned());
        }
        if let Some(temperature) = rule.temperature {
            attributes.push(format!("temperature={}", format_float_max_digits(temperature)));
        }
        let mut res = rule.name.clone();
        if suffix_stop_info.is_some() || !attributes.is_empty() {
            let _ = write!(res, "[{}]", attributes.join(", "));
        }
        let _ = write!(res, " ::= {}", self.print_expr(rule.body_expr_id));
        if rule.lookahead_assertion_id != NO_EXPR {
            let _ = write!(res, " (={})", self.print_expr(rule.lookahead_assertion_id));
        }
        res
    }

    fn print_expr(
        &self,
        expr_id: i32,
    ) -> String {
        let expr = self.expr(expr_id);
        match expr.ty {
            GrammarExprType::ByteString => {
                format!("\"{}\"", escape_bytes(&expr.byte_string()))
            },
            GrammarExprType::CharacterClass => self.print_character_class(expr_id, false),
            GrammarExprType::CharacterClassStar => self.print_character_class(expr_id, true),
            GrammarExprType::EmptyStr => "\"\"".to_owned(),
            GrammarExprType::RuleRef => self.rule(expr.rule_ref_id()).name.clone(),
            GrammarExprType::Sequence => self.print_joined(expr.data, " "),
            GrammarExprType::Choices => self.print_joined(expr.data, " | "),
            GrammarExprType::TagDispatch => self.print_tag_dispatch(expr_id),
            GrammarExprType::Repeat => {
                let (rule_id, lower, upper) = expr.repeat();
                format!("{}{{{lower}, {upper}}}", self.rule(rule_id).name)
            },
            GrammarExprType::Token => Self::print_token_list("Token", expr.data),
            GrammarExprType::ExcludeToken => Self::print_token_list("ExcludeToken", expr.data),
            GrammarExprType::TokenTagDispatch => self.print_token_tag_dispatch(expr_id),
            GrammarExprType::Regex => {
                let json_string = if expr.regex_is_json_string() {
                    ", json_string=true"
                } else {
                    ""
                };
                format!("Regex({}{json_string})", Self::print_quoted(&expr.regex_pattern()))
            },
            GrammarExprType::Substring => {
                let chunks: Vec<String> =
                    expr.substring_chunks().iter().map(|chunk| Self::print_quoted(chunk)).collect();
                format!("Substring({})", chunks.join(", "))
            },
        }
    }

    fn print_joined(
        &self,
        element_ids: &[i32],
        separator: &str,
    ) -> String {
        let mut out = String::from("(");
        for (i, &id) in element_ids.iter().enumerate() {
            if i > 0 {
                out.push_str(separator);
            }
            out.push_str(&self.print_expr(id));
        }
        out.push(')');
        out
    }

    fn print_character_class(
        &self,
        expr_id: i32,
        star: bool,
    ) -> String {
        let (is_negative, ranges) = self.expr(expr_id).character_class();
        let mut out = String::from("[");
        if is_negative {
            out.push('^');
        }
        for range in ranges {
            out.push_str(&escape_codepoint(range.lower, CHAR_CLASS_ESCAPES));
            if range.lower != range.upper {
                out.push('-');
                out.push_str(&escape_codepoint(range.upper, CHAR_CLASS_ESCAPES));
            }
        }
        out.push(']');
        if star {
            out.push('*');
        }
        out
    }

    fn print_token_list(
        label: &str,
        token_ids: &[i32],
    ) -> String {
        let mut out = format!("{label}(");
        for (i, id) in token_ids.iter().enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            let _ = write!(out, "{id}");
        }
        out.push(')');
        out
    }

    fn print_quoted(bytes: &[u8]) -> String {
        format!("\"{}\"", escape_bytes(bytes))
    }

    fn print_tag_dispatch(
        &self,
        expr_id: i32,
    ) -> String {
        let td = self.tag_dispatch(expr_id);
        let mut out = String::from("TagDispatch(\n");
        for (tag, rule_id) in &td.tag_rule_pairs {
            let _ = writeln!(out, "  ({}, {}),", Self::print_quoted(tag), self.rule(*rule_id).name);
        }
        let _ = writeln!(out, "  loop_after_dispatch={},", td.loop_after_dispatch);
        out.push_str("  excludes=(");
        for (i, ex) in td.excludes.iter().enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            out.push_str(&Self::print_quoted(ex));
        }
        out.push_str(")\n)");
        out
    }

    fn print_token_tag_dispatch(
        &self,
        expr_id: i32,
    ) -> String {
        let ttd = self.token_tag_dispatch(expr_id);
        let mut out = String::from("TokenTagDispatch(\n");
        for (token_id, rule_id) in &ttd.trigger_rule_pairs {
            let _ = writeln!(out, "  ({token_id}, {}),", self.rule(*rule_id).name);
        }
        let _ = writeln!(out, "  loop_after_dispatch={},", ttd.loop_after_dispatch);
        out.push_str("  excludes=(");
        for (i, id) in ttd.excludes.iter().enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            let _ = write!(out, "{id}");
        }
        out.push_str(")\n)");
        out
    }
}

impl fmt::Display for Grammar {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        f.write_str(&self.to_string_ebnf())
    }
}

/// Formats `value` like C++ `ostream << std::setprecision(max_digits10)` (i.e. `%.9g`).
fn format_float_max_digits(value: f32) -> String {
    // std::numeric_limits<float>::max_digits10
    const PRECISION: usize = 9;
    let value = f64::from(value);
    if value == 0.0 || !value.is_finite() {
        return format!("{value}");
    }
    let scientific = format!("{value:.prec$e}", prec = PRECISION - 1);
    let (mantissa, exponent) = scientific.split_once('e').expect("scientific notation has an exponent");
    let exponent: i32 = exponent.parse().expect("scientific exponent is an integer");
    let strip = |digits: &str| {
        if digits.contains('.') {
            digits.trim_end_matches('0').trim_end_matches('.').to_owned()
        } else {
            digits.to_owned()
        }
    };
    if (-4..PRECISION as i32).contains(&exponent) {
        let decimals = (PRECISION as i32 - 1 - exponent).max(0) as usize;
        strip(&format!("{value:.decimals$}"))
    } else {
        let sign = if exponent < 0 {
            '-'
        } else {
            '+'
        };
        format!("{}e{sign}{:02}", strip(mantissa), exponent.abs())
    }
}
