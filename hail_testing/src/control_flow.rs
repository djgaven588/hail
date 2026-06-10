use hail::Executor;

use crate::expected_compile;

/// Basic infinite loop with break condition
#[test]
fn basic_loop_with_break() {
    let program = expected_compile(
        "let mut i = 0; loop { i += 1; if i == 5 { break; } else { continue; } } i".to_owned(),
    );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 5);
}

/// Loop with continue skipping iterations
#[test]
fn loop_with_continue() {
    let program = expected_compile(
        "let mut i = 0; let mut sum = 0; loop { i += 1; if i % 2 == 0 { continue; } else { sum += i; if i >= 9 { break; } else { continue; } } } sum"
            .to_owned(),
    );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 25); // 1+3+5+7+9
}

/// Break in nested loops exits inner loop only
#[test]
fn nested_loop_break() {
    let program = expected_compile(
	"let mut i = 0; let mut j = 0; loop { i += 1; j = 0; loop { j += 1; if j == 3 { break; } else { continue; } } if i >= 2 { break; } else { continue; } } i * 10 + j".to_owned(),
	);

    let executor = Executor::default();

    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");

    assert_eq!(result, 23);
}

/// Count down and break early (non infinite loop fail case)
#[test]
fn break_statement() {
    let program = expected_compile(
        "let mut a = 5; while a > 0 { if a == 3 { break; } a -= 1; } a".to_owned(),
    );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 3);
}

/// Continue counting down then exit (non infinite loop fail case)
#[test]
fn continue_statement() {
    let program = expected_compile(
        "let mut a = 12; while a > 0 { a -= 1; if a > 7 { continue; } else { break; } } a"
            .to_owned(),
    );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 7);
}

/// If else with early return in else branch. Shouldn't have type mismatch.
/// The if branch produces i64, the else branch exits, expression type is i64.
#[test]
fn if_else_early_return_type_merge() {
    let program = expected_compile(
        "let x = true; let result = if x { 42 } else { print(\"missing\"); return; }; result"
            .to_owned(),
    );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 42);
}

/// If else with early return in if branch. Shouldn't have type mismatch.
/// The else branch produces i64, the if branch exits, expression type is i64.
#[test]
fn if_else_early_return_type_merge_reverse() {
    let program = expected_compile(
        "let x = false; let result = if x { print(\"found\"); return; } else { 99 }; result"
            .to_owned(),
    );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 99);
}

/// If else with early return in both branches. Expression type is none.
/// Both branches exit so the if else produces no value, trailing statement handles result.
#[test]
fn if_else_early_return_both_branches() {
    let program = expected_compile(
        "let x = false; if x { print(\"found\"); return 0; } else { print(\"missing\"); return 0; }; 42"
            .to_owned(),
    );
    let executor = Executor::default();
    // Both branches return, so the function exits early with NoReturnValue.
    // The trailing 42 is never reached.
    let result = executor.run_with_return::<i64>(&program);
    assert!(result.is_err());
}

/// If else with early return in else branch. Expression value from if.
#[test]
fn if_else_early_return_expression_value() {
    let program = expected_compile(
        "let x = true; let result = if x { 100 } else { print(\"missing\"); return; }; result"
            .to_owned(),
    );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 100);
}

/// Nested else if chain with return in one branch should not produce type mismatch.
/// Each else if branch can have a return; without causing String vs none mismatches.
#[test]
fn nested_else_if_with_return_branch() {
    let program = expected_compile(
        "let state = 0; let result; if state == 0 { result = \"zero\"; } else if state == 1 { result = \"one\"; } else if state == 2 { return; } else { result = \"other\"; }; result"
            .to_owned(),
    );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<String>(&program)
        .expect("Should execute");
    assert_eq!(result, "zero".to_string());
}

/// Nested else if chain with return in middle branch, testing the true path.
#[test]
fn nested_else_if_with_return_branch_true_path() {
    let program = expected_compile(
        "let state = 1; let mut result = \"\"; if state == 0 { result = \"zero\"; } else if state == 1 { result = \"one\"; } else if state == 2 { return; } else { result = \"other\"; }; result"
            .to_owned(),
    );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<String>(&program)
        .expect("Should execute");
    assert_eq!(result, "one".to_string());
}

/// If else where one branch has a block with return in the middle (partial exit).
#[test]
fn if_else_block_partial_exit() {
    let program = expected_compile(
        "let x = true; let mut result = \"\"; if x { result = \"yes\"; } else { print(\"no\"); return; }; result"
            .to_owned(),
    );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<String>(&program)
        .expect("Should execute");
    assert_eq!(result, "yes".to_string());
}

/// If else where both branches produce String but one also has a return guard.
#[test]
fn if_else_both_strings_one_with_guard() {
    let program = expected_compile(
        "let x = true; let mut result = \"\"; if x { result = \"yes\"; } else if false { result = \"maybe\"; } else { print(\"no\"); return; }; result"
            .to_owned(),
    );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<String>(&program)
        .expect("Should execute");
    assert_eq!(result, "yes".to_string());
}
