use hail::Executor;

use crate::expected_compile;

/// Test basic iteration over Vec
#[test]
fn for_loop_basic() {
    let program = expected_compile(
        "let arr = [10, 20, 30]; let mut sum = 0; for x in arr { sum += x; } sum".to_owned(),
    );

    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");

    assert_eq!(result, 60); // 10 + 20 + 30
}

/// If empty, it shouldn't iterate
#[test]
fn for_loop_empty() {
    let program = expected_compile(
        "let arr = Vec<i64>(); let mut count = 0; for x in arr { count += 1; } count".to_owned(),
    );

    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");

    assert_eq!(result, 0); // Loop body never executes
}

/// For loop with mutable variable
#[test]
fn for_loop_mutable() {
    let program = expected_compile(
        "let arr = [1, 2, 3]; let mut sum = 0; for mut x in arr { x *= 2; sum += x; } sum"
            .to_owned(),
    );

    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");

    assert_eq!(result, 12); // (1+2+3) * 2
}

/// Nested for loop
#[test]
fn for_loop_nested_loops() {
    let program = expected_compile(
        "let outer = [1, 2]; let inner = [10, 20]; let mut sum = 0; for a in outer { for b in inner { sum += a + b; } } sum"
        .to_owned(),
    );

    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");

    // (1+10) + (1+20) + (2+10) + (2+20) = 11 + 21 + 12 + 22 = 66
    assert_eq!(result, 66);
}

/// For loop modify external
#[test]
fn for_loop_modify_external() {
    let program = expected_compile(
        "let arr = [1, 2, 3, 4, 5]; let mut found = false; for x in arr { if x == 3 { found = true; } } found"
            .to_owned(),
    );

    let executor = Executor::default();
    let result = executor
        .run_with_return::<bool>(&program)
        .expect("Should execute");

    assert_eq!(result, true);
}

/// Test for loop over exclusive range (0..4 -> 0,1,2,3)
#[test]
fn for_loop_exclusive_range() {
    let program = expected_compile("let mut sum = 0; for i in 0..4 { sum += i; } sum".to_owned());

    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");

    assert_eq!(result, 6); // 0+1+2+3
}

/// Test for loop over inclusive range (0..=4 -> 0,1,2,3,4)
#[test]
fn for_loop_inclusive_range() {
    let program = expected_compile("let mut sum = 0; for i in 0..=4 { sum += i; } sum".to_owned());

    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");

    assert_eq!(result, 10); // 0+1+2+3+4
}
