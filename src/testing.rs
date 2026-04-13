#[cfg(test)]
mod tests {

    use crate::{
        Location,
        constructor::Constructor,
        executor::Executor,
        instructor::Instructor,
        module::Module,
        parser::{BinaryOp, Expr, Parser, Stmt},
        scanner::{Scanner, Token, TokenType},
    };

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

        let module = Module::std();

        // Type checking and such
        let constructor = Constructor::new(module.clone());
        let astmts = constructor
            .generate(parser.get().unwrap())
            .expect("Should be able to construct");

        // We should have the stmts needed
        assert_eq!(astmts.len(), 1);

        // Turn to machine code
        let instructor = Instructor::new(module);
        let program = instructor.generate(&astmts);

        // Should execute to valid value
        let mut executor = Executor::default();
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

        let module = Module::std();

        // Type checking and such
        let constructor = Constructor::new(module.clone());
        let astmts = constructor
            .generate(parser.get().expect("Should have stmts"))
            .expect("Should be able to construct");

        // We should have the stmts needed
        assert_eq!(astmts.len(), 1);

        // Turn to machine code
        let instructor = Instructor::new(module);
        let program = instructor.generate(&astmts);

        // Should execute to valid value
        let mut executor = Executor::default();
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

                let mut executor = Executor::default();
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

                let mut executor = Executor::default();
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
            "let mut i = 1_000_000; let mut r = 0; while i > 0 { i -= 1; r += 1; } i + r"
                .to_owned(),
        );
        let mut executor = Executor::default();
        let result = executor
            .run_with_return::<i64>(&program)
            .expect("Should execute");
        assert_eq!(result, 1_000_000);
    }

    fn expected_compile(source: String) -> crate::instructor::Program {
        let mut scanner = Scanner::new(source);
        scanner.scan();

        // We should have no errors
        assert_eq!(scanner.errors(), &[]);

        // Parse the scanned tokens
        let mut parser = Parser::new(scanner.get().unwrap());
        parser.parse();

        // We should have no errors
        assert_eq!(parser.errors(), &[]);

        let module = Module::std();

        // Type checking and such
        let constructor = Constructor::new(module.clone());
        let astmts = constructor
            .generate(parser.get().expect("Should have stmts"))
            .expect("Should be able to construct");

        // Turn to machine code
        let instructor = Instructor::new(module);
        let program = instructor.generate(&astmts);
        program
    }
}
