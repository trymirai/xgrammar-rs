//! JSON serialization for FSM types — matches the C++ `"v13"` reflection layout.

use serde_json::{Value, json};

use super::{
    CompactFsm, CompactFsmWithStartEnd, CompactFsmWithStartEndWithSize, FsmEdge,
};
use crate::grammar::DeserializeError;

fn ends_to_indices(ends: &[bool]) -> Vec<i32> {
    ends.iter()
        .enumerate()
        .filter_map(|(state, &accepting)| accepting.then_some(state as i32))
        .collect()
}

fn indices_to_ends(
    indices: &[i32],
    num_states: i32,
) -> Result<Vec<bool>, DeserializeError> {
    let mut ends = vec![false; num_states as usize];
    for &state in indices {
        let idx = state as usize;
        if idx >= ends.len() {
            return Err(DeserializeError::Format(format!(
                "end state {state} out of range for {num_states} states"
            )));
        }
        ends[idx] = true;
    }
    Ok(ends)
}

fn serialize_edges(edges: &crate::support::Compact2dArray<FsmEdge>) -> Value {
    let data: Vec<Value> = edges
        .iter()
        .flat_map(|row| {
            row.iter()
                .map(|edge| json!([edge.min, edge.max, edge.target]))
        })
        .collect();
    json!({
        "data_": data,
        "indptr_": edges.indptr(),
    })
}

fn deserialize_edges(
    value: &Value
) -> Result<crate::support::Compact2dArray<FsmEdge>, DeserializeError> {
    let obj = value.as_object().ok_or_else(|| {
        DeserializeError::Format("compact fsm edges must be an object".to_owned())
    })?;
    let data = obj.get("data_").ok_or_else(|| {
        DeserializeError::Format("missing compact fsm data_".to_owned())
    })?;
    let indptr = i32_array(
        obj.get("indptr_")
            .ok_or_else(|| DeserializeError::Format("missing compact fsm indptr_".to_owned()))?,
    )?;
    let rows = data.as_array().ok_or_else(|| {
        DeserializeError::Format("compact fsm data_ must be an array".to_owned())
    })?;
    let mut edges = crate::support::Compact2dArray::<FsmEdge>::new();
    for state in 0..indptr.len().saturating_sub(1) {
        let start = indptr[state] as usize;
        let end = indptr[state + 1] as usize;
        let row: Result<Vec<FsmEdge>, DeserializeError> = rows[start..end]
            .iter()
            .map(parse_edge)
            .collect();
        edges.push_row(&row?);
    }
    Ok(edges)
}

fn parse_edge(value: &Value) -> Result<FsmEdge, DeserializeError> {
    let arr = value.as_array().ok_or_else(|| {
        DeserializeError::Format("fsm edge must be an array".to_owned())
    })?;
    if arr.len() != 3 {
        return Err(DeserializeError::Format(
            "fsm edge must have three elements".to_owned(),
        ));
    }
    Ok(FsmEdge::new(
        i32_of(&arr[0])?,
        i32_of(&arr[1])?,
        i32_of(&arr[2])?,
    ))
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

impl CompactFsm {
    /// Serializes this compact FSM to the C++ JSON object form.
    #[must_use]
    pub fn serialize_json_value(&self) -> Value {
        json!({
            "edges": serialize_edges(self.edges_table()),
            "edge_aux_data": self.edge_aux_data(),
            "edge_num": self.num_edges(),
        })
    }

    /// Deserializes a compact FSM from the C++ JSON object form.
    ///
    /// # Errors
    /// Returns [`DeserializeError`] when the JSON shape is invalid.
    pub fn deserialize_json_value(
        value: &Value
    ) -> Result<Self, DeserializeError> {
        let obj = value.as_object().ok_or_else(|| {
            DeserializeError::Format("compact fsm must be an object".to_owned())
        })?;
        let edges = obj.get("edges").ok_or_else(|| {
            DeserializeError::Format("missing compact fsm edges".to_owned())
        })?;
        let edge_aux_data = i32_array(obj.get("edge_aux_data").ok_or_else(|| {
            DeserializeError::Format("missing compact fsm edge_aux_data".to_owned())
        })?)?;
        Ok(Self::from_parts(deserialize_edges(edges)?, edge_aux_data))
    }
}

impl CompactFsmWithStartEnd {
    /// Serializes as `[compact_fsm, start, end_indices, is_dfa]`.
    #[must_use]
    pub fn serialize_json_value(&self) -> Value {
        json!([
            self.fsm().serialize_json_value(),
            self.start(),
            ends_to_indices(self.ends()),
            self.is_dfa(),
        ])
    }

    /// Deserializes from `[compact_fsm, start, end_indices, is_dfa]`.
    ///
    /// # Errors
    /// Returns [`DeserializeError`] when the JSON shape is invalid.
    pub fn deserialize_json_value(
        value: &Value
    ) -> Result<Self, DeserializeError> {
        let arr = value.as_array().ok_or_else(|| {
            DeserializeError::Format(
                "compact fsm with start/end must be an array".to_owned(),
            )
        })?;
        if arr.len() != 4 {
            return Err(DeserializeError::Format(
                "compact fsm with start/end must have four elements".to_owned(),
            ));
        }
        let fsm = CompactFsm::deserialize_json_value(&arr[0])?;
        let start = i32_of(&arr[1])?;
        let end_indices = i32_array(&arr[2])?;
        let is_dfa = arr[3].as_bool().ok_or_else(|| {
            DeserializeError::Format("expected is_dfa boolean".to_owned())
        })?;
        let ends = indices_to_ends(&end_indices, fsm.num_states())?;
        Ok(Self::new(fsm, start, ends, is_dfa))
    }
}

impl CompactFsmWithStartEndWithSize {
    /// Serializes as `[[compact_fsm, start, end_indices, is_dfa, node_num], edge_num, node_num]`.
    #[must_use]
    pub fn serialize_json_value(&self) -> Value {
        json!([
            [
                self.fsm().fsm().serialize_json_value(),
                self.fsm().start(),
                ends_to_indices(self.fsm().ends()),
                self.fsm().is_dfa(),
                self.node_num(),
            ],
            self.edge_num(),
            self.node_num(),
        ])
    }

    /// Deserializes the C++ nested array form.
    ///
    /// # Errors
    /// Returns [`DeserializeError`] when the JSON shape is invalid.
    pub fn deserialize_json_value(
        value: &Value
    ) -> Result<Self, DeserializeError> {
        let arr = value.as_array().ok_or_else(|| {
            DeserializeError::Format(
                "compact fsm with start/end/size must be an array".to_owned(),
            )
        })?;
        let (inner, edge_num, node_num) = if arr.len() == 3 {
            (&arr[0], i32_of(&arr[1])?, i32_of(&arr[2])?)
        } else if arr.len() == 5 {
            let fsm = CompactFsm::deserialize_json_value(&arr[0])?;
            let edge_num = fsm.num_edges() as i32;
            let start = i32_of(&arr[1])?;
            let end_indices = i32_array(&arr[2])?;
            let is_dfa = arr[3].as_bool().ok_or_else(|| {
                DeserializeError::Format("expected is_dfa boolean".to_owned())
            })?;
            let node_num = i32_of(&arr[4])?;
            let ends = indices_to_ends(&end_indices, fsm.num_states())?;
            return Ok(Self::new(
                CompactFsmWithStartEnd::new(fsm, start, ends, is_dfa),
                edge_num,
                node_num,
            ));
        } else {
            return Err(DeserializeError::Format(
                "compact fsm with start/end/size has unexpected length".to_owned(),
            ));
        };
        let inner_arr = inner.as_array().ok_or_else(|| {
            DeserializeError::Format(
                "compact fsm with start/end inner must be an array".to_owned(),
            )
        })?;
        if inner_arr.len() != 5 {
            return Err(DeserializeError::Format(
                "compact fsm with start/end inner must have five elements".to_owned(),
            ));
        }
        let fsm = CompactFsm::deserialize_json_value(&inner_arr[0])?;
        let start = i32_of(&inner_arr[1])?;
        let end_indices = i32_array(&inner_arr[2])?;
        let is_dfa = inner_arr[3].as_bool().ok_or_else(|| {
            DeserializeError::Format("expected is_dfa boolean".to_owned())
        })?;
        let node_num = i32_of(&inner_arr[4])?;
        let ends = indices_to_ends(&end_indices, fsm.num_states())?;
        Ok(Self::new(
            CompactFsmWithStartEnd::new(fsm, start, ends, is_dfa),
            edge_num,
            node_num,
        ))
    }
}
