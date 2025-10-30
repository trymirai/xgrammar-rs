use serial_test::serial;
use xgrammar::Grammar;

#[test]
#[serial]
fn test_e2e_to_string_roundtrip() {
    let before = r#"root ::= ((b c) | (b root))
 b ::= ((b_1 d))
 c ::= ((c_1))
 d ::= ((d_1))
 b_1 ::= ("" | ("b" b_1)) (=(d))
 c_1 ::= (([acep-z] c_1) | ([acep-z])) (=("d"))
 d_1 ::= ("" | ("d"))
"#;
    let g1 = Grammar::from_ebnf(before, "root");
    let s1 = g1.to_string();
    let g2 = Grammar::from_ebnf(&s1, "root");
    let s2 = g2.to_string();
    assert_eq!(s1, s2);
}
