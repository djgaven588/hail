use hail::Executor;

use crate::{expected_compile, try_compile};

/// User defined function: simple identity style call
#[test]
fn script_func_simple_call() {
    let program = expected_compile("fn double(x: i64) -> i64 { x + x } double(7)".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 14);
}

/// User defined function with no return type (void body value ignored)
#[test]
fn script_func_no_return_type() {
    // Infer return type of function
    let program = expected_compile("fn say_hi(x: i64) -> i64 { x + 1 } say_hi(5); 99".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 99);
}

/// User defined function: multiple parameters
#[test]
fn script_func_multiple_params() {
    let program = expected_compile(
        "fn add3(a: i64, b: i64, c: i64) -> i64 { a + b + c } add3(1, 2, 3)".to_owned(),
    );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 6);
}

/// User defined function: result used in arithmetic
#[test]
fn script_func_result_in_expr() {
    let program = expected_compile("fn inc(n: i64) -> i64 { n + 1 } inc(10) + inc(20)".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 32);
}

/// User defined function: result stored in variable
#[test]
fn script_func_result_stored() {
    let program =
        expected_compile("fn square(n: i64) -> i64 { n * n } let x = square(6); x".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 36);
}

/// User defined function: chained calls
#[test]
fn script_func_chained_calls() {
    let program = expected_compile("fn inc(n: i64) -> i64 { n + 1 } inc(inc(inc(0)))".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 3);
}

/// User defined function: one function calls another (forward reference)
#[test]
fn script_func_calls_other_func() {
    let program = expected_compile(
        "fn double(n: i64) -> i64 { n + n } fn quad(n: i64) -> i64 { double(double(n)) } quad(3)"
            .to_owned(),
    );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 12);
}

/// User defined function with a String parameter and return
#[test]
fn script_func_string_param_return() {
    let program = expected_compile(
        r#"fn greet(name: String) -> String { "Hello, " + name } greet("World")"#.to_owned(),
    );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<String>(&program)
        .expect("Should execute");
    assert_eq!(result, "Hello, World".to_string());
}

/// User defined function: zero parameters
#[test]
fn script_func_zero_params() {
    let program = expected_compile("fn answer() -> i64 { 42 } answer()".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 42);
}

/// User defined function: local variable declared inside body
#[test]
fn script_func_local_variable() {
    let program = expected_compile(
        "fn compute(x: i64) -> i64 { let mut tmp = x * 2; tmp + 1 } compute(5)".to_owned(),
    );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 11);
}

/// Function used as a first class value stored in a variable
#[test]
fn func_value_stored_and_called() {
    let program =
        expected_compile("fn double(n: i64) -> i64 { n + n } let f = double; f(5)".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 10);
}

/// Function value: zero param function stored and called
#[test]
fn func_value_zero_params_stored_and_called() {
    let program = expected_compile("fn answer() -> i64 { 42 } let f = answer; f()".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 42);
}

/// Function value: result used in expression
#[test]
fn func_value_result_in_expr() {
    let program =
        expected_compile("fn inc(n: i64) -> i64 { n + 1 } let f = inc; f(9) + f(1)".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 12);
}

/// Multiple user defined functions
#[test]
fn script_func_multiple_functions_ordering() {
    let program = expected_compile(
        "fn first(n: i64) -> i64 { n + 1 }\n\
              fn second(n: i64) -> i64 { n * 2 }\n\
              fn third(n: i64) -> i64 { n - 3 }\n\
              first(10) + second(5) + third(8)"
            .to_owned(),
    );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    // first(10)=11, second(5)=10, third(8)=5 => 11+10+5=26
    assert_eq!(result, 26);
}

/// Three functions calling each other in a chain (A->B->C), all defined before calls
#[test]
fn script_func_chained_three_way() {
    let program = expected_compile(
        "fn step_a(n: i64) -> i64 { n + 1 }\n\
              fn step_b(n: i64) -> i64 { step_a(n) * 2 }\n\
              fn step_c(n: i64) -> i64 { step_b(n) - 3 }\n\
              step_c(5)"
            .to_owned(),
    );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    // step_a(5)=6, step_b(5)=12, step_c(5)=9
    assert_eq!(result, 9);
}

/// Multiple functions with different parameter counts and return types (String vs i64)
#[test]
fn script_func_mixed_types() {
    let program = expected_compile(
        "fn add(a: i64, b: i64) -> i64 { a + b }\n\
              fn greet(name: String) -> String { \"Hello, \" + name }\n\
              add(3, 4)"
            .to_owned(),
    );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 7);
}

/// Function stored as value and called later (tests LoadFunction + CallScript ordering)
#[test]
fn script_func_stored_and_called_later() {
    let program = expected_compile(
        "fn double(n: i64) -> i64 { n * 2 }\n\
              fn triple(n: i64) -> i64 { n * 3 }\n\
              let f = double;\n\
              let g = triple;\n\
              f(5) + g(7)"
            .to_owned(),
    );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    // double(5)=10, triple(7)=21 => 10+21=31
    assert_eq!(result, 31);
}

/// Deeply nested function calls with multiple functions in the program
#[test]
fn script_func_deep_nesting() {
    let program = expected_compile(
        "fn inc(n: i64) -> i64 { n + 1 }\n\
              fn double(n: i64) -> i64 { n * 2 }\n\
              fn triple(n: i64) -> i64 { n * 3 }\n\
              inc(double(triple(inc(0))))"
            .to_owned(),
    );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    // inc(0)=1, triple(1)=3, double(3)=6, inc(6)=7
    assert_eq!(result, 7);
}

/// Four functions defined in order, called in reverse
#[test]
fn script_func_reverse_call_order() {
    let program = expected_compile(
        "fn func_a(n: i64) -> i64 { n + 10 }\n\
              fn func_b(n: i64) -> i64 { n * 2 }\n\
              fn func_c(n: i64) -> i64 { n - 5 }\n\
              fn func_d(n: i64) -> i64 { n + n }\n\
              func_d(func_c(func_b(func_a(0))))"
            .to_owned(),
    );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    // func_a(0)=10, func_b(10)=20, func_c(20)=15, func_d(15)=30
    assert_eq!(result, 30);
}

/// Multiple functions with bodies that reference earlier defined functions by index
#[test]
fn script_func_index_resolution() {
    let program = expected_compile(
        "fn add_one(n: i64) -> i64 { n + 1 }\n\
              fn double(n: i64) -> i64 { n * 2 }\n\
              fn apply_add_then_double(n: i64) -> i64 { double(add_one(n)) }\n\
              apply_add_then_double(5)"
            .to_owned(),
    );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    // add_one(5)=6, double(6)=12 => 12
    assert_eq!(result, 12);
}

/// Bare return inside an if block should fail a return typing check
#[test]
fn script_func_bare_return_in_if_block() {
    let result = try_compile(
        "fn test() -> i64 {
            let mut my_condition = true;
            if my_condition {
                return;
            }

            5
        }
        test()"
            .to_owned(),
    );
    assert!(
        result.is_err(),
        "Bare `return;` in a function with explicit return type should fail compilation"
    );
}
