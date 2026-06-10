use crate::expected_compile;
use hail::Executor;

/// Property: "hello".len == 5
#[test]
fn dot_property_string_len() {
    let program = expected_compile(r#""hello".len()"#.to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 5);
}

/// Property: "".is_empty == true
#[test]
fn dot_property_string_is_empty_true() {
    let program = expected_compile(r#""".is_empty()"#.to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<bool>(&program)
        .expect("Should execute");
    assert_eq!(result, true);
}

/// Property: "hi".is_empty == false
#[test]
fn dot_property_string_is_empty_false() {
    let program = expected_compile(r#""hi".is_empty()"#.to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<bool>(&program)
        .expect("Should execute");
    assert_eq!(result, false);
}

/// Method: "ha".repeat(3) == "hahaha"
#[test]
fn dot_method_string_repeat() {
    let program = expected_compile(r#""ha".repeat(3)"#.to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<String>(&program)
        .expect("Should execute");
    assert_eq!(result, "hahaha".to_string());
}

/// Method: "hello world".contains("world") == true
#[test]
fn dot_method_string_contains_true() {
    let program = expected_compile(r#""hello world".contains("world")"#.to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<bool>(&program)
        .expect("Should execute");
    assert_eq!(result, true);
}

/// Method: "hello world".contains("xyz") == false
#[test]
fn dot_method_string_contains_false() {
    let program = expected_compile(r#""hello world".contains("xyz")"#.to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<bool>(&program)
        .expect("Should execute");
    assert_eq!(result, false);
}

/// Property: (-7).abs == 7
#[test]
fn dot_property_int_abs() {
    let program = expected_compile("(-7).abs()".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 7);
}

/// Method: 2.pow(10) == 1024
#[test]
fn dot_method_int_pow() {
    let program = expected_compile("2.pow(10)".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 1024);
}

/// Chained: "haha".len.abs == 4   (len returns i64, abs is a property of Int)
#[test]
fn dot_chain_len_abs() {
    let program = expected_compile(r#""haha".len().abs()"#.to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 4);
}

/// Property result used in expression: "hi".len + 3 == 5
#[test]
fn dot_property_result_in_expr() {
    let program = expected_compile(r#""hi".len() + 3"#.to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 5);
}

/// Property result stored in variable: let n = "hello".len; n
#[test]
fn dot_property_result_stored() {
    let program = expected_compile(r#"let n = "hello".len(); n"#.to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 5);
}
