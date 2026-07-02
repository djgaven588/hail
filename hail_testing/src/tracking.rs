use hail::{
    ExecutorError,
    executor::{Executor, ExecutorErrorKind},
};

use crate::expected_compile;

/// Stable case to ensure tracking is "ok"
#[test]
fn basic() {
    let program = expected_compile("let x = 42; x + 1".to_owned());
    let executor = Executor::tracked(5, 32);
    assert_eq!(executor.run_with_return::<i64>(&program), Ok(43));
}

#[test]
fn instruction_limit_exceeded() {
    let program = expected_compile("let mut i = 0; while i < 128 { i += 1; }".to_owned());

    // Takes roughly 1028 instructions, with 3 i64 in memory at max
    let executor = Executor::tracked(512, std::mem::size_of::<i64>() * 3);
    assert_eq!(
        executor.run(&program),
        Err(ExecutorError::new_with_program(
            ExecutorErrorKind::InstructionLimit(512),
            &program,
            // FIXME / NOTE: This may be fragile in the event of compiler optimizations
            8
        ))
    );
}

#[test]
fn memory_limit_exceeded() {
    // Make a big string
    let program = expected_compile(format!("let s = \"{}\"; s", "x".repeat(128)));

    let executor = Executor::tracked(3, 256);
    assert_eq!(
        executor.run(&program),
        Err(ExecutorError::new_with_program(
            ExecutorErrorKind::OutOfMemory(136, 136, 256),
            &program,
            // FIXME / NOTE: This may be fragile in the event of compiler optimizations
            2
        ))
    );
}

/// Expressions need to release memory
#[test]
fn memory_released() {
    let program = expected_compile(
        "let z = { let x = 1; let y = 2; x + y + 7 + 11 + 19 }; let w = { z + 12 + 13 }; z + w"
            .to_owned(),
    );
    let executor = Executor::tracked(32, 32);

    assert_eq!(executor.run_with_return::<i64>(&program), Ok(105));
}

/// Range iterator should not allocate a vector for large ranges
#[test]
fn range_iterator_large_range() {
    // Sum of 0..500 is 500*499/2 = 124750
    let program = expected_compile("let mut sum = 0; for i in 0..500 { sum += i; } sum".to_owned());

    // A simple iterator like this shouldn't use more than 57 bytes of memory:
    // 8: "Sum"
    // +16: 0 + 500 on stack
    // -+16: Stack to iterator
    // +9 (+16 temp): Option "i" from iterator
    // +8 (+9 temp): Unwrap "i"
    
    // Instruction limit of 8192 is "reasonable"
    let executor = Executor::tracked(8192, 57);
    let result = executor.run_with_return::<i64>(&program);

    assert_eq!(result, Ok(124750));
}
