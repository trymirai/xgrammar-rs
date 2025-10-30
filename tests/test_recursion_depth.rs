use serial_test::serial;

#[test]
#[serial]
fn test_set_get_recursion_depth() {
    let default_depth = xgrammar::get_max_recursion_depth();
    assert!(default_depth > 0);
    xgrammar::set_max_recursion_depth(1000);
    assert_eq!(xgrammar::get_max_recursion_depth(), 1000);
    xgrammar::set_max_recursion_depth(default_depth);
}
