use crate::expected_compile;
use hail::Executor;

/// New None
#[test]
fn option_i64_none() {
    let program = expected_compile("Option<i64>()".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<Option<i64>>(&program)
        .expect("Should execute");
    assert!(result.is_none());
}

/// New Some(val)
#[test]
fn option_i64_some() {
    let program = expected_compile("Some(42)".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<Option<i64>>(&program)
        .expect("Should execute");
    assert!(result.is_some());
    assert_eq!(result.unwrap(), 42);
}

/// Make sure Some only has is_some, not is_none
#[test]
fn option_i64_is_methods_some() {
    let executor = Executor::default();
    assert!(
        executor
            .run_with_return::<bool>(&expected_compile("Some(1).is_some()".to_owned()))
            .expect("Should execute")
    );
    let executor = Executor::default();
    assert!(
        !executor
            .run_with_return::<bool>(&expected_compile("Some(1).is_none()".to_owned()))
            .expect("Should execute")
    );
}

/// Make sure None only has is_none, not is_some
#[test]
fn option_i64_is_methods_none() {
    let executor = Executor::default();
    assert!(
        !executor
            .run_with_return::<bool>(&expected_compile("Option<i64>().is_some()".to_owned()))
            .expect("Should execute")
    );
    let executor = Executor::default();
    assert!(
        executor
            .run_with_return::<bool>(&expected_compile("Option<i64>().is_none()".to_owned()))
            .expect("Should execute")
    );
}

/// Unwrap success condition
#[test]
fn option_i64_unwrap_some() {
    let program = expected_compile("Some(99).unwrap()".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 99);
}

/// Unwrap fail condition
#[test]
fn option_i64_unwrap_none_fails() {
    let program = expected_compile("Option<i64>().unwrap()".to_owned());
    let executor = Executor::default();
    assert!(executor.run_with_return::<i64>(&program).is_err());
}

/// Expect success condition
#[test]
fn option_i64_expect_some() {
    let program = expected_compile(r#"Some(50).expect("ignored")"#.to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 50);
}

/// Expect failure condition
#[test]
fn option_i64_expect_none_fails() {
    let program = expected_compile(r#"Option<i64>().expect("my custom error")"#.to_owned());
    let executor = Executor::default();
    assert!(executor.run_with_return::<i64>(&program).is_err());
}

/// Option used in conditional (Some branch taken)
#[test]
fn option_i64_in_if_some() {
    let program = expected_compile(
        "let opt = Some(1); if opt.is_some() { opt.unwrap() } else { 0 }".to_string(),
    );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 1);
}

/// Option used in conditional (None branch taken)
#[test]
fn option_i64_in_if_none() {
    let program = expected_compile(
        "let opt = Option<i64>(); if opt.is_none() { 0 } else { opt.unwrap() }".to_string(),
    );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 0);
}
