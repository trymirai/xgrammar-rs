//! Port of `external/xgrammar/tests/cpp/test_serialization.cc`.

use std::collections::{HashMap, HashSet};

use serde_json::{Value, json};
use xgrammar::{
    fsm::{
        CompactFsm, CompactFsmWithStartEnd, Fsm, FsmEdge, deserialize_compact_2d_fsm_edges, edge_type,
        serialize_compact_2d_fsm_edges,
    },
    support::{Compact2dArray, DynamicBitset, byte_to_latin1, latin1_to_bytes},
};

/// Compact JSON dump matching picojson's escaping (non-ASCII → `\u00xx`).
fn dump(value: &Value) -> String {
    match value {
        Value::Null => "null".to_owned(),
        Value::Bool(true) => "true".to_owned(),
        Value::Bool(false) => "false".to_owned(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => dump_string(s),
        Value::Array(items) => {
            let body = items.iter().map(dump).collect::<Vec<_>>().join(",");
            format!("[{body}]")
        },
        Value::Object(map) => {
            let body = map.iter().map(|(k, v)| format!("{}:{}", dump_string(k), dump(v))).collect::<Vec<_>>().join(",");
            format!("{{{body}}}")
        },
    }
}

fn dump_string(s: &str) -> String {
    let mut out = String::from("\"");
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 || (c as u32) > 0x7E => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            },
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn serialize_bytes_string(bytes: &[u8]) -> Value {
    let latin1 = byte_to_latin1(bytes);
    Value::String(String::from_utf8(latin1).expect("latin1 is valid utf-8"))
}

fn deserialize_bytes_string(value: &Value) -> Vec<u8> {
    let s = value.as_str().expect("string json");
    latin1_to_bytes(s.as_bytes()).expect("valid latin1")
}

fn serialize_i32_set(set: &HashSet<i32>) -> Value {
    let mut values: Vec<i32> = set.iter().copied().collect();
    values.sort_unstable();
    json!(values)
}

#[test]
fn test_stl_and_builtin_types() {
    {
        let value = true;
        let json_value = json!(value);
        assert!(json_value.is_boolean());
        assert_eq!(json_value.as_bool(), Some(true));
        assert_eq!(dump(&json_value), "true");
        let deserialized = json_value.as_bool().unwrap();
        assert_eq!(deserialized, value);
        assert_eq!(dump(&json!(deserialized)), dump(&json_value));
    }

    {
        let value = 42;
        let json_value = json!(value);
        assert!(json_value.is_i64());
        assert_eq!(json_value.as_i64(), Some(42));
        assert_eq!(dump(&json_value), "42");
        let deserialized = json_value.as_i64().unwrap() as i32;
        assert_eq!(deserialized, value);
        assert_eq!(dump(&json!(deserialized)), dump(&json_value));
    }

    {
        let value = 3.14;
        let json_value = json!(value);
        assert!(json_value.is_f64());
        assert_eq!(json_value.as_f64(), Some(3.14));
        let expected = "3.14";
        assert_eq!(dump(&json_value).parse::<f64>().unwrap(), expected.parse::<f64>().unwrap());
        let deserialized = json_value.as_f64().unwrap();
        assert_eq!(deserialized, value);
        assert_eq!(dump(&json!(deserialized)), dump(&json_value));
    }

    {
        let value = "hello";
        let json_value = serialize_bytes_string(value.as_bytes());
        assert!(json_value.is_string());
        assert_eq!(json_value.as_str(), Some("hello"));
        assert_eq!(dump(&json_value), "\"hello\"");
        let deserialized = deserialize_bytes_string(&json_value);
        assert_eq!(deserialized, value.as_bytes());
        assert_eq!(dump(&serialize_bytes_string(&deserialized)), dump(&json_value));
    }

    {
        let value = vec![1, 2, 3];
        let json_value = json!(&value);
        assert!(json_value.is_array());
        assert_eq!(dump(&json_value), "[1,2,3]");
        let deserialized: Vec<i32> = serde_json::from_value(json_value.clone()).unwrap();
        assert_eq!(deserialized, value);
        assert_eq!(dump(&json!(&deserialized)), dump(&json_value));
    }

    {
        let value: HashSet<i32> = [1, 2, 3].into_iter().collect();
        let json_value = serialize_i32_set(&value);
        assert!(json_value.is_array());
        assert_eq!(dump(&json_value), "[1,2,3]");
        let deserialized: HashSet<i32> =
            json_value.as_array().unwrap().iter().map(|v| v.as_i64().unwrap() as i32).collect();
        assert_eq!(deserialized, value);
        assert_eq!(dump(&serialize_i32_set(&deserialized)), dump(&json_value));
    }

    {
        let value = (42, "hello".to_owned());
        let json_value = json!([value.0, value.1]);
        assert!(json_value.is_array());
        assert_eq!(dump(&json_value), "[42,\"hello\"]");
        let arr = json_value.as_array().unwrap();
        let deserialized = (arr[0].as_i64().unwrap() as i32, arr[1].as_str().unwrap().to_owned());
        assert_eq!(deserialized, value);
        assert_eq!(dump(&json!([deserialized.0, deserialized.1])), dump(&json_value));
    }

    {
        let value = Some(42);
        let json_value = json!(42);
        assert!(json_value.is_i64());
        assert_eq!(json_value.as_i64(), Some(42));
        assert_eq!(dump(&json_value), "42");
        let deserialized = Some(json_value.as_i64().unwrap() as i32);
        assert_eq!(deserialized, value);
        assert_eq!(dump(&json!(deserialized.unwrap())), dump(&json_value));
    }

    {
        let value: Option<i32> = None;
        let json_value = Value::Null;
        assert!(json_value.is_null());
        assert_eq!(dump(&json_value), "null");
        let mut deserialized = Some(999);
        if json_value.is_null() {
            deserialized = None;
        }
        assert_eq!(deserialized, value);
        assert_eq!(dump(&Value::Null), dump(&json_value));
    }
}

#[test]
fn test_string() {
    {
        let value = b"hello\nworld";
        let json_value = serialize_bytes_string(value);
        assert_eq!(dump(&json_value), "\"hello\\nworld\"");
        let deserialized = deserialize_bytes_string(&json_value);
        assert_eq!(deserialized, value);
    }

    {
        let value = [0xC3, 0x28];
        let json_value = serialize_bytes_string(&value);
        assert_eq!(dump(&json_value), "\"\\u00c3(\"");
        let deserialized = deserialize_bytes_string(&json_value);
        assert_eq!(deserialized, value);
    }

    {
        let value = "我".as_bytes();
        let json_value = serialize_bytes_string(value);
        assert_eq!(dump(&json_value), "\"\\u00e6\\u0088\\u0091\"");
        let deserialized = deserialize_bytes_string(&json_value);
        assert_eq!(deserialized, value);
    }
}

#[test]
fn test_fsm_edge() {
    {
        let edge = FsmEdge::new(1, 2, 3);
        let json_value = edge.serialize_json_value();
        assert!(json_value.is_array());
        assert_eq!(dump(&json_value), "[1,2,3]");
        let deserialized = FsmEdge::deserialize_json_value(&json_value).unwrap();
        assert_eq!(deserialized, edge);
        assert_eq!(dump(&deserialized.serialize_json_value()), dump(&json_value));
    }

    {
        let epsilon_edge = FsmEdge::new(edge_type::EPSILON, 0, 5);
        let json_value = epsilon_edge.serialize_json_value();
        assert_eq!(dump(&json_value), "[-1,0,5]");
        let deserialized = FsmEdge::deserialize_json_value(&json_value).unwrap();
        assert_eq!(deserialized, epsilon_edge);
        assert!(deserialized.is_epsilon());
        assert_eq!(dump(&deserialized.serialize_json_value()), dump(&json_value));
    }

    {
        let rule_edge = FsmEdge::new(edge_type::RULE_REF, 10, 7);
        let json_value = rule_edge.serialize_json_value();
        assert_eq!(dump(&json_value), "[-2,10,7]");
        let deserialized = FsmEdge::deserialize_json_value(&json_value).unwrap();
        assert_eq!(deserialized, rule_edge);
        assert!(deserialized.is_rule_ref());
        assert_eq!(deserialized.ref_rule_id(), 10);
        assert_eq!(dump(&deserialized.serialize_json_value()), dump(&json_value));
    }
}

#[test]
fn test_compact_2d_array() {
    {
        let array = Compact2dArray::<i32>::new();
        let json_value = array.serialize_json_value();
        assert!(json_value.is_object());
        assert_eq!(dump(&json_value), "{\"data_\":[],\"indptr_\":[0]}");
        let deserialized = Compact2dArray::<i32>::deserialize_json_value(&json_value).unwrap();
        assert_eq!(deserialized.len(), 0);
        assert_eq!(dump(&deserialized.serialize_json_value()), dump(&json_value));
    }

    {
        let mut array = Compact2dArray::<i32>::new();
        array.push_row(&[0, 1, 2, 3]);
        array.push_row(&[4, 5, 6, 7]);
        array.push_row(&[8, 9]);

        let json_value = array.serialize_json_value();
        assert!(json_value.is_object());
        assert_eq!(dump(&json_value), "{\"data_\":[0,1,2,3,4,5,6,7,8,9],\"indptr_\":[0,4,8,10]}");
        let obj = json_value.as_object().unwrap();
        assert!(obj.contains_key("data_"));
        assert!(obj.contains_key("indptr_"));
        assert_eq!(obj["data_"].as_array().unwrap().len(), 10);
        assert_eq!(obj["indptr_"].as_array().unwrap().len(), 4);

        let deserialized = Compact2dArray::<i32>::deserialize_json_value(&json_value).unwrap();
        assert_eq!(deserialized, array);
        assert_eq!(dump(&deserialized.serialize_json_value()), dump(&json_value));
    }

    {
        let mut array = Compact2dArray::<FsmEdge>::new();
        array.push_row(&[FsmEdge::new(1, 2, 3), FsmEdge::new(4, 5, 6)]);
        array.push_row(&[FsmEdge::new(edge_type::EPSILON, 0, 7)]);

        let json_value = serialize_compact_2d_fsm_edges(&array);
        assert!(json_value.is_object());
        assert_eq!(dump(&json_value), "{\"data_\":[[1,2,3],[4,5,6],[-1,0,7]],\"indptr_\":[0,2,3]}");
        let deserialized = deserialize_compact_2d_fsm_edges(&json_value).unwrap();
        assert_eq!(deserialized, array);
        assert_eq!(dump(&serialize_compact_2d_fsm_edges(&deserialized)), dump(&json_value));
    }
}

#[test]
fn test_dynamic_bitset() {
    {
        let bitset = DynamicBitset::new(0);
        let json_value = bitset.serialize_json_value_cpp();
        assert!(json_value.is_array());
        assert_eq!(dump(&json_value), "[0,0]");
        let deserialized = DynamicBitset::deserialize_json_value_cpp(&json_value).unwrap();
        assert_eq!(deserialized, bitset);
        assert_eq!(dump(&deserialized.serialize_json_value_cpp()), dump(&json_value));
    }

    {
        let mut bitset = DynamicBitset::new(64);
        bitset.set(0, true);
        bitset.set(10, true);
        bitset.set(63, true);

        let json_value = bitset.serialize_json_value_cpp();
        assert!(json_value.is_array());
        assert_eq!(dump(&json_value), "[64,2,1025,2147483648]");
        let arr = json_value.as_array().unwrap();
        assert_eq!(arr.len(), 4);
        assert_eq!(arr[0].as_i64(), Some(64));
        assert_eq!(arr[1].as_i64(), Some(2));

        let deserialized = DynamicBitset::deserialize_json_value_cpp(&json_value).unwrap();
        assert_eq!(deserialized, bitset);
        assert!(deserialized.get(0));
        assert!(deserialized.get(10));
        assert!(deserialized.get(63));
        assert!(!deserialized.get(1));
        assert!(!deserialized.get(62));
        assert_eq!(dump(&deserialized.serialize_json_value_cpp()), dump(&json_value));
    }

    {
        let mut bitset = DynamicBitset::new(10);
        bitset.set(0, true);
        bitset.set(5, true);
        bitset.set(9, true);

        let json_value = bitset.serialize_json_value_cpp();
        assert!(json_value.is_array());
        assert_eq!(dump(&json_value), "[10,1,545]");
        let deserialized = DynamicBitset::deserialize_json_value_cpp(&json_value).unwrap();
        assert_eq!(deserialized, bitset);
        assert_eq!(dump(&deserialized.serialize_json_value_cpp()), dump(&json_value));
    }
}

#[test]
fn test_compact_fsm() {
    {
        let mut fsm = Fsm::new(3);
        fsm.add_edge(0, 1, i32::from(b'a'), i32::from(b'a'));
        fsm.add_edge(1, 2, i32::from(b'b'), i32::from(b'b'));
        fsm.add_epsilon_edge(0, 2);
        let compact_fsm = CompactFsm::from_fsm(&fsm);

        let json_value = compact_fsm.serialize_json_value();
        assert!(json_value.is_object());
        assert_eq!(
            dump(&json_value),
            "{\"edges\":{\"data_\":[[-1,0,2],[97,97,1],[98,98,2]],\"indptr_\":[0,2,3,3]},\"edge_aux_data\":[],\"edge_num\":3}"
        );
        let deserialized = CompactFsm::deserialize_json_value(&json_value).unwrap();
        assert_eq!(deserialized.num_states(), compact_fsm.num_states());
        assert_eq!(dump(&deserialized.serialize_json_value()), dump(&json_value));
    }

    {
        let mut fsm = Fsm::new(3);
        fsm.add_edge(0, 1, i32::from(b'a'), i32::from(b'z'));
        fsm.add_rule_edge(1, 2, 5);
        fsm.add_eos_edge(2, 0);
        let compact_fsm = CompactFsm::from_fsm(&fsm);

        let json_value = compact_fsm.serialize_json_value();
        assert_eq!(
            dump(&json_value),
            "{\"edges\":{\"data_\":[[97,122,1],[-2,5,2],[-3,0,0]],\"indptr_\":[0,1,2,3]},\"edge_aux_data\":[],\"edge_num\":3}"
        );
        let deserialized = CompactFsm::deserialize_json_value(&json_value).unwrap();
        assert_eq!(deserialized.num_states(), compact_fsm.num_states());
        assert_eq!(dump(&deserialized.serialize_json_value()), dump(&json_value));
    }
}

#[test]
fn test_compact_fsm_with_start_end() {
    {
        let mut fsm = Fsm::new(3);
        fsm.add_edge(0, 1, i32::from(b'a'), i32::from(b'a'));
        fsm.add_edge(1, 2, i32::from(b'b'), i32::from(b'b'));
        fsm.add_epsilon_edge(0, 2);
        let compact_fsm = CompactFsm::from_fsm(&fsm);
        let compact_fsm_with_start_end =
            CompactFsmWithStartEnd::new(compact_fsm.clone(), 0, vec![false, false, true], false);

        let json_value = compact_fsm_with_start_end.serialize_json_value();
        assert!(json_value.is_array());
        assert_eq!(
            dump(&json_value),
            "[{\"edges\":{\"data_\":[[-1,0,2],[97,97,1],[98,98,2]],\"indptr_\":[0,2,3,3]},\"edge_aux_data\":[],\"edge_num\":3},0,[2],false,3]"
        );
        let deserialized = CompactFsmWithStartEnd::deserialize_json_value(&json_value).unwrap();
        assert_eq!(deserialized.fsm().num_states(), compact_fsm.num_states());
        assert_eq!(deserialized.start(), 0);
        assert_eq!(deserialized.ends(), &[false, false, true]);
        assert_eq!(dump(&deserialized.serialize_json_value()), dump(&json_value));
    }

    {
        let mut fsm = Fsm::new(3);
        fsm.add_edge(0, 1, i32::from(b'a'), i32::from(b'z'));
        fsm.add_rule_edge(1, 2, 5);
        fsm.add_eos_edge(2, 0);
        let compact_fsm = CompactFsm::from_fsm(&fsm);
        let compact_fsm_with_start_end =
            CompactFsmWithStartEnd::new(compact_fsm.clone(), 0, vec![false, false, true], false);

        let json_value = compact_fsm_with_start_end.serialize_json_value();
        assert_eq!(
            dump(&json_value),
            "[{\"edges\":{\"data_\":[[97,122,1],[-2,5,2],[-3,0,0]],\"indptr_\":[0,1,2,3]},\"edge_aux_data\":[],\"edge_num\":3},0,[2],false,3]"
        );
        let deserialized = CompactFsmWithStartEnd::deserialize_json_value(&json_value).unwrap();
        assert_eq!(deserialized.fsm().num_states(), compact_fsm.num_states());
        assert_eq!(deserialized.start(), 0);
        assert_eq!(deserialized.ends(), &[false, false, true]);
        assert_eq!(dump(&deserialized.serialize_json_value()), dump(&json_value));
    }
}

#[test]
fn test_complex_structures() {
    {
        let edges = vec![
            FsmEdge::new(1, 2, 3),
            FsmEdge::new(edge_type::EPSILON, 0, 4),
            FsmEdge::new(edge_type::RULE_REF, 5, 6),
        ];
        let json_value = Value::Array(edges.iter().map(FsmEdge::serialize_json_value).collect());
        assert!(json_value.is_array());
        assert_eq!(dump(&json_value), "[[1,2,3],[-1,0,4],[-2,5,6]]");
        let deserialized: Vec<FsmEdge> =
            json_value.as_array().unwrap().iter().map(|v| FsmEdge::deserialize_json_value(v).unwrap()).collect();
        assert_eq!(deserialized, edges);
        let roundtrip = Value::Array(deserialized.iter().map(FsmEdge::serialize_json_value).collect());
        assert_eq!(dump(&roundtrip), dump(&json_value));
    }

    {
        let map: HashMap<String, Vec<i32>> =
            [("key1".to_owned(), vec![1, 2, 3]), ("key2".to_owned(), vec![4, 5, 6])].into_iter().collect();
        // Stable key order for deterministic roundtrip (C++ unordered_map order is unspecified).
        let mut keys: Vec<_> = map.keys().cloned().collect();
        keys.sort();
        let mut obj = serde_json::Map::new();
        for key in &keys {
            obj.insert(key.clone(), json!(map[key]));
        }
        let json_value = Value::Object(obj);
        assert!(json_value.is_object());
        let obj = json_value.as_object().unwrap();
        assert_eq!(obj.len(), 2);
        assert!(obj.contains_key("key1"));
        assert!(obj.contains_key("key2"));

        let deserialized: HashMap<String, Vec<i32>> = obj
            .iter()
            .map(|(k, v)| (k.clone(), v.as_array().unwrap().iter().map(|n| n.as_i64().unwrap() as i32).collect()))
            .collect();
        assert_eq!(deserialized, map);

        let mut keys2: Vec<_> = deserialized.keys().cloned().collect();
        keys2.sort();
        let mut obj2 = serde_json::Map::new();
        for key in &keys2 {
            obj2.insert(key.clone(), json!(deserialized[key]));
        }
        assert_eq!(dump(&Value::Object(obj2)), dump(&json_value));
    }
}
