//! JSON serialization for [`Grammar`].
//!
//! The on-disk format uses the `"v16"` layout: rules as
//! `[name, body_expr_id, lookahead_assertion_id, is_exact_lookahead, max_tokens, max_chars,
//! capture_name, is_lazy, temperature]`, sparse `suffix_stop_infos`, and the expression store as
//! a flat `grammar_expr_data` array of `[type, data_len, data...]` rows indexed by
//! `grammar_expr_indptr` offsets. Strings are Latin-1 encoded byte strings, as in the C++.

use serde_json::{Value, json};

use super::{
    error::DeserializeError, grammar::Grammar, grammar_expr_type::GrammarExprType, rule::Rule,
    suffix_stop_info::SuffixStopInfo, tag_dispatch::EXTRA_PARAMETERS as TAG_DISPATCH_EXTRA_PARAMETER,
};
use crate::{
    config::SERIALIZATION_VERSION,
    fsm::{CompactFsm, CompactFsmWithStartEnd, CompactFsmWithStartEndWithSize},
    support::{Compact2dArray, byte_to_latin1, latin1_to_bytes, parse_json},
};

impl Grammar {
    /// Serializes the grammar to its `"v16"` JSON form (FSMs are `null`/`[]` until optimized).
    #[must_use]
    pub fn serialize_json(&self) -> String {
        serde_json::to_string(&self.serialize_json_value()).expect("grammar JSON serialization never fails")
    }

    /// Serializes the grammar to a JSON value.
    #[must_use]
    pub fn serialize_json_value(&self) -> Value {
        let mut value = self.serialize_json_value_embedded();
        value["__VERSION__"] = json!(SERIALIZATION_VERSION);
        value
    }

    /// Serializes the grammar nested inside a compiled grammar (no `__VERSION__` field).
    #[must_use]
    pub fn serialize_json_value_embedded(&self) -> Value {
        // Re-encode the expression store into the C++ flat layout: each row is
        // `[type, data_len, data...]`, indexed by cumulative offsets.
        let mut indptr: Vec<i32> = Vec::with_capacity(self.num_exprs() as usize);
        let mut data: Vec<i32> = Vec::new();
        for id in 0..self.num_exprs() {
            indptr.push(data.len() as i32);
            let expr = self.expr(id);
            data.push(expr.ty as i32);
            data.push(expr.data.len() as i32);
            data.extend_from_slice(expr.data);
        }
        let rules: Vec<Value> = self
            .rules()
            .iter()
            .map(|rule| {
                json!([
                    byte_to_latin1(rule.name.as_bytes()),
                    rule.body_expr_id,
                    rule.lookahead_assertion_id,
                    rule.is_exact_lookahead,
                    rule.max_tokens,
                    rule.max_chars,
                    byte_to_latin1(rule.capture_name.as_bytes()),
                    rule.is_lazy,
                    rule.temperature.map(f64::from),
                ])
            })
            .collect();
        let suffix_stop_infos: Vec<Value> = self
            .suffix_stop_infos()
            .iter()
            .map(|info| {
                json!([
                    info.rule_id,
                    info.hidden_suffix_bytes,
                    info.hidden_stop_bytes,
                    info.body_rule_id,
                    info.marker_rule_id,
                    byte_to_latin1(info.stop_capture_name.as_bytes()),
                ])
            })
            .collect();
        let with_fsm = self.is_optimized();
        let complete_fsm = if with_fsm {
            self.complete_fsm().serialize_json_value()
        } else {
            Value::Null
        };
        let per_rule_fsms: Vec<Value> = if with_fsm {
            self.per_rule_fsms_slice()
                .iter()
                .map(|fsm| fsm.as_ref().map_or(Value::Null, CompactFsmWithStartEndWithSize::serialize_json_value))
                .collect()
        } else {
            Vec::new()
        };
        json!({
            "rules": rules,
            "suffix_stop_infos": suffix_stop_infos,
            "grammar_expr_data": data,
            "grammar_expr_indptr": indptr,
            "root_rule_id": self.root_rule_id(),
            "complete_fsm": complete_fsm,
            "per_rule_fsms": per_rule_fsms,
            "allow_empty_rule_ids": self.allow_empty_rule_ids(),
            "optimized": self.is_optimized(),
        })
    }

    /// Deserializes a grammar from its `"v16"` JSON form.
    ///
    /// # Errors
    /// Returns [`DeserializeError`] for invalid JSON, a version mismatch, or a malformed body.
    pub fn deserialize_json(json_str: &str) -> Result<Grammar, DeserializeError> {
        let value = parse_json(json_str).map_err(DeserializeError::InvalidJson)?;
        Self::deserialize_json_value(&value)
    }

    /// Deserializes a grammar from a JSON value.
    ///
    /// # Errors
    /// Returns [`DeserializeError`] for a version mismatch or a malformed body.
    pub fn deserialize_json_value(value: &Value) -> Result<Grammar, DeserializeError> {
        Self::deserialize_json_value_impl(value, true)
    }

    /// Deserializes a grammar nested inside a compiled grammar (no `__VERSION__` field).
    ///
    /// # Errors
    /// Returns [`DeserializeError`] for a malformed body.
    pub(crate) fn deserialize_json_value_embedded(value: &Value) -> Result<Grammar, DeserializeError> {
        Self::deserialize_json_value_impl(value, false)
    }

    fn deserialize_json_value_impl(
        value: &Value,
        require_version: bool,
    ) -> Result<Grammar, DeserializeError> {
        match value.get("__VERSION__").and_then(Value::as_str) {
            Some(SERIALIZATION_VERSION) => {},
            Some(other) => {
                return Err(DeserializeError::Version {
                    expected: SERIALIZATION_VERSION.to_owned(),
                    got: other.to_owned(),
                });
            },
            None if require_version => {
                return Err(DeserializeError::Format("missing __VERSION__".to_owned()));
            },
            None => {},
        }

        let field = |name: &str| value.get(name).ok_or_else(|| format_error(&format!("missing {name}")));

        let mut rules = Vec::new();
        for entry in array_of(field("rules")?)? {
            let [name, body, lookahead, exact, max_tokens, max_chars, capture_name, is_lazy, temperature] =
                fixed_array(entry, "rule")?;
            let mut rule = Rule::new(string_of(name)?, i32_of(body)?);
            rule.lookahead_assertion_id = i32_of(lookahead)?;
            rule.is_exact_lookahead = bool_of(exact)?;
            rule.max_tokens = i32_of(max_tokens)?;
            rule.max_chars = i32_of(max_chars)?;
            rule.capture_name = string_of(capture_name)?;
            rule.is_lazy = bool_of(is_lazy)?;
            rule.temperature = if temperature.is_null() {
                None
            } else {
                Some(temperature.as_f64().ok_or_else(|| format_error("expected number"))? as f32)
            };
            rules.push(rule);
        }

        let mut suffix_stop_infos = Vec::new();
        for entry in array_of(field("suffix_stop_infos")?)? {
            let [rule_id, hidden_suffix_bytes, hidden_stop_bytes, body_rule_id, marker_rule_id, stop_capture_name] =
                fixed_array(entry, "suffix_stop_info")?;
            suffix_stop_infos.push(SuffixStopInfo {
                rule_id: i32_of(rule_id)?,
                hidden_suffix_bytes: i32_of(hidden_suffix_bytes)?,
                hidden_stop_bytes: i32_of(hidden_stop_bytes)?,
                body_rule_id: i32_of(body_rule_id)?,
                marker_rule_id: i32_of(marker_rule_id)?,
                stop_capture_name: string_of(stop_capture_name)?,
            });
        }
        if !suffix_stop_infos.is_sorted_by(|a, b| a.rule_id < b.rule_id) {
            return Err(format_error("suffix_stop_infos must be sorted by rule id"));
        }

        let data = i32_array(field("grammar_expr_data")?)?;
        let indptr = i32_array(field("grammar_expr_indptr")?)?;
        let mut exprs: Compact2dArray<i32> = Compact2dArray::new();
        for (expr_id, &start) in indptr.iter().enumerate() {
            let start = usize::try_from(start)
                .ok()
                .filter(|&start| start + 2 <= data.len())
                .ok_or_else(|| format_error(&format!("grammar_expr_indptr[{expr_id}] is out of range")))?;
            let (ty, len) = (data[start], data[start + 1]);
            let end = usize::try_from(len)
                .ok()
                .map(|len| start + 2 + len)
                .filter(|&end| end <= data.len())
                .ok_or_else(|| format_error(&format!("The length of grammar expr {expr_id} is out of range")))?;
            if GrammarExprType::try_from(ty).is_err() {
                return Err(format_error(&format!("Unknown type of grammar expr {expr_id}")));
            }
            // Drop the C++ `data_len` header element; keep `[type, data...]`.
            exprs.push_row_noncontiguous(ty, &data[start + 2..end]);
        }

        let root_rule_id = i32_of(field("root_rule_id")?)?;
        let allow_empty = i32_array(field("allow_empty_rule_ids")?)?;
        let optimized = bool_of(field("optimized")?)?;

        let mut grammar = Grammar::from_parts(rules, exprs, root_rule_id);
        grammar.set_suffix_stop_infos(suffix_stop_infos);
        grammar.set_allow_empty_rule_ids(allow_empty);
        let complete_fsm = field("complete_fsm")?;
        let per_rule_json = array_of(field("per_rule_fsms")?)?;
        if !complete_fsm.is_null() || !per_rule_json.is_empty() {
            let complete_fsm = if complete_fsm.is_null() {
                CompactFsm::default()
            } else {
                CompactFsm::deserialize_json_value(complete_fsm)?
            };
            // Generated views share the complete FSM; independently serialized rule graphs stay distinct.
            let per_rule_fsms = per_rule_json
                .iter()
                .map(|entry| {
                    if entry.is_null() {
                        return Ok(None);
                    }
                    let fsm = CompactFsmWithStartEndWithSize::deserialize_json_value(entry)?;
                    let view = fsm.fsm();
                    if view.fsm() != &complete_fsm {
                        return Ok(Some(fsm));
                    }
                    let shared = CompactFsmWithStartEnd::new(
                        complete_fsm.clone(),
                        view.start(),
                        view.ends().to_vec(),
                        view.is_dfa(),
                    );
                    Ok(Some(CompactFsmWithStartEndWithSize::new(shared, fsm.edge_num(), fsm.node_num())))
                })
                .collect::<Result<_, _>>()?;
            grammar.set_fsms(complete_fsm, per_rule_fsms);
        }
        grammar.set_optimized(optimized);
        grammar.validate(!field("complete_fsm")?.is_null()).map_err(|message| format_error(&message))?;
        Ok(grammar)
    }

    /// Checks that every id stored in the grammar is in range. Used after deserialization,
    /// where the fields are restored verbatim and none of the builder checks run.
    fn validate(
        &self,
        has_complete_fsm: bool,
    ) -> Result<(), String> {
        let num_rules = self.num_rules();
        let num_exprs = self.num_exprs();
        let rule_ok = |id: i32| (0..num_rules).contains(&id);
        let expr_ok = |id: i32| (0..num_exprs).contains(&id);

        for expr_id in 0..num_exprs {
            let expr = self.expr(expr_id);
            let data = expr.data;
            let size = data.len();
            let ok = match expr.ty {
                GrammarExprType::ByteString
                | GrammarExprType::EmptyStr
                | GrammarExprType::Token
                | GrammarExprType::ExcludeToken => true,
                GrammarExprType::CharacterClass | GrammarExprType::CharacterClassStar => size % 2 == 1,
                GrammarExprType::RuleRef => size == 1 && rule_ok(data[0]),
                GrammarExprType::Sequence | GrammarExprType::Choices => data.iter().all(|&id| expr_ok(id)),
                GrammarExprType::TagDispatch => {
                    let extra = TAG_DISPATCH_EXTRA_PARAMETER;
                    size >= extra
                        && (size - extra) % 2 == 0
                        && data[..size - extra].chunks_exact(2).all(|pair| expr_ok(pair[0]) && rule_ok(pair[1]))
                        && expr_ok(data[size - 1])
                        && self.expr(data[size - 1]).ty == GrammarExprType::Choices
                },
                GrammarExprType::Repeat => size == 3 && rule_ok(data[0]),
                GrammarExprType::TokenTagDispatch => match data.first().map(|&cnt| usize::try_from(cnt)) {
                    Some(Ok(trigger_cnt)) if 1 + 2 * trigger_cnt + 2 <= size => {
                        let exclude_cnt = data[1 + 2 * trigger_cnt + 1];
                        (0..trigger_cnt).all(|i| rule_ok(data[2 + 2 * i]))
                            && usize::try_from(exclude_cnt).is_ok_and(|cnt| 1 + 2 * trigger_cnt + 2 + cnt == size)
                    },
                    _ => false,
                },
                GrammarExprType::Regex => size >= 1,
                GrammarExprType::Substring => {
                    let mut index = 0;
                    let mut ok = true;
                    while ok && index < size {
                        let chunk_len = data[index];
                        index += 1;
                        ok = usize::try_from(chunk_len).is_ok_and(|len| index + len <= size);
                        index += chunk_len.max(0) as usize;
                    }
                    ok
                },
            };
            if !ok {
                return Err(format!("Grammar expr {expr_id} is malformed"));
            }
        }

        for (rule_id, rule) in self.rules().iter().enumerate() {
            if !expr_ok(rule.body_expr_id)
                || (rule.lookahead_assertion_id != -1 && !expr_ok(rule.lookahead_assertion_id))
            {
                return Err(format!("Rule {rule_id} refers to a grammar expr out of range"));
            }
        }
        if !rule_ok(self.root_rule_id()) {
            return Err(format!("root_rule_id {} is out of range", self.root_rule_id()));
        }
        if self.suffix_stop_infos().iter().any(|info| {
            !rule_ok(info.rule_id)
                || (info.body_rule_id != -1 && !rule_ok(info.body_rule_id))
                || (info.marker_rule_id != -1 && !rule_ok(info.marker_rule_id))
        }) {
            return Err("suffix_stop_infos refers to a rule out of range".to_owned());
        }
        if !self.allow_empty_rule_ids().iter().all(|&id| rule_ok(id)) {
            return Err("allow_empty_rule_ids refers to a rule out of range".to_owned());
        }

        // The parser reads the repeat info of a per-rule FSM edge from complete_fsm, so the aux
        // indices of per-rule repeat edges must also be valid for complete_fsm.
        let complete_aux = self.complete_fsm().edge_aux_data();
        let fsm_rules_ok = |fsm: &CompactFsm| {
            let aux = fsm.edge_aux_data();
            (0..fsm.num_states()).all(|state| {
                fsm.state_edges(state).iter().all(|edge| {
                    !(edge.is_rule_ref() && !rule_ok(edge.max))
                        && !(edge.is_repeat_ref()
                            && !usize::try_from(edge.max)
                                .ok()
                                .and_then(|index| aux.get(index))
                                .is_some_and(|&rule_id| rule_ok(rule_id)))
                })
            })
        };
        if has_complete_fsm && !fsm_rules_ok(self.complete_fsm()) {
            return Err("complete_fsm refers to a rule out of range".to_owned());
        }
        let per_rule_fsms = self.per_rule_fsms_slice();
        if self.is_optimized() && (!has_complete_fsm || per_rule_fsms.len() != num_rules as usize) {
            return Err("An optimized grammar must have complete_fsm and one FSM per rule".to_owned());
        }
        for (rule_id, fsm) in per_rule_fsms.iter().enumerate() {
            let Some(fsm) = fsm else {
                if self.is_optimized() {
                    return Err(format!("Rule {rule_id} has no FSM in an optimized grammar"));
                }
                continue;
            };
            let fsm = fsm.fsm().fsm();
            if !fsm_rules_ok(fsm) {
                return Err(format!("The FSM of rule {rule_id} refers to a rule out of range"));
            }
            let repeat_ok = (0..fsm.num_states()).all(|state| {
                fsm.state_edges(state).iter().all(|edge| {
                    !edge.is_repeat_ref()
                        || usize::try_from(edge.max)
                            .is_ok_and(|index| index + 3 <= complete_aux.len() && rule_ok(complete_aux[index]))
                })
            });
            if !repeat_ok {
                return Err(format!(
                    "The FSM of rule {rule_id} has a repeat edge that is out of range of complete_fsm"
                ));
            }
        }
        Ok(())
    }
}

fn format_error(message: &str) -> DeserializeError {
    DeserializeError::Format(message.to_owned())
}

fn array_of(value: &Value) -> Result<&Vec<Value>, DeserializeError> {
    value.as_array().ok_or_else(|| format_error("expected array"))
}

fn fixed_array<'a, const N: usize>(
    value: &'a Value,
    what: &str,
) -> Result<&'a [Value; N], DeserializeError> {
    array_of(value)?.as_slice().try_into().map_err(|_| format_error(&format!("{what} arity")))
}

fn i32_of(value: &Value) -> Result<i32, DeserializeError> {
    value.as_i64().and_then(|v| i32::try_from(v).ok()).ok_or_else(|| format_error("expected integer"))
}

fn bool_of(value: &Value) -> Result<bool, DeserializeError> {
    value.as_bool().ok_or_else(|| format_error("expected boolean"))
}

fn string_of(value: &Value) -> Result<String, DeserializeError> {
    let latin1 = value.as_str().ok_or_else(|| format_error("expected string"))?;
    let bytes = latin1_to_bytes(latin1.as_bytes()).map_err(|_| format_error("invalid latin-1 string"))?;
    String::from_utf8(bytes).map_err(|_| format_error("invalid UTF-8 string"))
}

fn i32_array(value: &Value) -> Result<Vec<i32>, DeserializeError> {
    array_of(value)?.iter().map(i32_of).collect()
}
