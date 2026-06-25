//! JSON serialization for [`Grammar`] — a port of the grammar (de)serialization in
//! `cpp/support/json_serializer.h` + `grammar.cc`.
//!
//! The on-disk format matches the C++ `"v11"` layout: rules as
//! `[name, body_expr_id, lookahead_assertion_id, is_exact_lookahead]`, and the expression
//! store as a flat `grammar_expr_indptr` array of `[type, data_len, data...]` rows indexed by
//! `grammar_expr_data` offsets (the two key names are swapped relative to their contents, a
//! C++ historical quirk reproduced here for byte parity).

use serde_json::{Value, json};

use super::{grammar::Grammar, rule::Rule};
use crate::{
    config::SERIALIZATION_VERSION,
    fsm::CompactFsm,
    support::Compact2dArray,
};

/// An error from [`Grammar::deserialize_json`] (and other deserializers) — a port of the C++
/// `SerializationError` family.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DeserializeError {
    /// The input was not valid JSON.
    #[error("invalid JSON: {0}")]
    InvalidJson(String),
    /// The serialized `__VERSION__` did not match the current one.
    #[error("version mismatch: expected {expected}, got {got}")]
    Version {
        /// The version this build expects.
        expected: String,
        /// The version found in the input.
        got: String,
    },
    /// The JSON was structurally valid but missing/ill-typed for the target type.
    #[error("invalid format: {0}")]
    Format(String),
}

impl Grammar {
    /// Serializes the grammar to its `"v13"` JSON form.
    #[must_use]
    pub fn serialize_json(&self) -> String {
        serde_json::to_string(&self.serialize_json_value())
            .expect("grammar JSON serialization never fails")
    }

    /// Serializes the grammar to a JSON value.
    #[must_use]
    pub fn serialize_json_value(&self) -> Value {
        // Re-encode the expression store into the C++ flat layout: each row is
        // `[type, data_len, data...]`, indexed by cumulative offsets.
        let mut offsets: Vec<i32> =
            Vec::with_capacity(self.num_exprs() as usize + 1);
        let mut flat: Vec<i32> = Vec::new();
        for id in 0..self.num_exprs() {
            offsets.push(flat.len() as i32);
            let expr = self.expr(id);
            flat.push(expr.ty as i32);
            flat.push(expr.data.len() as i32);
            flat.extend_from_slice(expr.data);
        }

        let rules: Vec<Value> = self
            .rules()
            .iter()
            .map(|r| {
                json!([
                    r.name,
                    r.body_expr_id,
                    r.lookahead_assertion_id,
                    r.is_exact_lookahead
                ])
            })
            .collect();

        let complete_fsm = if self.is_optimized() {
            self.complete_fsm().serialize_json_value()
        } else {
            Value::Null
        };
        let per_rule_fsms: Vec<Value> = if self.is_optimized() {
            self.per_rule_fsms_slice()
                .iter()
                .map(|opt| {
                    opt.as_ref()
                        .map(|fsm| fsm.serialize_json_value())
                        .unwrap_or(Value::Null)
                })
                .collect()
        } else {
            Vec::new()
        };

        json!({
            "rules": rules,
            "grammar_expr_data": offsets,
            "grammar_expr_indptr": flat,
            "root_rule_id": self.root_rule_id(),
            "complete_fsm": complete_fsm,
            "per_rule_fsms": per_rule_fsms,
            "allow_empty_rule_ids": self.allow_empty_rule_ids(),
            "optimized": self.is_optimized(),
            "__VERSION__": SERIALIZATION_VERSION,
        })
    }

    /// Deserializes a grammar from its `"v13"` JSON form.
    ///
    /// # Errors
    /// Returns [`DeserializeError`] for invalid JSON, a version mismatch, or a malformed body.
    pub fn deserialize_json(
        json_str: &str
    ) -> Result<Grammar, DeserializeError> {
        let value: Value = serde_json::from_str(json_str)
            .map_err(|e| DeserializeError::InvalidJson(e.to_string()))?;
        Self::deserialize_json_value(&value)
    }

    /// Deserializes a grammar from a JSON value.
    ///
    /// # Errors
    /// Returns [`DeserializeError`] for a version mismatch or a malformed body.
    pub fn deserialize_json_value(
        value: &Value
    ) -> Result<Grammar, DeserializeError> {

        match value.get("__VERSION__").and_then(Value::as_str) {
            Some(SERIALIZATION_VERSION) => {},
            Some(other) => {
                return Err(DeserializeError::Version {
                    expected: SERIALIZATION_VERSION.to_owned(),
                    got: other.to_owned(),
                });
            },
            None => {
                return Err(DeserializeError::Format(
                    "missing __VERSION__".to_owned(),
                ));
            },
        }

        let field = |name: &str| {
            value.get(name).ok_or_else(|| {
                DeserializeError::Format(format!("missing {name}"))
            })
        };

        let rules_json = field("rules")?
            .as_array()
            .ok_or_else(|| DeserializeError::Format("rules".to_owned()))?;
        let mut rules = Vec::with_capacity(rules_json.len());
        for r in rules_json {
            let arr = r.as_array().ok_or_else(|| {
                DeserializeError::Format("rule entry".to_owned())
            })?;
            if arr.len() != 4 {
                return Err(DeserializeError::Format("rule arity".to_owned()));
            }
            let name = arr[0].as_str().ok_or_else(|| {
                DeserializeError::Format("rule name".to_owned())
            })?;
            let body = i32_of(&arr[1])?;
            let lookahead = i32_of(&arr[2])?;
            let exact = arr[3].as_bool().ok_or_else(|| {
                DeserializeError::Format("rule flag".to_owned())
            })?;
            let mut rule = Rule::new(name, body);
            rule.lookahead_assertion_id = lookahead;
            rule.is_exact_lookahead = exact;
            rules.push(rule);
        }

        let offsets = i32_array(field("grammar_expr_data")?)?;
        let flat = i32_array(field("grammar_expr_indptr")?)?;
        let mut exprs: Compact2dArray<i32> = Compact2dArray::new();
        for (i, &start) in offsets.iter().enumerate() {
            let start = start as usize;
            let end = offsets.get(i + 1).map_or(flat.len(), |&o| o as usize);
            let row = flat.get(start..end).ok_or_else(|| {
                DeserializeError::Format("expr offset out of range".to_owned())
            })?;
            if row.len() < 2 {
                return Err(DeserializeError::Format("expr header".to_owned()));
            }
            let data_len = row[1] as usize;
            // Drop the C++ `data_len` header element; keep `[type, data...]`.
            let mut my_row = Vec::with_capacity(1 + data_len);
            my_row.push(row[0]);
            my_row.extend_from_slice(&row[2..2 + data_len]);
            exprs.push_row(&my_row);
        }

        let root_rule_id = i32_of(field("root_rule_id")?)?;
        let allow_empty = i32_array(field("allow_empty_rule_ids")?)?;
        let optimized =
            value.get("optimized").and_then(Value::as_bool).unwrap_or(false);

        let mut grammar = Grammar::from_parts(rules, exprs, root_rule_id);
        grammar.set_allow_empty_rule_ids(allow_empty);
        if optimized {
            let complete_fsm = CompactFsm::deserialize_json_value(field("complete_fsm")?)?;
            let per_rule_json = field("per_rule_fsms")?
                .as_array()
                .ok_or_else(|| DeserializeError::Format("per_rule_fsms".to_owned()))?;
            let mut per_rule_fsms = Vec::with_capacity(per_rule_json.len());
            for entry in per_rule_json {
                per_rule_fsms.push(if entry.is_null() {
                    None
                } else {
                    Some(
                        crate::fsm::CompactFsmWithStartEndWithSize::deserialize_json_value(
                            entry,
                        )?,
                    )
                });
            }
            grammar.set_fsms(complete_fsm, per_rule_fsms);
            grammar.set_optimized(true);
        } else {
            grammar.set_optimized(optimized);
        }
        Ok(grammar)
    }
}

fn i32_of(value: &Value) -> Result<i32, DeserializeError> {
    value
        .as_i64()
        .map(|v| v as i32)
        .ok_or_else(|| DeserializeError::Format("expected integer".to_owned()))
}

fn i32_array(value: &Value) -> Result<Vec<i32>, DeserializeError> {
    value
        .as_array()
        .ok_or_else(|| DeserializeError::Format("expected array".to_owned()))?
        .iter()
        .map(i32_of)
        .collect()
}
