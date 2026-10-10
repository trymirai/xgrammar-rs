//! JSON serialization for FSM types — uses the `"v13"` reflection layout.
//!
//! Deserialization restores the fields verbatim, so it also checks the invariants the
//! constructors normally guarantee (the C++ `Validate` hooks): the CSR offsets cover the edge
//! data, every edge target and auxiliary-data reference is in range, `edge_num` matches, and the
//! start and end states are in range.

use serde_json::{Value, json};

use super::{CompactFsm, CompactFsmWithStartEnd, CompactFsmWithStartEndWithSize, FsmEdge};
use crate::{grammar::DeserializeError, support::Compact2dArray};

fn format_error(message: impl Into<String>) -> DeserializeError {
    DeserializeError::Format(message.into())
}

/// Serializes a CSR edge table in the Compact2DArray FSMEdge form.
#[must_use]
pub fn serialize_compact_2d_fsm_edges(edges: &Compact2dArray<FsmEdge>) -> Value {
    let data: Vec<Value> = edges.data().iter().map(FsmEdge::serialize_json_value).collect();
    json!({
        "data_": data,
        "indptr_": edges.indptr(),
    })
}

/// Deserializes a CSR edge table from the Compact2DArray FSMEdge form.
///
/// # Errors
/// Returns [`DeserializeError`] when the JSON shape is invalid or the `indptr_` offsets do not
/// cover the edge data.
pub fn deserialize_compact_2d_fsm_edges(value: &Value) -> Result<Compact2dArray<FsmEdge>, DeserializeError> {
    let obj = value.as_object().ok_or_else(|| format_error("compact fsm edges must be an object"))?;
    let data = obj
        .get("data_")
        .and_then(Value::as_array)
        .ok_or_else(|| format_error("missing compact fsm data_ array"))?
        .iter()
        .map(|edge| FsmEdge::deserialize_json_value(edge).map_err(format_error))
        .collect::<Result<Vec<_>, _>>()?;
    let indptr = i32_array(obj.get("indptr_").ok_or_else(|| format_error("missing compact fsm indptr_"))?)?;
    Compact2dArray::from_data_and_indptr(data, indptr).map_err(|error| format_error(error.to_string()))
}

fn i32_of(value: &Value) -> Result<i32, DeserializeError> {
    value.as_i64().and_then(|v| i32::try_from(v).ok()).ok_or_else(|| format_error("expected a 32-bit integer"))
}

fn i32_array(value: &Value) -> Result<Vec<i32>, DeserializeError> {
    value.as_array().ok_or_else(|| format_error("expected array"))?.iter().map(i32_of).collect()
}

impl CompactFsm {
    /// Serializes this compact FSM to the C++ JSON object form.
    #[must_use]
    pub fn serialize_json_value(&self) -> Value {
        json!({
            "edges": serialize_compact_2d_fsm_edges(self.edges_table()),
            "edge_aux_data": self.edge_aux_data(),
            "edge_num": self.num_edges(),
        })
    }

    /// Deserializes a compact FSM from the C++ JSON object form.
    ///
    /// # Errors
    /// Returns [`DeserializeError`] when the JSON shape is invalid, an edge target or auxiliary
    /// data reference is out of range, or `edge_num` does not match the number of edges.
    pub fn deserialize_json_value(value: &Value) -> Result<Self, DeserializeError> {
        let obj = value.as_object().ok_or_else(|| format_error("compact fsm must be an object"))?;
        let field = |name: &str| obj.get(name).ok_or_else(|| format_error(format!("missing compact fsm {name}")));
        let edges = deserialize_compact_2d_fsm_edges(field("edges")?)?;
        let edge_aux_data = i32_array(field("edge_aux_data")?)?;
        let edge_num = field("edge_num")?.as_u64().ok_or_else(|| format_error("expected edge_num integer"))?;
        let fsm = Self::from_parts(edges, edge_aux_data);
        fsm.validate()?;
        if edge_num != fsm.num_edges() as u64 {
            return Err(format_error("edge_num does not match the number of edges"));
        }
        Ok(fsm)
    }

    /// Checks that every edge target and every auxiliary data reference is in range.
    fn validate(&self) -> Result<(), DeserializeError> {
        let aux = self.edge_aux_data();
        let aux_size = aux.len() as i64;
        for state in 0..self.num_states() {
            for edge in self.state_edges(state) {
                if !(0..self.num_states()).contains(&edge.target) {
                    return Err(format_error(format!("Edge target {} is out of range", edge.target)));
                }
                if !edge.is_aux_edge() {
                    continue;
                }
                // A repeat edge owns 3 aux elements; a token edge owns a count followed by count ids.
                let idx = i64::from(edge.max);
                let in_range = (0..aux_size).contains(&idx)
                    && if edge.is_repeat_ref() {
                        idx + 3 <= aux_size
                    } else {
                        let count = i64::from(aux[idx as usize]);
                        count >= 0 && idx + 1 + count <= aux_size
                    };
                if !in_range {
                    return Err(format_error(format!("Edge aux index {} is out of range", edge.max)));
                }
            }
        }
        Ok(())
    }
}

impl CompactFsmWithStartEnd {
    /// Serializes as `[compact_fsm, start, end_indices, is_dfa, edge_num]`.
    #[must_use]
    pub fn serialize_json_value(&self) -> Value {
        json!([self.fsm().serialize_json_value(), self.start(), self.ends(), self.is_dfa(), self.edge_num(),])
    }

    /// Deserializes from `[compact_fsm, start, end_indices, is_dfa, edge_num]`.
    ///
    /// # Errors
    /// Returns [`DeserializeError`] when the JSON shape is invalid, the FSM is malformed, or the
    /// start or an end state is out of range.
    pub fn deserialize_json_value(value: &Value) -> Result<Self, DeserializeError> {
        let arr = value.as_array().ok_or_else(|| format_error("compact fsm with start/end must be an array"))?;
        if arr.len() != 5 {
            return Err(format_error("compact fsm with start/end must have five elements"));
        }
        let fsm = CompactFsm::deserialize_json_value(&arr[0])?;
        let start = i32_of(&arr[1])?;
        let ends = i32_array(&arr[2])?;
        let is_dfa = arr[3].as_bool().ok_or_else(|| format_error("expected is_dfa boolean"))?;
        let _ = i32_of(&arr[4])?;
        let in_range = |state: &i32| (0..fsm.num_states()).contains(state);
        if !in_range(&start) || !ends.iter().all(in_range) {
            return Err(format_error("The start or end state is out of range"));
        }
        Ok(Self::new(fsm, start, ends, is_dfa))
    }
}

impl CompactFsmWithStartEndWithSize {
    /// Serializes as `[compact_fsm_with_start_end, edge_num, node_num]`.
    #[must_use]
    pub fn serialize_json_value(&self) -> Value {
        json!([self.fsm().serialize_json_value(), self.edge_num(), self.node_num(),])
    }

    /// Deserializes from `[compact_fsm_with_start_end, edge_num, node_num]`.
    ///
    /// # Errors
    /// Returns [`DeserializeError`] when the JSON shape is invalid.
    pub fn deserialize_json_value(value: &Value) -> Result<Self, DeserializeError> {
        let arr = value.as_array().ok_or_else(|| format_error("compact fsm with start/end/size must be an array"))?;
        if arr.len() != 3 {
            return Err(format_error("compact fsm with start/end/size must have three elements"));
        }
        let fsm = CompactFsmWithStartEnd::deserialize_json_value(&arr[0])?;
        let edge_num = i32_of(&arr[1])?;
        let node_num = i32_of(&arr[2])?;
        Ok(Self::new(fsm, edge_num, node_num))
    }
}
