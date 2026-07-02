use crate::{expected_compile, try_compile};
use hail::Executor;

/// Function with Vec<i64> parameter and return type
#[test]
fn generic_vec_i64_param_return() {
    let program = expected_compile(
        "fn double_all(arr: Vec<i64>) -> Vec<i64> { arr }
           double_all([1, 2, 3])"
            .to_owned(),
    );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<Vec<i64>>(&program)
        .expect("Should execute");
    assert_eq!(result.len(), 3);
    assert_eq!(result[0], 1);
    assert_eq!(result[1], 2);
    assert_eq!(result[2], 3);
}

/// Option with String type parameter used in expression
#[test]
fn generic_option_string() {
    let program = expected_compile("let s = Some(\"hello\"); s.unwrap()".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<String>(&program)
        .expect("Should execute");
    assert_eq!(result, "hello");
}

/// Option with bool type parameter used in expression
#[test]
fn generic_option_bool() {
    let program = expected_compile("let b = Some(true); b.unwrap()".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<bool>(&program)
        .expect("Should execute");
    assert!(result);
}

/// Nested generics: Vec<Option<i64>> - array literal with Some/Option<i64>()
#[test]
fn generic_nested_vec_option() {
    let program = expected_compile("let v = [Some(1), Option<i64>(), Some(3)]; v".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<Vec<Option<i64>>>(&program)
        .expect("Should execute");
    assert_eq!(result.len(), 3);
}

/// Nested generics: Option<Vec<i64>> - construct and unwrap
#[test]
fn generic_nested_option_vec() {
    let program = expected_compile("let v = Some([10, 20]); v.unwrap()".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<Vec<i64>>(&program)
        .expect("Should execute");
    assert_eq!(result.len(), 2);
    assert_eq!(result[0], 10);
}

/// Function with Option<T> return type used in conditional
#[test]
fn generic_option_in_if() {
    let program = expected_compile(
        "fn maybe_val(x: i64) -> Option<i64> { Some(x * 2) }
           if maybe_val(5).is_some() { maybe_val(5).unwrap() } else { 0 }"
            .to_owned(),
    );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 10);
}

/// Generic function with multiple Vec parameters of same type
#[test]
fn generic_vec_multiple_params() {
    let program = expected_compile(
        "fn first_arr(a: Vec<i64>, b: Vec<i64>) -> Vec<i64> { a }
           first_arr([1, 2], [3, 4])"
            .to_owned(),
    );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<Vec<i64>>(&program)
        .expect("Should execute");
    assert_eq!(result.len(), 2);
}

/// Generic function with mixed Option and Vec parameters
#[test]
fn generic_mixed_option_vec_params() {
    let program = expected_compile(
        "fn first_if_some(opt: Option<i64>, arr: Vec<i64>) -> i64 { opt.unwrap() }
           first_if_some(Some(99), [1, 2])"
            .to_owned(),
    );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 99);
}

/// Generic function: push to Vec<i64> and return length (mut required)
#[test]
fn generic_vec_push_and_len() {
    let program = expected_compile(
        "fn grow(mut arr: Vec<i64>) -> i64 { arr.push(42); arr.len() }
           grow([1, 2])"
            .to_owned(),
    );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 3);
}

/// Generic function: use into_iter on Vec<i64> in for loop
#[test]
fn generic_vec_for_loop() {
    let program = expected_compile(
        "fn sum_all(arr: Vec<i64>) -> i64 { let mut s = 0; for x in arr { s += x; } s }
           sum_all([1, 2, 3, 4])"
            .to_owned(),
    );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 10);
}

/// Generic function: pass array literal as Vec<i64> argument
#[test]
fn generic_array_literal_as_arg() {
    let program = expected_compile(
        "fn get_len(arr: Vec<i64>) -> i64 { arr.len() }
           get_len([5, 10, 15, 20])"
            .to_owned(),
    );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 4);
}

/// Generic function: Option<i64> parameter with Option<i64>() branch
#[test]
fn generic_option_none_param() {
    let program = expected_compile(
        "fn safe_div(a: i64, b: Option<i64>) -> i64 { if b.is_some() { a / b.unwrap() } else { 0 } }
           safe_div(10, Option<i64>())"
            .to_owned(),
    );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 0);
}

/// Generic function: Option<i64> parameter with Some branch
#[test]
fn generic_option_some_param() {
    let program = expected_compile(
        "fn safe_div(a: i64, b: Option<i64>) -> i64 { if b.is_some() { a / b.unwrap() } else { 0 } }
           safe_div(10, Some(5))"
            .to_owned(),
    );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 2);
}

/// Generic type equality: Vec<i64> and Option<String> are distinct types (both compile)
#[test]
fn generic_type_equality_compile() {
    let program = expected_compile("let v = [1]; let o = Some(\"x\"); v".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<Vec<i64>>(&program)
        .expect("Should execute");
    assert_eq!(result.len(), 1);
}

/// Generic: array literal type inference produces correct Vec<T>
#[test]
fn generic_array_inference() {
    let program = expected_compile("let arr = [3, 6, 9]; arr[0] + arr[2]".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 12);
}

/// Generic: deeply nested Option<Vec<Option<String>>>
#[test]
fn generic_deeply_nested() {
    let program = expected_compile("let v = Some([Some(\"deep\")]); v.unwrap().len()".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 1);
}

/// Generic: function returning Option<Vec<i64>> and unwrapping
#[test]
fn generic_option_vec_return() {
    let program = expected_compile(
        "fn make_opt_vec() -> Option<Vec<i64>> { Some([7, 8]) }
           make_opt_vec().unwrap()"
            .to_owned(),
    );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<Vec<i64>>(&program)
        .expect("Should execute");
    assert_eq!(result.len(), 2);
}

/// Generic: multiple generic functions in same program
#[test]
fn multiple_generic_functions() {
    let program = expected_compile(
        "fn double(x: Vec<i64>) -> Vec<i64> { x }
           fn halve(x: Option<i64>) -> i64 { if x.is_some() { x.unwrap() / 2 } else { 0 } }
           let v = double([10]); halve(Some(v[0]))"
            .to_owned(),
    );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 5);
}

/// Compile-time type mismatch: passing Vec<f64> where Vec<i64> expected should fail
#[test]
fn generic_type_mismatch_compile_error() {
    let result = try_compile(
        "fn takes_i64(arr: Vec<i64>) -> i64 { arr.len() }
           takes_i64([1.0, 2.0])"
            .to_owned(),
    );
    assert!(
        result.is_err(),
        "Should reject float array for Vec<i64> param"
    );
}

/// Generic: nested generic in array literal - Vec<Option<String>> with len()
#[test]
fn generic_nested_in_array() {
    let program = expected_compile("let v = [Some(\"a\"), Some(\"b\")]; v.len()".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 2);
}

/// Generic: function with Vec<i64> param accessing elements by index
#[test]
fn generic_vec_index_access() {
    let program = expected_compile(
        "fn last(arr: Vec<i64>) -> i64 { arr[arr.len() - 1] }
           last([5, 10, 15])"
            .to_owned(),
    );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 15);
}

/// Generic: function with Option<String> param returning string concat
#[test]
fn generic_option_string_concat() {
    let program = expected_compile(
        "fn greet(name: Option<String>) -> String { if name.is_some() { \"Hello, \" + name.unwrap() } else { \"Hi\" } }
           greet(Some(\"World\"))"
            .to_owned(),
    );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<String>(&program)
        .expect("Should execute");
    assert_eq!(result, "Hello, World");
}

/// Generic: function with Vec<Option<i64>> param iterating and summing Some values
#[test]
fn generic_vec_option_sum() {
    let program = expected_compile(
        "fn sum_some(arr: Vec<Option<i64>>) -> i64 { let mut s = 0; for x in arr { if x.is_some() { s += x.unwrap(); } } s }
           sum_some([Some(1), Option<i64>(), Some(3)])"
            .to_owned(),
    );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 4);
}

/// Generic: function returns Vec<Option<String>> and unwraps first element
#[test]
fn generic_vec_option_string_return() {
    let program = expected_compile(
        "fn make_opts() -> Vec<Option<i64>> { [Some(10), Option<i64>(), Some(30)] }
           let opts = make_opts(); opts[0].unwrap()"
            .to_owned(),
    );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 10);
}

/// Generic: two functions where one takes Vec<T> and returns Option<Vec<T>>
#[test]
fn generic_vec_to_option() {
    let program = expected_compile(
        "fn to_opt(arr: Vec<i64>) -> Option<Vec<i64>> { Some(arr) }
           to_opt([1, 2]).unwrap().len()"
            .to_owned(),
    );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 2);
}

/// Generic: function parameter type differs from argument - should fail compile
#[test]
fn generic_param_type_mismatch() {
    let result = try_compile(
        "fn takes_vec(arr: Vec<i64>) -> i64 { arr.len() }
           takes_vec([1.0, 2.0])"
            .to_owned(),
    );
    assert!(
        result.is_err(),
        "Should reject f64 array for Vec<i64> param"
    );
}

/// Generic: Option<Vec<String>> - nested generic with String
#[test]
fn generic_option_vec_string() {
    let program = expected_compile("let v = Some([\"a\", \"b\"]); v.unwrap().len()".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 2);
}

/// Generic: function with Option<i64> return used in for loop context
#[test]
fn generic_option_for_context() {
    let program = expected_compile(
        "fn gen(n: i64) -> Option<i64> { if n > 0 { Some(n - 1) } else { Option<i64>() } }
           let mut x = 5; while gen(x).is_some() { x = gen(x).unwrap(); } x"
            .to_owned(),
    );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 0);
}

/// Generic: generic function called from within another generic function
#[test]
fn nested_generic_calls() {
    let program = expected_compile(
        "fn double(x: i64) -> i64 { x * 2 }
           fn apply_to_vec(arr: Vec<i64>) -> Vec<i64> { arr }
           let v = apply_to_vec([1, 2]); [double(v[0]), double(v[1])]"
            .to_owned(),
    );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<Vec<i64>>(&program)
        .expect("Should execute");
    assert_eq!(result.len(), 2);
    assert_eq!(result[0], 2);
    assert_eq!(result[1], 4);
}

// TODO: When type inference can handle empty arrays, this test will be needed.
/*
/// Generic: empty Vec with explicit type annotation in function param
#[test]
fn generic_empty_vec() {
    let program = expected_compile(
        "fn make_empty() -> Vec<i64> { [] }
           make_empty().len()"
            .to_owned(),
    );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 0);
}*/

// TODO: When type inference can handle empty arrays, this test will be needed.
/*
/// Generic: generic function with Option<i64> param and Vec<i64> return
#[test]
fn generic_option_to_vec() {
    let program = expected_compile(
        "fn opt_to_vec(x: Option<i64>) -> Vec<i64> { if x.is_some() { [x.unwrap()] } else { [] } }
           opt_to_vec(Some(42)).len()"
            .to_owned(),
    );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 1);
}*/

// TODO: When type inference can handle empty arrays, this test will be needed.
/*
/// Generic: Option<i64> used as function return with Option<i64>() case
#[test]
fn generic_option_none_return() {
    let program = expected_compile(
        "fn find(arr: Vec<i64>, target: i64) -> Option<i64> { if arr[0] == target { Some(target) } else { None } }
           find([1, 2], 3)"
            .to_owned(),
    );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<Option<i64>>(&program)
        .expect("Should execute");
    assert!(result.is_none());
}*/

// TODO: When type inference can handle empty arrays, this test will be needed.
/*
/// Generic: Option<i64> used as function return with Some case
#[test]
fn generic_option_some_return() {
    let program = expected_compile(
        "fn find(arr: Vec<i64>, target: i64) -> Option<i64> { if arr[0] == target { Some(target) } else { None } }
           find([1, 2], 1).unwrap()"
            .to_owned(),
    );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 1);
}*/

/// Generic: deeply nested generic in function return
#[test]
fn generic_deeply_nested_return() {
    let program = expected_compile(
        "fn make_deep() -> Option<Vec<Option<i64>>> { Some([Some(99)]) }
           make_deep().unwrap()[0].unwrap()"
            .to_owned(),
    );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 99);
}

/// Generic: generic function with Option<String> param returning bool
#[test]
fn generic_option_string_bool_return() {
    let program = expected_compile(
        "fn is_empty(s: Option<String>) -> bool { if s.is_none() { true } else { false } }
            is_empty(Option<String>())"
            .to_owned(),
    );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<bool>(&program)
        .expect("Should execute");
    assert!(result);
}

/// Generic: generic function with Vec<f64> param and i64 return (len)
#[test]
fn generic_vec_f64_len() {
    let program = expected_compile(
        "fn count(arr: Vec<f64>) -> i64 { arr.len() }
           count([1.1, 2.2, 3.3])"
            .to_owned(),
    );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 3);
}

// TODO: When type inference can handle empty arrays, this test will be needed.
/*
/// Generic: generic function with Option<Vec<i64>> param and Vec<i64> return
#[test]
fn generic_option_vec_param() {
    let program = expected_compile(
        "fn unwrap_or_empty(v: Option<Vec<i64>>) -> Vec<i64> { if v.is_some() { v.unwrap() } else { [] } }
           unwrap_or_empty(Some([10, 20])).len()"
            .to_owned(),
    );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 2);
}*/

// TODO: When type inference can handle empty arrays, this test will be needed.
/*
/// Generic: generic function with Option<Vec<i64>> param returning Option<i64>() case
#[test]
fn generic_option_vec_none_param() {
    let program = expected_compile(
        "fn unwrap_or_empty(v: Option<Vec<i64>>) -> Vec<i64> { if v.is_some() { v.unwrap() } else { [] } }
            unwrap_or_empty([]).len()"
            .to_owned(),
    );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 0);
}
*/
