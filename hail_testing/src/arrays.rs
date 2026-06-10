use crate::try_compile;
use hail::Executor;

use crate::expected_compile;

/// Simple integer array
#[test]
fn int_array_literal() {
    let program = expected_compile("[1, 2, 3]".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<Vec<i64>>(&program)
        .expect("Should execute");
    assert_eq!(result.len(), 3);
    assert_eq!(result[0], 1);
    assert_eq!(result[1], 2);
    assert_eq!(result[2], 3);
}

/// Single element
#[test]
fn single_element_array() {
    let program = expected_compile("[42]".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<Vec<i64>>(&program)
        .expect("Should execute");
    assert_eq!(result.len(), 1);
    assert_eq!(result[0], 42);
}

/// Array stored in variable and used later
#[test]
fn array_in_variable() {
    let program = expected_compile("let arr = [1, 2, 3]; arr".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<Vec<i64>>(&program)
        .expect("Should execute");
    assert_eq!(result.len(), 3);
    assert_eq!(result[0], 1);
    assert_eq!(result[1], 2);
    assert_eq!(result[2], 3);
}

/// Array used in sum
#[test]
fn array_used_in_expression() {
    let program = expected_compile("let arr = [1, 2, 3]; arr[0] + arr[1] + arr[2]".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 6);
}

/// Empty array should fail to compile (ArrayTypeUnclear)
#[test]
fn empty_array_fails() {
    let result = try_compile("[]".to_owned());
    assert!(result.is_err(), "Empty array should fail compilation");
}

/// Mixed types in array should fail (ArrayTypeMismatch)
#[test]
fn mixed_types_in_array_fail() {
    let result = try_compile("[1, 2.0, 3]".to_owned());
    assert!(
        result.is_err(),
        "Mixed int and float in array should fail compilation"
    );
}

/// Array used in if condition (boolean array element access)
#[test]
fn array_in_if_condition() {
    let program =
        expected_compile("let arr = [true, false]; if arr[0] { 1 } else { 2 }".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 1);
}

/// Array with many elements (stress test for push operations)
#[test]
fn array_with_many_elements() {
    let program = expected_compile("[0, 1, 2, 3, 4, 5, 6, 7, 8, 9]".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<Vec<i64>>(&program)
        .expect("Should execute");
    assert_eq!(result.len(), 10);
    for i in 0..10 {
        assert_eq!(result[i], i as i64);
    }
}

/// Array with variable references as elements
#[test]
fn array_with_variable_references() {
    let program = expected_compile("let a = 10; let b = 20; let c = 30; [a, b, c]".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<Vec<i64>>(&program)
        .expect("Should execute");
    assert_eq!(result.len(), 3);
    assert_eq!(result[0], 10);
    assert_eq!(result[1], 20);
    assert_eq!(result[2], 30);
}

/// Array with function that returns array
#[test]
fn array_returned_from_function() {
    let program = expected_compile(
        "fn make_array() -> Vec<i64> { [1, 2, 3] }
           make_array()[0]"
            .to_owned(),
    );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 1);
}

/// Array with negative indice
#[test]
fn array_negative_indice() {
    let program = expected_compile(
        "let arr = [1, 2, 3];
           arr[-1]"
            .to_owned(),
    );

    let executor = Executor::default();
    // This should fail
    if let Ok(_) = executor.run_with_return::<i64>(&program) {
        panic!("Returned when it should have failed!")
    }
}

/// Array with function that takes array parameter
#[test]
fn array_passed_to_function() {
    let program = expected_compile(
        "fn first(arr: Vec<i64>) -> i64 { arr[0] }
           first([99, 88, 77])"
            .to_owned(),
    );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 99);
}
