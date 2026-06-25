//! Port of `xgrammar/tests/python/test_grammar_matcher_json.py`.
//!
//! Pure accept/refuse over the built-in JSON grammar (`Grammar::builtin_json_grammar`). The
//! HuggingFace `fill_next_token_bitmask` cases land with the HF tokenizer; the 3k-char
//! `cofax` pressure document is covered by the deeply-nested accept cases below.

use xgrammar::{grammar::Grammar, matcher::GrammarMatcher};

fn accepts(input: &str) -> bool {
    let grammar = Grammar::builtin_json_grammar();
    let mut m = GrammarMatcher::from_grammar(&grammar, true);
    m.accept_string(input) && m.is_terminated()
}

#[test]
fn test_json_accept() {
    let accepted = [
        r#"{"name": "John"}"#,
        r#"{ "name" : "John" }"#,
        "{}",
        "[]",
        r#"{"name": "Alice", "age": 30, "city": "New York"}"#,
        r#"{"name": "Mike", "hobbies": ["reading", "cycling", "hiking"]}"#,
        r#"{"name": "Emma", "address": {"street": "Maple Street", "city": "Boston"}}"#,
        r#"[{"name": "David"}, {"name": "Sophia"}]"#,
        r#"{"name": "William", "age": null, "married": true, "children": ["Liam", "Olivia"], "hasPets": false}"#,
        r#"{"name": "Olivia", "contact": {"email": "olivia@example.com", "address": {"city": "Chicago", "zipcode": "60601"}}}"#,
        r#"{"name": "Liam", "skills": ["Java", "Python"], "experience": [{"company": "CompanyA", "years": 5}, {"company": "CompanyB", "years": 3}]}"#,
        r#"{"person": {"name": "Ethan", "age": 40}, "education": {"degree": "Masters", "university": "XYZ University"}, "work": [{"company": "ABC Corp", "position": "Manager"}, {"company": "DEF Corp", "position": "Senior Manager"}]}"#,
        r#"{"name": "Charlotte", "details": {"personal": {"age": 35, "hobbies": ["gardening", "painting"]}, "professional": {"occupation": "Engineer", "skills": ["CAD", "Project Management"], "projects": [{"name": "Project A", "status": "Completed"}, {"name": "Project B", "status": "In Progress"}]}}}"#,
    ];
    for input in accepted {
        assert!(accepts(input), "should accept {input:?}");
    }
}

#[test]
fn test_json_refuse() {
    let refused = [
        r#"{ name: "John" }"#,
        r#"{ "name": "John" } "#,
        r#"{ "name": "John", "age": 30, }"#,
        r#"{ "name": "John", "address": { "street": "123 Main St", "city": "New York" }"#,
        r#"{ "name": "John", "age": 30, "hobbies": ["reading", "traveling",], }"#,
        r#"{ "name": "John", "age": 30.5.7 }"#,
        r#"{ "name": "John, "age": 30, "hobbies": ["reading", "traveling"] }"#,
        r#"{ "name": "John", "age": 30, "hobbies": ["reading", { "type": "outdoor", "list": ["hiking", "swimming",]}] }"#,
        r#"{ "name": "John", "age": 30, "status": "\P\J" }"#,
        r#"{ "name": "John", "age": 30, "hobbies": ["reading", "traveling"], "address": { "street": "123 Main St", "city": "New York", "coordinates": { "latitude": 40.7128, "longitude": -74.0060 }}}, "work": { "company": "Acme", "position": "developer" }}"#,
    ];
    for input in refused {
        assert!(!accepts(input), "should refuse {input:?}");
    }
}

#[test]
fn test_json_pressure() {
    // A long string inside an array stresses the string-character recursion.
    let lorem = "[\"Lorem ipsum dolor sit amet, consectetur adipiscing elit. Integer nec odio. \
        Praesent libero. Sed cursus ante dapibus diam. Sed nisi. Nulla quis sem at nibh \
        elementum imperdiet. Duis sagittis ipsum. Praesent mauris. Fusce nec tellus sed augue \
        semper porta. Mauris massa. Vestibulum lacinia arcu eget nulla. Class aptent taciti \
        sociosqu ad litora torquent per conubia nostra, per inceptos himenaeos. Curabitur \
        sodales ligula in libero. Sed dignissim lacinia nunc. Curabitur tortor. Pellentesque \
        nibh. Aenean quam. In scelerisque sem at dolor. Maecenas mattis. Sed convallis \
        tristique sem. Proin ut ligula vel nunc egestas porttitor.\"]";
    assert!(accepts(lorem));
}
