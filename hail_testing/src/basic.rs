use hail::Token;
use hail::TokenType;
use std::sync::Arc;

use crate::try_compile;
use hail::{
    BinaryOp, Constructor, Executor, Expr, Instructor, Location, MemoryImportResolver, Parser,
    Scanner, Stmt,
};

use crate::expected_compile;

/// Ensure that PEMDAS is followed, as well as the general pipeline functioning as expected.
#[test]
fn basic_math() {
    // This is expected to be:
    // 1 + 4 * (2 - 3) / 2
    // 1 + 4 * (-1) / 2
    // 1 + -4 / 2
    // 1 + -2
    // -1

    // Scan the program for all its tokens
    let mut scanner = Scanner::new("1 + 4 * (2 - 3) / 2".to_owned());
    scanner.scan();

    // We should have no errors
    assert_eq!(scanner.errors(), &[]);

    // Make sure the token count is expected
    assert_eq!(scanner.get().expect("Scanner should have tokens").len(), 12);

    // Parse the scanned tokens
    let mut parser = Parser::new(scanner.get().unwrap());
    parser.parse();

    // We should have no errors
    assert_eq!(parser.errors(), &[]);

    // Make sure the tree is expected
    // Generally this will be too large to care, but simpler cases are useful
    assert_eq!(
        parser.get().expect("Should have stmts"),
        [Stmt::Expression(
            Box::new(Expr::Binary(
                Box::new(Expr::Integer(
                    Location {
                        line: 1,
                        column: 1,
                        length: 1
                    },
                    1
                )),
                BinaryOp::Plus,
                Box::new(Expr::Binary(
                    Box::new(Expr::Binary(
                        Box::new(Expr::Integer(
                            Location {
                                line: 1,
                                column: 5,
                                length: 1
                            },
                            4
                        )),
                        BinaryOp::Multiply,
                        Box::new(Expr::Binary(
                            Box::new(Expr::Integer(
                                Location {
                                    line: 1,
                                    column: 10,
                                    length: 1
                                },
                                2
                            )),
                            BinaryOp::Minus,
                            Box::new(Expr::Integer(
                                Location {
                                    line: 1,
                                    column: 14,
                                    length: 1
                                },
                                3
                            ))
                        ))
                    )),
                    BinaryOp::Divide,
                    Box::new(Expr::Integer(
                        Location {
                            line: 1,
                            column: 19,
                            length: 1
                        },
                        2
                    ))
                ))
            )),
            true
        )]
    );

    let module = Arc::new(hail_std::std().expect("Should register"));
    let resolver = Arc::new(MemoryImportResolver::default());

    // Type checking and such
    let constructor = Constructor::new(module.clone(), resolver.clone(), None);
    let astmts = constructor
        .generate(parser.get().unwrap())
        .expect("Should be able to construct");

    // We should have the stmts needed
    assert_eq!(astmts.stmts.len(), 1);

    // Turn to machine code
    let instructor = Instructor::new(module, resolver, None);
    let program = instructor.generate(astmts, "NOT HERE".to_string(), vec![]);

    // Should execute to valid value
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");

    // Result valid?
    assert_eq!(result, -1);
}

/// Ensure unary ops work
#[test]
fn unary() {
    // Scan the program for all its tokens
    let mut scanner = Scanner::new("-3".to_owned());
    scanner.scan();

    // We should have no errors
    assert_eq!(scanner.errors(), &[]);

    // Make sure the token count is expected
    assert_eq!(
        scanner.get().expect("Scanner should have tokens"),
        &[
            Token {
                location: Location {
                    line: 1,
                    column: 1,
                    length: 1
                },
                token_type: TokenType::Minus,
                lexeme: "-".to_string()
            },
            Token {
                location: Location {
                    line: 1,
                    column: 2,
                    length: 1
                },
                token_type: TokenType::Integer,
                lexeme: "3".to_string()
            },
            Token {
                location: Location {
                    line: 1,
                    column: 3,
                    length: 0
                },
                token_type: TokenType::Eof,
                lexeme: "".to_string()
            }
        ]
    );

    // Parse the scanned tokens
    let mut parser = Parser::new(scanner.get().unwrap());
    parser.parse();

    // We should have no errors
    assert_eq!(parser.errors(), &[]);

    let module = Arc::new(hail_std::std().expect("Should register"));
    let resolver = Arc::new(MemoryImportResolver::default());

    // Type checking and such
    let constructor = Constructor::new(module.clone(), resolver.clone(), None);
    let astmts = constructor
        .generate(parser.get().expect("Should have stmts"))
        .expect("Should be able to construct");

    // We should have the stmts needed
    assert_eq!(astmts.stmts.len(), 1);

    // Turn to machine code
    let instructor = Instructor::new(module, resolver, None);
    let program = instructor.generate(astmts, "NOT HERE".to_string(), vec![]);

    // Should execute to valid value
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");

    // Result valid?
    assert_eq!(result, -3);
}

// Stress many parts of math as a basic "fuzz test"
#[test]
fn math_stressor() {
    // Int cases
    {
        let cases = vec![
            ("-5 / -2", -5 / -2),
            ("8 * -2 + (4 / 2 - 3)", 8 * -2 + (4 / 2 - 3)),
        ];

        for case in cases {
            println!("Case '{}', expect: {}", case.0, case.1);
            let program = expected_compile(case.0.to_string());

            let executor = Executor::default();
            let result = executor
                .run_with_return::<i64>(&program)
                .expect("Should execute");

            println!("Result: {result}");
            assert_eq!(result, case.1);
        }
    }

    // Float cases
    {
        let cases = vec![
            ("2.2 + -1.0", 2.2 + -1.0),
            ("-9.3 / 2.3 * 2.0", -9.3 / 2.3 * 2.0),
            ("3.1 / -9.3 / 2.3 * 2.0 / 1.2", 3.1 / -9.3 / 2.3 * 2.0 / 1.2),
        ];

        for case in cases {
            println!("Case '{}', expect: {}", case.0, case.1);
            let program = expected_compile(case.0.to_string());

            let executor = Executor::default();
            let result = executor
                .run_with_return::<f64>(&program)
                .expect("Should execute");

            println!("Result: {result}");
            assert_eq!(result, case.1);
        }
    }
}

/// Test a while loop which counts down i and counts up r every iteration
/// The end result should be i = 0, and r = 1,000,000 (flipping them)
#[test]
fn while_one_million() {
    // Loop one million times, count iterations, return countdown + countup (= 1000000)
    let program = expected_compile(
        "let mut i = 1_000_000; let mut r = 0; while i > 0 { i -= 1; r += 1; } i + r".to_owned(),
    );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 1_000_000);
}

/// Do basic checks on variable mutability, happy cases
#[test]
fn variable_mutability() {
    // Ensure constants can be defined, uninitialized can be initialized, and mutable can be mutated.
    let program = expected_compile(
        "const X = 1000; let y; y = 10; let mut z; z = 10.0; z += 5.0; z".to_owned(),
    );
    let executor = Executor::default();
    assert_eq!(executor.run_with_return::<f64>(&program), Ok(10.0 + 5.0));
}

/// Test if else with true condition
#[test]
fn if_else_true_branch() {
    let program = expected_compile(
        "let result; if true { result = 10; } else { result = 20; } result".to_owned(),
    );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 10);
}

/// Test if else with false condition
#[test]
fn if_else_false_branch() {
    let program = expected_compile(
        "let result; if false { result = 10; } else { result = 20; } result".to_owned(),
    );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 20);
}

/// Test if else where outer is false, inner should never execute
#[test]
fn nested_if_else_false_outer() {
    let program = expected_compile(
            "let x = 10; let y = 5; let result; if x < y { if x > 0 { result = 1; } else { result = 2; } } else { result = 3; } result".to_owned(),
        );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 3);
}

/// Test while loop with false condition (zero iterations)
#[test]
fn while_zero_iterations() {
    let program =
        expected_compile("let mut counter = 0; while false { counter += 1; } counter".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 0);
}

/// Test block expression with variable declaration
#[test]
fn block_expression() {
    let program = expected_compile("{ let x = 5; x + 3 }".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 8);
}

/// Test empty block returns
#[test]
fn empty_block() {
    let program = expected_compile("{}".to_owned());
    let executor = Executor::default();
    let result = executor.run(&program);
    // Empty block should succeed without return value issues
    assert!(result.is_ok());
}

/// Test multiple sequential statements
#[test]
fn multiple_statements() {
    let program = expected_compile("let a = 1; let b = 2; let c = a + b; c".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 3);
}

/// Discard useless value production
#[test]
fn useless_statements() {
    let program = expected_compile("11; 5; let a = 2; 7; 3; a".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 2);
}

/// Test comparison operators: <, >, <=, >=
#[test]
fn comparison_operators() {
    // Less than
    let program = expected_compile("let a = 5; let b = 10; if a < b { 1 } else { 0 }".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 1);

    // Greater than
    let program = expected_compile("let a = 10; let b = 5; if a > b { 1 } else { 0 }".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 1);

    // Less than or equal
    let program = expected_compile("let a = 5; let b = 5; if a <= b { 1 } else { 0 }".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 1);

    // Greater than or equal
    let program = expected_compile("let a = 5; let b = 5; if a >= b { 1 } else { 0 }".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 1);
}

/// Test string concatenation operations
#[test]
fn string_operations() {
    let program = expected_compile("let a = \"Hello\"; let b = \" World\"; a + b".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<String>(&program)
        .expect("Should execute");
    assert_eq!(result, "Hello World");
}

/// Test deeply nested blocks
#[test]
fn deeply_nested_blocks() {
    let program = expected_compile("{ { { let x = 42; x } } }".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 42);
}

/// Test variable reassignment multiple times
#[test]
fn variable_reassignment() {
    let program = expected_compile("let mut x = 1; x = 2; x = 3; x".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 3);
}

/// Test compound assignment operators
#[test]
fn compound_assignment_operators() {
    let program = expected_compile("let mut x = 10; x += 5; x -= 3; x *= 2; x /= 4; x".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 6); // 10 + 5 = 15, 15 - 3 = 12, 12 * 2 = 24, 24 / 4 = 6
}

/// Test deeply nested while loop with block scoping
#[test]
fn while_with_nested_block_scoping() {
    let program =
        expected_compile("let mut x = 3; while x > 0 { { let y = x; x = y - 1; } } x".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 0);
}

/// Test consecutive if statements (not else if chain)
#[test]
fn consecutive_if_statements() {
    let program =
        expected_compile("let mut x = 0; if true { x += 1; } if true { x += 10; } x".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 11);
}

/// Test boolean NOT logic
#[test]
fn boolean_not() {
    let program = expected_compile("let a = true; if !a { 1 } else { 0 }".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 0);

    let program = expected_compile("let a = false; if !a { 1 } else { 0 }".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 1);
}

/// Test variable shadowing in nested block
#[test]
fn variable_shadowing() {
    let program = expected_compile("let x = 10; { let x = 20; x }".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 20);
}

/// Test variable shadowing with mutable
#[test]
fn variable_shadowing_mutable() {
    let program = expected_compile("let mut x = 10; { let mut x = 20; x += 5; } x".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 10);
}

/// Regression test for variable shadowing with initializer expressions.
/// When a variable is shadowed by another let that uses the old var's value
/// in an if else expression, definite initialization tracking must not break.
/// This was causing chopper.hail to fail with "Variable might be uninitialized".
#[test]
fn variable_shadowing_with_initializer_expr() {
    // Simulates: shadowed by if-else that reads the old var's value
    let program = expected_compile(
        "let x = 10; \n\
         let mut y = if x > 5 { x } else { 0 }; \n\
         y += 3; \n\
         y"
        .to_owned(),
    );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should compile and execute");
    assert_eq!(result, 13);
}

/// Regression: shadowing + nested if else modifications (mimics chopper.hail on_tick)
#[test]
fn variable_shadowing_nested_if_else() {
    // - Shadow with Option unwrap via if else
    // - Modify inside nested if else blocks
    // - Use after all blocks
    let program = expected_compile(
        "let mut progress = 0; \n\
         if true { \n\
             if progress > 0 { \n\
                 progress -= 1; \n\
             } else { \n\
                 progress += 1; \n\
             } \n\
         } \n\
         if true { \n\
             let mut was_running = false; \n\
             if true { \n\
                 progress += 1; \n\
                 was_running = true; \n\
             } else { \n\
                 progress = 0; \n\
             } \n\
             progress += 5; \n\
         } \n\
         progress"
            .to_owned(),
    );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should compile and execute");
    assert_eq!(result, 7);
}

/// Test block expression returning a value that is used in subsequent operations
#[test]
fn block_expression_value_used() {
    let program = expected_compile("{ let x = 5; let y = 10; x + y }".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 15);
}

/// Test block expression with nested control flow followed by identifier
#[test]
fn block_expression_with_control_flow() {
    let program = expected_compile("let mut x = 1; if true { { x = 2; } } x".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 2);
}

/// Test deeply nested blocks with multiple levels of scoping (outer vars accessible from inner)
#[test]
fn deeply_nested_blocks_scoping() {
    let program =
        expected_compile("{ let a = 1; { let b = 2; { let c = 3; a + b + c } } }".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 6);
}

/// Test block expression followed by identifier
#[test]
fn block_expression_followed_by_identifier() {
    let program = expected_compile("let mut base = 5; { base = 10; } base * 2".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 20);
}

/// Test deeply nested variable shadowing chain
#[test]
fn deep_shadowing_chain() {
    let program = expected_compile("let x = 1; { let x = 2; { let x = 3; x } }".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 3);
}

/// Test block-scoped variable not visible outside block
#[test]
fn block_scoped_variable_hidden() {
    let result = try_compile("{ let x = 42; } x".to_owned());
    assert!(
        result.is_err(),
        "Should fail: variable x not visible outside block"
    );

    let result = try_compile("let x = 0; { let y = 42; } y".to_string());
    assert!(
        result.is_err(),
        "Should fail: variable y not visible outside block"
    );
}

/// Test Fibonacci sequence with while loop
#[test]
fn fibonacci_while() {
    let program = expected_compile(
            "let mut a = 0; let mut b = 1; let mut i = 0; while i < 10 { let temp = a + b; a = b; b = temp; i += 1; } a".to_owned(),
        );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 55); // 10th Fibonacci number
}

/// Test if else with boolean result as condition
#[test]
fn if_with_boolean_expression_condition() {
    let program = expected_compile(
        "let x = 5; let y = 10; let z = 15; if x + y == z { 1 } else { 0 }".to_owned(),
    );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 1);
}

/// Test if else chain where first condition is true
#[test]
fn if_else_chain_first_true() {
    let program = expected_compile(
        "let x = 200; if x > 100 { 1 } else if x > 50 { 2 } else { 3 }".to_owned(),
    );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 1);
}

/// Test if else chain where last condition is true
#[test]
fn if_else_chain_last_true() {
    let program = expected_compile(
        "let x = 10; if x > 100 { 1 } else if x > 50 { 2 } else if x > 5 { 3 } else { 4 }"
            .to_owned(),
    );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 3);
}

/// Test if else chain where none match (falls to else)
#[test]
fn if_else_chain_falls_to_else() {
    let program = expected_compile(
        "let x = 5; if x > 100 { 1 } else if x > 50 { 2 } else if x > 10 { 3 } else { 0 }"
            .to_owned(),
    );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 0);
}

/// After if statements were converted to if expressions, there was a regression in a case that looked like v
/// Hopefully will prevent it in the future.
#[test]
fn if_else_followed_by_let() {
    let program =
        expected_compile("let x = 5; if x > 100 { return; } let result = 5 + x; result".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 10);
}

#[test]
fn if_else_followed_by_return() {
    let program = expected_compile(
        "let x = 5; { if x > 2 { if x > 3 { /* Something */ } return; } }".to_owned(),
    );
    let executor = Executor::default();
    let _ = executor.run(&program).expect("Should execute");
}

/// Test while loop with early exit pattern (simulated break)
#[test]
fn while_early_exit_pattern() {
    let program = expected_compile(
        r#"let mut found = false;
let mut result = -1;
let mut i = 0;
while !found && i < 10 {
	if i == 5 {
		found = true;
		result = i;
	}
	i += 1;
}
result"#
            .to_owned(),
    );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 5);
}

/// Test that uninitialized variable used without assignment causes compile error
#[test]
fn uninitialized_variable_used_without_assignment() {
    let result = try_compile("let x; x + 1".to_string());
    // Should fail: x is uninitialized and never assigned
    assert!(
        result.is_err(),
        "Uninitialized variable without assignment should fail"
    );
}

/// Test that potentially uninitialized variable in conditional causes compile error
#[test]
fn potentially_uninitialized_in_loop() {
    let result = try_compile("let y; while false { y = 1; } y".to_string());
    // Should fail: y might not be initialized
    assert!(
        result.is_err(),
        "Potentially uninitialized variable should fail"
    );
}

/// Test constant variable cannot be reassigned (type check level)
#[test]
fn constant_cannot_be_reassigned() {
    let result = try_compile("const x = 5; x = 10; x".to_string());
    // Should fail: x is constant
    assert!(
        result.is_err(),
        "Constant variable reassignment should fail"
    );
}

/// Test string concatenation with multiple operands
#[test]
fn string_concatenation_multiple() {
    let program = expected_compile(
        "let a = \"Hello\"; let b = \" \"; let c = \"World\"; a + b + c".to_owned(),
    );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<String>(&program)
        .expect("Should execute");
    assert_eq!(result, "Hello World");
}

/// Test string with numeric variable
#[test]
fn string_with_numeric_variable() {
    let program = expected_compile("let x = 42; \"The answer is \" + x".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<String>(&program)
        .expect("Should execute");
    assert_eq!(result, "The answer is 42");
}

/// Test self-assignment pattern
#[test]
fn self_assignment() {
    let program = expected_compile("let mut x = 5; x = x; x".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 5);
}

/// Test swap pattern with temporary variable
#[test]
fn swap_pattern() {
    let program = expected_compile(
        "let mut a = 5; let mut b = 10; let temp = a; a = b; b = temp; a + b".to_owned(),
    );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 15);
}

/// Test nested blocks with different variable scopes
#[test]
fn nested_blocks_different_scopes() {
    let program = expected_compile("{ let a = 1; { let b = 2; a + b } }".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 3);
}

/// Test multiple sequential blocks
#[test]
fn multiple_sequential_blocks() {
    let program =
        expected_compile("{ let a = 1; a } + { let b = 2; b } + { let c = 3; c }".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 6);
}

/// Test deeply nested if conditions
#[test]
fn deeply_nested_if_conditions() {
    let program = expected_compile(
            "let a = true; let b = true; let c = true; let d = true; if a { if b { if c { if d { 1 } else { 0 } } else { 0 } } else { 0 } } else { 0 }".to_owned(),
        );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 1);
}

/// Test if else with nested block in branch
#[test]
fn if_else_with_nested_block() {
    let program =
        expected_compile("let x = 5; if x > 3 { { let y = 10; { y + 1 } } } else { 0 }".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 11);
}

/// Bitwise AND
#[test]
fn bitwise_and_operator() {
    let program = expected_compile("let a = 12; let b = 10; a & b".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 8); // 12 & 10 = 8 (1100 & 1010 = 1000)
}

/// Bitwise OR
#[test]
fn bitwise_or_operator() {
    let program = expected_compile("let a = 12; let b = 10; a | b".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 14); // 12 | 10 = 14 (1100 | 1010 = 1110)
}

/// Bitwise XOR
#[test]
fn bitwise_xor_operator() {
    let program = expected_compile("let a = 12; let b = 10; a ^ b".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 6); // 12 ^ 10 = 6 (1100 ^ 1010 = 0110)
}

/// Bitwise left shift
#[test]
fn bitwise_shift_left_operator() {
    let program = expected_compile("let a = 1; let b = 4; a << b".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 16); // 1 << 4 = 16 (0001 << 4 = 10000)
}

/// Bitwise right shift
#[test]
fn bitwise_shift_right_operator() {
    let program = expected_compile("let a = 16; let b = 2; a >> b".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 4); // 16 >> 2 = 4 (10000 >> 2 = 00100)
}

/// Bitwise AND equal
#[test]
fn bitwise_and_equal_operator() {
    let program = expected_compile("let mut a = 15; a &= 10; a".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 10); // 15 &= 10 -> 15 & 10 = 10 (1111 & 1010 = 1010)
}

/// Bitwise OR equal
#[test]
fn bitwise_or_equal_operator() {
    let program = expected_compile("let mut a = 4; a |= 3; a".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 7); // 4 |= 3 -> 4 | 3 = 7 (0100 | 0011 = 0111)
}

/// Bitwise XOR equal
#[test]
fn bitwise_xor_equal_operator() {
    let program = expected_compile("let mut a = 15; a ^= 8; a".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 7); // 15 ^= 8 -> 15 ^ 8 = 7 (1111 ^ 1000 = 0111)
}

/// Ensure character escaping is working
#[test]
fn light_saber_battle() {
    let program = expected_compile(
        r#""Light saber battle       V/\"\\E\n------------------------------------""#.to_owned(),
    );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<String>(&program)
        .expect("Should execute");
    // Two
    // Light saber battle       V/"\E
    // ------------------------------------
    assert_eq!(
        result,
        "Light saber battle       V/\"\\E\n------------------------------------"
    );
}

/// Test fail std function returns execution error with given message
#[test]
fn fail_std_function() {
    let program = expected_compile(r#"fail("this is an error")"#.to_owned());
    let executor = Executor::default();
    let result = executor.run(&program);
    assert!(result.is_err(), "fail() should produce an execution error");
    let err = result.unwrap_err();
    assert_eq!(err.message(), "FAIL: this is an error");
}

/// Test fail std function with empty message
#[test]
fn fail_std_function_empty_message() {
    let program = expected_compile(r#"fail("")"#.to_owned());
    let executor = Executor::default();
    let result = executor.run(&program);
    assert!(
        result.is_err(),
        "fail(\"\") should produce an execution error"
    );
    let err = result.unwrap_err();
    assert_eq!(err.message(), "FAIL: ");
}

/// Test fail std function inside a conditional branch (statement context)
#[test]
fn fail_std_function_in_branch() {
    let program = expected_compile(
        r#"let x = 10; if x > 5 { fail("x is too big"); } else { /* Ok */ }; 42"#.to_owned(),
    );
    let executor = Executor::default();
    let result = executor.run(&program);
    assert!(
        result.is_err(),
        "fail() inside if branch should produce an error"
    );
}

/*
/// Test unary range
#[test]
fn unary_range() {
    let program = expected_compile("..5".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<Range<i64>>(&program)
        .expect("Should execute");
    assert_eq!(result, Range { start: 0, end: 5 });
}

/// Test unary equal range
#[test]
fn unary_range_equal() {
    let program = expected_compile("..=8".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<Range<i64>>(&program)
        .expect("Should execute");
    assert_eq!(result, Range { start: 0, end: 9 });
}

/// Test binary range
#[test]
fn binary_range() {
    let program = expected_compile("2..7".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<Range<i64>>(&program)
        .expect("Should execute");
    assert_eq!(result, Range { start: 2, end: 7 });
}

/// Test binary equal range
#[test]
fn binary_range_equal() {
    let program = expected_compile("-3..=-1".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<Range<i64>>(&program)
        .expect("Should execute");
    assert_eq!(result, Range { start: -3, end: 0 });
}

/// Test range with addition on left side (Term precedence: + and .. are equal, left to right)
#[test]
fn binary_range_with_addition_left() {
    let program = expected_compile("1 + 2..5".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<Range<i64>>(&program)
        .expect("Should execute");
    // (1 + 2)..5 = 3..5
    assert_eq!(result, Range { start: 3, end: 5 });
}

/// Test range with addition on right side (Term precedence: .. binds left operand first)
#[test]
fn binary_range_with_addition_right() {
    let program = expected_compile("1..2 + 3".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<Range<i64>>(&program)
        .expect("Should execute");
    // 1..(2 + 3) = 1..5
    assert_eq!(result, Range { start: 1, end: 5 });
}

/// Test range with multiplication on both sides (Factor > Term)
#[test]
fn binary_range_with_multiplication_valid() {
    let program = expected_compile("2 * 3..4 + 5".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<Range<i64>>(&program)
        .expect("Should execute");
    // (2*3)..(4+5) = 6..9
    assert_eq!(result, Range { start: 6, end: 9 });
}

/// Test unary range with multiplication on right side
#[test]
fn unary_range_with_multiplication() {
    let program = expected_compile("..2 * 3".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<Range<i64>>(&program)
        .expect("Should execute");
    // ..(2*3) = ..6
    assert_eq!(result, Range { start: 0, end: 6 });
}

/// Test range with subtraction on left (both Term, left to right associativity)
#[test]
fn binary_range_with_subtraction_left() {
    let program = expected_compile("10 - 3..5".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<Range<i64>>(&program)
        .expect("Should execute");
    // (10 - 3)..5 = 7..5 (empty range, start >= end for exclusive)
    assert_eq!(result, Range { start: 7, end: 5 });
}

/// Test range with subtraction on right (term precedence)
#[test]
fn binary_range_with_subtraction_right() {
    let program = expected_compile("1..10 - 3".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<Range<i64>>(&program)
        .expect("Should execute");
    // 1..(10 - 3) = 1..7
    assert_eq!(result, Range { start: 1, end: 7 });
}

/// Test exclusive range with comparison on right side (Comparison < Term, so .. binds tighter)
#[test]
fn binary_range_with_comparison() {
    let program = expected_compile("let r = 0..5; if r.start > 3 { 1 } else { 0 }".to_owned());
    let executor = Executor::default();
    // Comparison(50) < Term(60), so .. binds tighter than >
    // (0..5).start > 3 → false, returns 0
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 0);
}

/// Test inclusive range with negative numbers on both sides
#[test]
fn binary_range_equal_negative_both_sides() {
    let program = expected_compile("-5..=-2".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<Range<i64>>(&program)
        .expect("Should execute");
    assert_eq!(result, Range { start: -5, end: -1 });
}

/// Test exclusive range with negative numbers on both sides
#[test]
fn binary_range_negative_both_sides() {
    let program = expected_compile("-5..-2".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<Range<i64>>(&program)
        .expect("Should execute");
    assert_eq!(result, Range { start: -5, end: -2 });
}

/// Test unary range with equal bound and multiplication
#[test]
fn unary_range_equal_with_multiplication() {
    let program = expected_compile("..=2 * 3".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<Range<i64>>(&program)
        .expect("Should execute");
    // ..=(2*3) = ..=6, so end is 7
    assert_eq!(result, Range { start: 0, end: 7 });
}

/// Test chained ranges (left to right associativity at Term level)
#[test]
fn binary_range_chained() {
    let program = expected_compile("1..2..3".to_owned());
    let executor = Executor::default();
    // Parsed as (1..2)..3, but the inner 1..2 is a Range, and Range..i64 doesn't make sense
    // This should fail at constructor / type checking level since you can't have Range as left operand of ..
    let result = try_compile("1..2..3".to_owned());
    assert!(
        result.is_err(),
        "Chained ranges should fail: left operand of .. must be numeric, not Range"
    );
}

/// Test range in if condition context (range as expression)
#[test]
fn binary_range_in_expression() {
    let program = expected_compile("let r = 1..5; r".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<Range<i64>>(&program)
        .expect("Should execute");
    assert_eq!(result, Range { start: 1, end: 5 });
}

/// Test range with parentheses to override precedence
#[test]
fn binary_range_with_parentheses() {
    let program = expected_compile("(1 + 2)..(4 - 1)".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<Range<i64>>(&program)
        .expect("Should execute");
    assert_eq!(result, Range { start: 3, end: 3 }); // Empty range
}

/// Test unary range with parentheses
#[test]
fn unary_range_with_parentheses() {
    let program = expected_compile("(..5)".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<Range<i64>>(&program)
        .expect("Should execute");
    assert_eq!(result, Range { start: 0, end: 5 });
}

/// Test range with variable on left side
#[test]
fn binary_range_with_variable_left() {
    let program = expected_compile("let start = 10; start..20".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<Range<i64>>(&program)
        .expect("Should execute");
    assert_eq!(result, Range { start: 10, end: 20 });
}

/// Test range with variable on right side
#[test]
fn binary_range_with_variable_right() {
    let program = expected_compile("let end = 20; 10..end".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<Range<i64>>(&program)
        .expect("Should execute");
    assert_eq!(result, Range { start: 10, end: 20 });
}

/// Test range with variables on both sides
#[test]
fn binary_range_with_variables_both_sides() {
    let program = expected_compile("let a = 5; let b = 15; a..b".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<Range<i64>>(&program)
        .expect("Should execute");
    assert_eq!(result, Range { start: 5, end: 15 });
}

/// Test range with arithmetic on both sides using variables
#[test]
fn binary_range_with_arithmetic_both_sides() {
    let program =
        expected_compile("let a = 2; let b = 3; let c = 4; let d = 5; a * b..c + d".to_owned());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<Range<i64>>(&program)
        .expect("Should execute");
    // (2*3)..(4+5) = 6..9
    assert_eq!(result, Range { start: 6, end: 9 });
}*/
