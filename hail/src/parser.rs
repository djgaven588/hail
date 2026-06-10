use crate::{
    Location,
    scanner::{Token, TokenType},
};

#[derive(Debug, PartialEq, Clone)]
pub struct ParseError {
    pub token: Token,
    error: ParseErrorType,
}

impl ParseError {
    pub fn new(token: Token, error: ParseErrorType) -> ParseError {
        ParseError { token, error }
    }

    pub fn message(&self) -> String {
        match &self.error {
            ParseErrorType::ExpectedClosingParenthese => {
                "Expected ')' after expression.".to_string()
            }
            ParseErrorType::ExpectedExpression => "Expected an expression here.".to_string(),
            ParseErrorType::ExpectedParameters => {
                "Expected function parameters in parentheses.".to_string()
            }
            ParseErrorType::InvalidOperator(op) => format!("Invalid operator: {op:?}."),
            ParseErrorType::InvalidForStatement => "Invalid 'for' statement syntax.".to_string(),
            ParseErrorType::UnexpectedEndOfFile => "Unexpected end of file.".to_string(),
            ParseErrorType::UnexpectedToken => "Unexpected token.".to_string(),
            ParseErrorType::MissingSemicolon => "Expected ';' before next statement.".to_string(),
            ParseErrorType::StatementInvalid => "Invalid statement syntax.".to_string(),
            ParseErrorType::ExpectedIdentifier => "Expected an identifier here.".to_string(),
            ParseErrorType::ExpectedVariableName => "Expected a variable name here.".to_string(),
            ParseErrorType::ExpectedBlock => "Expected a block '{ ... }'.".to_string(),
            ParseErrorType::ExpectedCondition => "Expected a condition expression.".to_string(),
            ParseErrorType::ExpectedForBody => {
                "Expected 'for' body after the loop clause.".to_string()
            }
            ParseErrorType::ExpectedAssignmentOp => {
                "Expected an assignment operator (=, +=, -=, *=, /=).".to_string()
            }
            ParseErrorType::ExpectedComma => "Expected ',' between values.".to_string(),
            ParseErrorType::ExpectedTypeColon => "Expected ':' for type annotation.".to_string(),
            ParseErrorType::ExpectedType => "Expected a type name.".to_string(),
            ParseErrorType::ExpectedForIn => {
                "Expected 'in' keyword in 'for ... in ...' loop.".to_string()
            }
            ParseErrorType::ExpectedRangeBinaryOp => {
                "Expected '..' or '..=' for range syntax.".to_string()
            }
            ParseErrorType::ExpectedImportPath => {
                "Expected a module path after 'import'.".to_string()
            }
            ParseErrorType::ExpectedAs => "Expected 'as' keyword for renaming.".to_string(),
            ParseErrorType::ExpectedClosingSquareBracket => "Expected ']' end bracket.".to_string(),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum ParseErrorType {
    ExpectedClosingParenthese,
    ExpectedExpression,
    ExpectedParameters,
    InvalidOperator(TokenType),
    InvalidForStatement,
    UnexpectedEndOfFile,
    UnexpectedToken,
    MissingSemicolon,
    StatementInvalid,
    ExpectedIdentifier,
    ExpectedVariableName,
    ExpectedBlock,
    ExpectedCondition,
    ExpectedForBody,
    ExpectedAssignmentOp,
    ExpectedComma,
    ExpectedTypeColon,
    ExpectedType,
    ExpectedForIn,
    ExpectedRangeBinaryOp,
    ExpectedImportPath,
    ExpectedAs,
    ExpectedClosingSquareBracket,
}

#[derive(Debug)]
pub struct Parser {
    tokens: Vec<Token>,
    statements: Vec<Stmt>,
    parse_errors: Vec<ParseError>,
    current: usize,
    panicking: bool,
}

impl Parser {
    pub fn new(tokens: &[Token]) -> Parser {
        Parser {
            tokens: tokens.to_vec(),
            parse_errors: vec![],
            statements: vec![],
            current: 0,
            panicking: false,
        }
    }

    /// Called in the event that we had a parsing error to try and recover
    fn synchronize(&mut self) {
        while !self.is_end()
            && let Some(token) = self.peek()
        {
            match token.token_type {
                TokenType::Semicolon
                | TokenType::If
                | TokenType::While
                | TokenType::Fn
                | TokenType::Return
                | TokenType::Let
                | TokenType::Struct
                | TokenType::Import
                | TokenType::DocComment
                | TokenType::Eof => {
                    break;
                }
                _ => {}
            }

            let _ = self.advance();
        }
    }

    pub fn parse(&mut self) {
        while !self.is_end() {
            // We continue regardless for parsing reasons
            if let Some(stmt) = self.declaration() {
                self.statements.push(stmt);
            }
        }
    }

    fn declaration(&mut self) -> Option<Stmt> {
        let stmt = if self.try_consume(&[TokenType::Fn]).is_some() {
            self.function_declaration()
        } else if self.try_consume(&[TokenType::Let]).is_some() {
            self.var_declaration()
        } else if self.try_consume(&[TokenType::Const]).is_some() {
            self.const_declaration()
        } else {
            self.statement()
        };

        if self.panicking {
            // Get the parser into *any* state we can continue with.
            self.synchronize();
            self.panicking = false;
        }

        stmt
    }

    fn function_declaration(&mut self) -> Option<Stmt> {
        let Some(function_name) = self.try_consume(&[TokenType::Identifier]).cloned() else {
            self.error(ParseErrorType::ExpectedIdentifier);
            return None;
        };

        self.consume(
            TokenType::LeftParenthese,
            ParseErrorType::ExpectedParameters,
        )?;

        let mut parameters = vec![];

        // If there's an immediate `)` the parameter list is empty; otherwise parse params.
        if self.try_consume(&[TokenType::RightParenthese]).is_none() {
            // Loop until we run out of parameters
            loop {
                // Check if this variable will be mutable, it's fine if it isn't.
                let mutability: VariableMutability =
                    if self.try_consume(&[TokenType::Mut]).is_some() {
                        VariableMutability::Mutable
                    } else if self.try_consume(&[TokenType::Const]).is_some() {
                        VariableMutability::Constant
                    } else {
                        VariableMutability::Immutable
                    };

                // We *must* have an identifier
                let parameter_name =
                    self.consume(TokenType::Identifier, ParseErrorType::ExpectedIdentifier)?;

                self.consume(TokenType::Colon, ParseErrorType::ExpectedTypeColon)?;

                let typing = self.consume(TokenType::Identifier, ParseErrorType::ExpectedType)?;

                parameters.push(FunctionParameter::new(
                    mutability,
                    parameter_name.clone(),
                    typing,
                ));

                // Remove trailing commas, we're done if there is none.
                if self.try_consume(&[TokenType::Comma]).is_none() {
                    break;
                }
            }

            self.consume(
                TokenType::RightParenthese,
                ParseErrorType::ExpectedClosingParenthese,
            )?;
        }

        // Determine the type
        let mut return_typing = None;
        if self.try_consume(&[TokenType::Arrow]).is_some() {
            // Should have an identifier
            return_typing =
                Some(self.consume(TokenType::Identifier, ParseErrorType::ExpectedType)?);
        }

        self.consume(TokenType::LeftBrace, ParseErrorType::ExpectedBlock)?;

        let body = self.block_expression()?;

        Some(Stmt::Function(
            function_name,
            parameters,
            Box::new(body),
            return_typing,
        ))
    }

    fn const_declaration(&mut self) -> Option<Stmt> {
        let Some(name) = self.try_consume(&[TokenType::Identifier]).cloned() else {
            self.error(ParseErrorType::ExpectedIdentifier);
            return None;
        };

        self.consume(TokenType::Equal, ParseErrorType::ExpectedExpression)?;

        let Some(initializer) = self.expression() else {
            return None;
        };

        self.consume(TokenType::Semicolon, ParseErrorType::MissingSemicolon)?;

        Some(Stmt::DefineVariable(
            name,
            Some(Box::new(initializer)),
            VariableMutability::Constant,
        ))
    }

    fn var_declaration(&mut self) -> Option<Stmt> {
        let mutable = self.try_consume(&[TokenType::Mut]).is_some();

        let Some(name) = self.try_consume(&[TokenType::Identifier]).cloned() else {
            self.error(ParseErrorType::ExpectedIdentifier);
            return None;
        };

        let initializer = if self.try_consume(&[TokenType::Equal]).is_some() {
            if let Some(expr) = self.expression() {
                Some(Box::new(expr))
            } else {
                self.error(ParseErrorType::ExpectedAssignmentOp);
                return None;
            }
        } else {
            None
        };

        self.consume(TokenType::Semicolon, ParseErrorType::MissingSemicolon)?;

        Some(Stmt::DefineVariable(
            name,
            initializer,
            if mutable {
                VariableMutability::Mutable
            } else {
                VariableMutability::Immutable
            },
        ))
    }

    fn statement(&mut self) -> Option<Stmt> {
        if self.try_consume(&[TokenType::For]).is_some() {
            self.for_statement()
        } else if self.try_consume(&[TokenType::While]).is_some() {
            self.while_statement()
        } else if self.try_consume(&[TokenType::Loop]).is_some() {
            self.loop_statement()
        } else if self.try_consume(&[TokenType::Return]).is_some() {
            self.return_statement()
        } else if self.try_consume(&[TokenType::Break]).is_some() {
            self.break_statement()
        } else if self.try_consume(&[TokenType::Continue]).is_some() {
            self.continue_statement()
        } else if self.try_consume(&[TokenType::Import]).is_some() {
            self.import_statement()
        } else {
            self.expr_statement()
        }
    }

    fn import_statement(&mut self) -> Option<Stmt> {
        let path = self.consume(TokenType::String, ParseErrorType::ExpectedImportPath)?;
        self.consume(TokenType::As, ParseErrorType::ExpectedAs)?;
        let alias = self.consume(TokenType::Identifier, ParseErrorType::ExpectedIdentifier)?;

        // Fix lexeme to not contain the string quotes, should this just be fixed at scanning? Probably not.
        let mut path = path;
        path.lexeme = path.lexeme.to_string();

        self.consume(TokenType::Semicolon, ParseErrorType::MissingSemicolon)?;

        Some(Stmt::Import(path, alias))
    }

    fn for_statement(&mut self) -> Option<Stmt> {
        // There are two ways this goes:
        // - for (let i=0; i<5; i+=1;) {}
        // - for x in range {} (additional sugar)
        if self.try_consume(&[TokenType::LeftParenthese]).is_some() {
            self.classic_for_statement()
        } else if self.try_consume(&[TokenType::Identifier]).is_some() {
            self.range_for_statement(false)
        } else if self.try_consume(&[TokenType::Mut]).is_some() {
            self.consume(TokenType::Identifier, ParseErrorType::ExpectedVariableName)?;
            self.range_for_statement(true)
        } else {
            self.error(ParseErrorType::InvalidForStatement);
            None
        }
    }

    fn range_for_statement(&mut self, is_mutable: bool) -> Option<Stmt> {
        // for mut var_name in expression {}
        // for var_name in expression {}
        let variable = self.last().unwrap().clone();

        self.consume(TokenType::In, ParseErrorType::ExpectedForIn)?;

        let iterator_expr = self.expression()?;

        self.consume(TokenType::LeftBrace, ParseErrorType::ExpectedBlock)?;

        let body = self.block_expression()?;

        // Of note, the same variable name is used for the iterator variable, the acquired next variable, and the unwrapped variable
        // This is intentional, and allows shadowing to hide away this desugaring.
        // TODO: Should this be in the constructor? Error messages may be suboptimal here.
        Some(Stmt::Expression(
            Box::new(Expr::Block(
                self.last().expect("Should have for keyword").location,
                vec![
                    // Convert expression to iterator
                    Stmt::DefineVariable(
                        variable.clone(),
                        Some(Box::new(Expr::DotAccess(
                            Box::new(iterator_expr),
                            "into_iter".to_string(),
                            Some(vec![]),
                        ))),
                        VariableMutability::Mutable,
                    ),
                    Stmt::While(
                        Box::new(Expr::Block(
                            body.get_location(),
                            vec![
                                // Try taking the next item
                                Stmt::DefineVariable(
                                    variable.clone(),
                                    Some(Box::new(Expr::DotAccess(
                                        Box::new(Expr::Variable(
                                            variable.location.clone(),
                                            variable.lexeme.clone(),
                                        )),
                                        "next".to_owned(),
                                        Some(vec![]),
                                    ))),
                                    VariableMutability::Immutable,
                                ),
                                Stmt::Expression(
                                    Box::new(Expr::If(
                                        // If we have another item...
                                        Box::new(Expr::DotAccess(
                                            Box::new(Expr::Variable(
                                                variable.location.clone(),
                                                variable.lexeme.clone(),
                                            )),
                                            "is_some".to_owned(),
                                            Some(vec![]),
                                        )),
                                        // Unwrap it and call the body for it
                                        Box::new(Expr::Block(
                                            variable.location,
                                            vec![
                                                Stmt::DefineVariable(
                                                    variable.clone(),
                                                    Some(Box::new(Expr::DotAccess(
                                                        Box::new(Expr::Variable(
                                                            variable.location,
                                                            variable.lexeme,
                                                        )),
                                                        "unwrap".to_owned(),
                                                        Some(vec![]),
                                                    ))),
                                                    if is_mutable {
                                                        VariableMutability::Mutable
                                                    } else {
                                                        VariableMutability::Immutable
                                                    },
                                                ),
                                                // Actual looping body
                                                Stmt::Expression(Box::new(body.clone()), false),
                                                // Continue looping
                                                Stmt::Expression(
                                                    Box::new(Expr::Bool(variable.location, true)),
                                                    true,
                                                ),
                                            ],
                                        )),
                                        // End loop otherwise
                                        Some(Box::new(Expr::Block(
                                            variable.location,
                                            vec![Stmt::Expression(
                                                Box::new(Expr::Bool(variable.location, false)),
                                                true,
                                            )],
                                        ))),
                                    )),
                                    true,
                                ),
                            ],
                        )),
                        // Empty body, loop handled in condition
                        Box::new(Expr::Block(body.get_location(), vec![])),
                    ),
                ],
            )),
            false,
        ))
    }

    fn classic_for_statement(&mut self) -> Option<Stmt> {
        let initializer = self.declaration()?;

        let condition = self.expression()?;

        self.consume(TokenType::Semicolon, ParseErrorType::MissingSemicolon)?;

        // Optional semicolon on chaser
        // (initializer; condition; chaser) { body }
        // (initializer; condition; chaser;) { body }
        let chaser = self.expression()?;

        self.try_consume(&[TokenType::Semicolon]);

        self.consume(
            TokenType::RightParenthese,
            ParseErrorType::ExpectedClosingParenthese,
        )?;

        let block_start = self.consume(TokenType::LeftBrace, ParseErrorType::ExpectedBlock)?;

        let body = self.block_expression()?;

        Some(Stmt::Expression(
            Box::new(Expr::Block(
                block_start.location,
                vec![
                    // Preamble
                    initializer,
                    Stmt::While(
                        Box::new(condition),
                        Box::new(Expr::Block(
                            body.get_location(),
                            vec![
                                // Encapsulated Body
                                Stmt::Expression(Box::new(body), false),
                                // ^ Prevents redeclaration within body impacting loop
                                Stmt::Expression(Box::new(chaser), false),
                            ],
                        )),
                    ),
                ],
            )),
            false,
        ))
    }

    fn while_statement(&mut self) -> Option<Stmt> {
        let condition = self.expression()?;

        self.consume(TokenType::LeftBrace, ParseErrorType::ExpectedBlock)?;

        let body = self.block_expression()?;

        Some(Stmt::While(Box::new(condition), Box::new(body)))
    }

    fn loop_statement(&mut self) -> Option<Stmt> {
        self.consume(TokenType::LeftBrace, ParseErrorType::ExpectedBlock)?;

        let body = self.block_expression()?;

        Some(Stmt::Loop(Box::new(body)))
    }

    fn if_expression(&mut self) -> Option<Expr> {
        let condition = self.expression()?;

        self.consume(TokenType::LeftBrace, ParseErrorType::ExpectedBlock)?;

        let body = self.block_expression()?;
        let else_block = if self.try_consume(&[TokenType::Else]).is_some() {
            // We first try to find an else if
            Some(Box::new(if self.try_consume(&[TokenType::If]).is_some() {
                self.if_expression()?
            } else {
                // Then we fall back to expecting a block.
                self.consume(TokenType::LeftBrace, ParseErrorType::ExpectedBlock)?;
                self.block_expression()?
            }))
        } else {
            None
        };

        Some(Expr::If(Box::new(condition), Box::new(body), else_block))
    }

    fn block_expression(&mut self) -> Option<Expr> {
        let mut stmts = vec![];

        while self.try_consume(&[TokenType::RightBrace]).is_none() && !self.is_end() {
            // Only push actual declarations, we keep going for parser reasons
            if let Some(stmt) = self.declaration() {
                stmts.push(stmt);
            }
        }

        Some(Expr::Block(
            self.last().expect("Should have previous token").location,
            stmts,
        ))
    }

    fn return_statement(&mut self) -> Option<Stmt> {
        // Catch return with no value
        if self.try_consume(&[TokenType::Semicolon]).is_some() {
            return Some(Stmt::Return(self.last().unwrap().location, None));
        }

        let expr = if let Some(expr) = self.expression() {
            Box::new(expr)
        } else {
            self.error(ParseErrorType::ExpectedExpression);
            return None;
        };
        self.consume(TokenType::Semicolon, ParseErrorType::MissingSemicolon)?;

        Some(Stmt::Return(self.last().unwrap().location, Some(expr)))
    }

    fn break_statement(&mut self) -> Option<Stmt> {
        // Catch return with no value
        self.consume(TokenType::Semicolon, ParseErrorType::MissingSemicolon)?;

        Some(Stmt::Break(self.last().unwrap().location))
    }

    fn continue_statement(&mut self) -> Option<Stmt> {
        // Catch return with no value
        self.consume(TokenType::Semicolon, ParseErrorType::MissingSemicolon)?;

        Some(Stmt::Continue(self.last().unwrap().location))
    }

    fn expr_statement(&mut self) -> Option<Stmt> {
        let expr = self.expression()?.into();

        Some(Stmt::Expression(
            expr,
            // This statement *can* produce a value if it didn't have a semicolon
            // If there was a semicolon, any potential value will be discarded
            self.try_consume(&[TokenType::Semicolon]).is_none(),
        ))
    }

    fn expression(&mut self) -> Option<Expr> {
        if let Some(expr) = self.parse_precendence(Precedence::Assignment) {
            Some(expr)
        } else {
            self.error(ParseErrorType::ExpectedExpression);
            None
        }
    }

    fn is_end(&self) -> bool {
        self.tokens[self.current].token_type == TokenType::Eof
    }

    fn parse_precendence(&mut self, precedence: Precedence) -> Option<Expr> {
        if self.is_end() {
            self.panicking = true;
            self.parse_errors.push(ParseError::new(
                self.peek().unwrap().clone(),
                ParseErrorType::UnexpectedEndOfFile,
            ));
            return None;
        }

        let mut left = if let (Some(prefix), _, _) = match self.peek()?.token_type.rule() {
            Ok(val) => val,
            Err(err) => {
                self.error(err);
                let _ = self.advance();
                return None;
            }
        } {
            let _ = self.advance();
            prefix(self)?
        } else {
            self.error(ParseErrorType::UnexpectedToken);
            let _ = self.advance();
            return None;
        };

        while !self.is_end() {
            let Some(next) = self.peek().cloned() else {
                return None;
            };

            let (_, infix, token_precedence) = match next.token_type.rule() {
                Ok(val) => val,
                Err(err) => {
                    self.error(err);
                    let _ = self.advance();
                    return None;
                }
            };

            if token_precedence <= precedence {
                break;
            }

            let Some(infix_func) = infix else {
                // If the token cannot act as an infix operator, break out so we don't overconsume tokens
                break;
            };

            let _ = self.advance();
            left = infix_func(self, left)?;
        }

        Some(left)
    }

    fn call(&mut self, callee: Expr) -> Option<Expr> {
        // Easy case, no parameters
        if self.try_consume(&[TokenType::RightParenthese]).is_some() {
            return Some(Expr::Call(Box::new(callee), vec![]));
        }

        // Get each argument
        let mut params = vec![];
        loop {
            params.push(Box::new(self.parse_precendence(Precedence::Assignment)?));

            if self.try_consume(&[TokenType::RightParenthese]).is_some() {
                return Some(Expr::Call(Box::new(callee), params));
            }

            // Try and remove the comma that follows, otherwise we're done.
            self.consume(TokenType::Comma, ParseErrorType::ExpectedComma)?;
        }
    }

    fn qualified_access(&mut self, left: Expr) -> Option<Expr> {
        // Get the namespace from the left expression
        let Expr::Variable(location, namespace) = &left else {
            self.error(ParseErrorType::UnexpectedToken);
            return None;
        };

        // Get the next identifier as the function name
        let identifier = self.consume(TokenType::Identifier, ParseErrorType::ExpectedIdentifier)?;

        Some(Expr::QualifiedAccess(
            *location,
            namespace.to_string(),
            identifier.lexeme,
        ))
    }

    fn method_call(&mut self, receiver: Expr) -> Option<Expr> {
        // The dot has already been consumed as the infix operator.
        // Consume the member name.
        let name_token = self.consume(TokenType::Identifier, ParseErrorType::ExpectedIdentifier)?;
        let name = name_token.lexeme.clone();

        // Optionally consume an argument list if '(' follows.
        let args = if self.try_consume(&[TokenType::LeftParenthese]).is_some() {
            // Easy case, no parameters
            Some(
                if self.try_consume(&[TokenType::RightParenthese]).is_some() {
                    vec![]
                } else {
                    // Get each argument
                    let mut args = vec![];
                    loop {
                        args.push(Box::new(self.parse_precendence(Precedence::Assignment)?));

                        if self.try_consume(&[TokenType::RightParenthese]).is_some() {
                            break;
                        }

                        // Try and remove the comma that follows, otherwise we're done.
                        self.consume(TokenType::Comma, ParseErrorType::ExpectedComma)?;
                    }

                    args
                },
            )
        } else {
            None
        };

        Some(Expr::DotAccess(Box::new(receiver), name, args))
    }

    fn grouping(&mut self) -> Option<Expr> {
        let expr = self.parse_precendence(Precedence::Assignment)?;

        self.consume(
            TokenType::RightParenthese,
            ParseErrorType::ExpectedClosingParenthese,
        )?;

        Some(expr)
    }

    fn logical(&mut self, left: Expr) -> Option<Expr> {
        let token = self.last()?.clone();
        let precedence = match token.token_type.rule() {
            Ok(val) => val.2,
            Err(err) => {
                self.error(err);
                let _ = self.advance();
                return None;
            }
        };

        let Some(right) = self.parse_precendence(precedence) else {
            self.error(ParseErrorType::ExpectedCondition);
            return None;
        };

        let op_type = match token.token_type {
            TokenType::And => LogicalOp::And,
            TokenType::Or => LogicalOp::Or,
            a => {
                unimplemented!("'{a:?}' is not a valid token for a logical op.")
            }
        };

        Some(Expr::Condition(Box::new(left), op_type, Box::new(right)))
    }

    fn binary(&mut self, left: Expr) -> Option<Expr> {
        let token = self.last()?.clone();
        let precedence = match token.token_type.rule() {
            Ok(val) => val.2,
            Err(err) => {
                self.error(err);
                let _ = self.advance();
                return None;
            }
        };
        let Some(right) = self.parse_precendence(precedence) else {
            self.error(ParseErrorType::ExpectedExpression);
            return None;
        };

        let op = match token.token_type {
            TokenType::Minus => BinaryOp::Minus,
            TokenType::Plus => BinaryOp::Plus,
            TokenType::Slash => BinaryOp::Divide,
            TokenType::Star => BinaryOp::Multiply,
            TokenType::Percent => BinaryOp::Remainder,
            TokenType::BangEqual => BinaryOp::BangEqual,
            TokenType::EqualEqual => BinaryOp::EqualEqual,
            TokenType::Greater => BinaryOp::Greater,
            TokenType::GreaterEqual => BinaryOp::GreaterEqual,
            TokenType::Less => BinaryOp::Less,
            TokenType::LessEqual => BinaryOp::LessEqual,
            TokenType::BinaryAnd => todo!(),
            TokenType::BinaryOr => todo!(),
            TokenType::DotDot => BinaryOp::Range(false),
            TokenType::DotDotEqual => BinaryOp::Range(true),
            token => {
                self.error(ParseErrorType::InvalidOperator(token));
                return None;
            }
        };

        Some(Expr::Binary(Box::new(left), op, Box::new(right)))
    }

    fn assign(&mut self, left: Expr) -> Option<Expr> {
        let token = self.last()?.clone();

        let Some(right) = self.parse_precendence(Precedence::Assignment) else {
            self.error(ParseErrorType::ExpectedExpression);
            return None;
        };

        let op = match token.token_type {
            TokenType::Equal => AssignmentOp::Equal,
            TokenType::PlusEqual => AssignmentOp::PlusEqual,
            TokenType::MinusEqual => AssignmentOp::MinusEqual,
            TokenType::StarEqual => AssignmentOp::MultiplyEqual,
            TokenType::SlashEqual => AssignmentOp::DivideEqual,
            TokenType::PercentEqual => AssignmentOp::RemainderEqual,
            token => {
                self.error(ParseErrorType::InvalidOperator(token));
                return None;
            }
        };

        Some(Expr::Assign(Box::new(left), op, Box::new(right)))
    }

    fn unary(&mut self) -> Option<Expr> {
        let token = self.last()?.clone();
        let Some(left) = self.parse_precendence(Precedence::Unary) else {
            self.error(ParseErrorType::ExpectedExpression);
            return None;
        };

        match token.token_type {
            TokenType::Minus => Some(Expr::Unary(UnaryOp::Negate, Box::new(left))),
            TokenType::Bang => Some(Expr::Unary(UnaryOp::Invert, Box::new(left))),
            TokenType::DotDot => Some(Expr::Unary(UnaryOp::Range(false), Box::new(left))),
            TokenType::DotDotEqual => Some(Expr::Unary(UnaryOp::Range(true), Box::new(left))),
            a => unreachable!("{a:?}"),
        }
    }

    fn integer(&mut self) -> Option<Expr> {
        let token = self.last()?;
        match token.token_type {
            TokenType::Integer => Some(Expr::Integer(
                token.location,
                token
                    .lexeme
                    .replace("_", "")
                    .parse()
                    .expect("Integer literal should be valid."),
            )),
            a => unreachable!("{a:?}"),
        }
    }

    fn float(&mut self) -> Option<Expr> {
        let token = self.last()?;
        match token.token_type {
            TokenType::Float => Some(Expr::Float(
                token.location,
                token
                    .lexeme
                    .replace("_", "")
                    .parse()
                    .expect("Float literal should be valid."),
            )),
            a => unreachable!("{a:?}"),
        }
    }

    fn bool(&mut self) -> Option<Expr> {
        let token = self.last()?;
        match token.token_type {
            TokenType::True => Some(Expr::Bool(token.location, true)),
            TokenType::False => Some(Expr::Bool(token.location, false)),
            a => unreachable!("{a:?}"),
        }
    }

    fn string(&mut self) -> Option<Expr> {
        let token = self.last()?;
        match token.token_type {
            TokenType::String => Some(Expr::String(token.location, token.lexeme.to_string())),
            a => unreachable!("{a:?}"),
        }
    }

    fn variable(&mut self) -> Option<Expr> {
        let token = self.last()?;
        match token.token_type {
            TokenType::Identifier => Some(Expr::Variable(token.location, token.lexeme.clone())),
            a => unreachable!("{a:?}"),
        }
    }

    fn array(&mut self) -> Option<Expr> {
        let token = self.last()?.clone();

        let mut params = vec![];
        loop {
            // See if we've hit the end
            if self
                .peek()
                .is_some_and(|v| v.token_type == TokenType::RightSquare)
            {
                break;
            }

            // Grab another value
            params.push(self.parse_precendence(Precedence::Assignment)?);

            // Try and remove the comma that follows, otherwise we're done
            if self.try_consume(&[TokenType::Comma]).is_none() {
                break;
            }
        }

        // End
        self.consume(
            TokenType::RightSquare,
            ParseErrorType::ExpectedClosingSquareBracket,
        )?;

        Some(Expr::Array(token.location, params))
    }

    fn index(&mut self, callee: Expr) -> Option<Expr> {
        let index_value = Box::new(self.parse_precendence(Precedence::Assignment)?);

        self.consume(
            TokenType::RightSquare,
            ParseErrorType::ExpectedClosingSquareBracket,
        )?;

        Some(Expr::Index(Box::new(callee), index_value))
    }

    fn error(&mut self, error: ParseErrorType) {
        if self.panicking {
            // Prevent further errors from coming through while we're within the same panic
            return;
        }

        self.panicking = true;
        self.parse_errors.push(ParseError::new(
            self.peek()
                .expect("Parse error shouldn't occur on missing token..?")
                .clone(),
            error,
        ));
    }

    #[must_use]
    fn consume(&mut self, token_type: TokenType, error: ParseErrorType) -> Option<Token> {
        if let Some(token) = self.try_consume(&[token_type]) {
            Some(token.clone())
        } else {
            self.error(error);
            None
        }
    }

    fn last(&mut self) -> Option<&Token> {
        self.tokens.get(self.current.saturating_sub(1))
    }

    fn advance(&mut self) -> Option<&Token> {
        if let Some(token) = self.tokens.get(self.current) {
            self.current = (self.current + 1).min(self.tokens.len().saturating_sub(1));
            Some(token)
        } else {
            None
        }
    }

    /// Tries to get a token of one of the given types, moving the cursor if successful
    fn try_consume(&mut self, expected: &[TokenType]) -> Option<&Token> {
        if let Some(token) = self.tokens.get(self.current)
            && expected.contains(&token.token_type)
        {
            self.current = (self.current + 1).min(self.tokens.len().saturating_sub(1));
            Some(token)
        } else {
            None
        }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.current)
    }

    fn peek_next(&self) -> Option<&Token> {
        self.tokens.get(self.current + 1)
    }

    pub fn errors(&self) -> &[ParseError] {
        &self.parse_errors
    }

    pub fn get(&self) -> Option<&[Stmt]> {
        if self.parse_errors.is_empty() {
            Some(&self.statements)
        } else {
            None
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Precedence {
    None = 0,
    Assignment = 10,
    AssignTest = 15,
    Or = 20,
    And = 30,
    Equality = 40,
    Comparison = 50,
    Range = 55,
    Term = 60,
    Factor = 70,
    Unary = 80,
    Call = 90,
    Primary = 100,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VariableMutability {
    Immutable,
    Mutable,
    Constant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AssignmentOp {
    Equal,
    PlusEqual,
    MinusEqual,
    MultiplyEqual,
    DivideEqual,
    RemainderEqual,
}

impl AssignmentOp {
    pub fn base_op(&self) -> &str {
        match self {
            AssignmentOp::Equal => "",
            AssignmentOp::PlusEqual => "+",
            AssignmentOp::MinusEqual => "-",
            AssignmentOp::MultiplyEqual => "*",
            AssignmentOp::DivideEqual => "/",
            AssignmentOp::RemainderEqual => "%",
        }
    }
}

impl std::fmt::Display for AssignmentOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AssignmentOp::Equal => write!(f, "="),
            AssignmentOp::PlusEqual => write!(f, "+="),
            AssignmentOp::MinusEqual => write!(f, "-="),
            AssignmentOp::MultiplyEqual => write!(f, "*="),
            AssignmentOp::DivideEqual => write!(f, "/="),
            AssignmentOp::RemainderEqual => write!(f, "%="),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnaryOp {
    Negate,
    Invert,
    // .. vs ..=
    Range(bool),
}

impl std::fmt::Display for UnaryOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UnaryOp::Negate => write!(f, "-"),
            UnaryOp::Invert => write!(f, "!"),
            UnaryOp::Range(inclusive) => write!(f, "{}", if *inclusive { "..=" } else { ".." }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BinaryOp {
    Plus,
    Minus,
    Multiply,
    Divide,
    Remainder,
    Greater,
    GreaterEqual,
    Less,
    LessEqual,
    EqualEqual,
    BangEqual,
    // .. vs ..=
    Range(bool),
}

impl std::fmt::Display for BinaryOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BinaryOp::Plus => write!(f, "+"),
            BinaryOp::Minus => write!(f, "-"),
            BinaryOp::Multiply => write!(f, "*"),
            BinaryOp::Divide => write!(f, "/"),
            BinaryOp::Remainder => write!(f, "%"),
            BinaryOp::Greater => write!(f, ">"),
            BinaryOp::GreaterEqual => write!(f, ">="),
            BinaryOp::Less => write!(f, "<"),
            BinaryOp::LessEqual => write!(f, "<="),
            BinaryOp::EqualEqual => write!(f, "=="),
            BinaryOp::BangEqual => write!(f, "!="),
            BinaryOp::Range(inclusive) => write!(f, "{}", if *inclusive { "..=" } else { ".." }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LogicalOp {
    And,
    Or,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionParameter {
    // TODO: Reference parameters?
    // reference: bool,
    pub mutability: VariableMutability,
    pub name: Token,
    pub typing: Token,
}

impl FunctionParameter {
    fn new(mutability: VariableMutability, name: Token, typing: Token) -> FunctionParameter {
        FunctionParameter {
            mutability,
            name,
            typing,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    // Name, initializer, mutable
    DefineVariable(Token, Option<Box<Expr>>, VariableMutability),
    // Evaluate, can return
    Expression(Box<Expr>, bool),
    Return(Location, Option<Box<Expr>>),
    Break(Location),
    Continue(Location),
    Loop(Box<Expr>),
    // Condition, body
    While(Box<Expr>, Box<Expr>),
    // Name, parameter names (with mutability), body, explicit return type
    Function(Token, Vec<FunctionParameter>, Box<Expr>, Option<Token>),
    // Path,
    Import(Token, Token),
}

impl Stmt {
    pub fn get_location(&self) -> Location {
        match self {
            Stmt::DefineVariable(token, _, _) => token.location,
            Stmt::Expression(expr, _) => expr.get_location(),
            Stmt::Return(location, _) => *location,
            Stmt::Break(location) => *location,
            Stmt::Continue(location) => *location,
            Stmt::While(stmt, _) => stmt.get_location(),
            Stmt::Loop(expr) => expr.get_location(),
            Stmt::Function(token, _, _, _) => token.location,
            Stmt::Import(token, _) => token.location,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Float(Location, f64),
    Integer(Location, i64),
    Bool(Location, bool),
    String(Location, String),
    Array(Location, Vec<Expr>),
    Unary(UnaryOp, Box<Expr>),
    Binary(Box<Expr>, BinaryOp, Box<Expr>),
    Variable(Location, String),
    Condition(Box<Expr>, LogicalOp, Box<Expr>),
    // Callee and its parameters
    Call(Box<Expr>, Vec<Box<Expr>>),
    // Receiver, name, args
    DotAccess(Box<Expr>, String, Option<Vec<Box<Expr>>>),
    // Source, index
    Index(Box<Expr>, Box<Expr>),
    // Namespace alias, identifier (utils::helper)
    QualifiedAccess(Location, String, String),
    Block(Location, Vec<Stmt>),
    Assign(Box<Expr>, AssignmentOp, Box<Expr>),
    // Condition, body, otherwise
    If(Box<Expr>, Box<Expr>, Option<Box<Expr>>),
}

impl Expr {
    pub fn get_location(&self) -> Location {
        match self {
            Expr::Float(location, _) => *location,
            Expr::Integer(location, _) => *location,
            Expr::Bool(location, _) => *location,
            Expr::String(location, _) => *location,
            Expr::Array(location, _) => *location,
            Expr::Unary(_, expr) => expr.get_location(),
            Expr::Binary(expr, _, _) => expr.get_location(),
            Expr::Variable(location, _) => *location,
            Expr::Condition(expr, _, _) => expr.get_location(),
            Expr::Call(expr, _) => expr.get_location(),
            Expr::DotAccess(expr, _, _) => expr.get_location(),
            Expr::Index(expr, _) => expr.get_location(),
            Expr::QualifiedAccess(location, _, _) => *location,
            Expr::Block(location, _) => *location,
            Expr::Assign(expr, _, _) => expr.get_location(),
            Expr::If(expr, _, _) => expr.get_location(),
        }
    }
}

impl Expr {
    pub fn display(&self, level: usize) -> String {
        match self {
            Expr::Float(_, token) => "\t".repeat(level) + token.to_string().as_str() + "\n",
            Expr::Integer(_, token) => "\t".repeat(level) + token.to_string().as_str() + "\n",
            Expr::Bool(_, token) => "\t".repeat(level) + token.to_string().as_str() + "\n",
            Expr::String(_, token) => "\t".repeat(level) + token.to_string().as_str() + "\n",
            Expr::Array(_, elements) => {
                "\t".repeat(level) + format!("{:?}", elements).as_str() + "\n"
            }
            Expr::Unary(op, expr) => {
                "\t".repeat(level)
                    + format!("{:?}{}\n", op, expr.display(level + 1).as_str()).as_str()
            }
            Expr::Binary(expr, op, expr1) => {
                expr.display(level + 1)
                    + "\t".repeat(level).as_str()
                    + format!("{:?}\n{}", op, expr1.display(level + 1)).as_str()
            }
            Expr::QualifiedAccess(_, namespace, name) => {
                "\t".repeat(level) + format!("QualifiedAccess {}::{}\n", namespace, name).as_str()
            }
            Expr::Condition(expr, op, expr1) => {
                expr.display(level + 1)
                    + "\t".repeat(level).as_str()
                    + format!("{:?}\n{}", op, expr1.display(level + 1)).as_str()
            }
            Expr::Variable(_, identifier) => {
                "\t".repeat(level) + "Variable: " + identifier.as_str() + "\n"
            }
            Expr::Call(expr, exprs) => {
                "\t".repeat(level) + format!("Call '{expr:?}' with [{exprs:?}]\n").as_str()
            }
            Expr::DotAccess(expr, name, args) => {
                "\t".repeat(level)
                    + format!("DotAccess '{name}' on [{expr:?}] with {args:?}\n").as_str()
            }
            Expr::Index(expr, index_expr) => {
                "\t".repeat(level) + format!("Index '{expr:?}' at {index_expr:?}\n").as_str()
            }
            Expr::Block(_, stmts) => {
                "\t".repeat(level) + format!("Block statements: [{stmts:?}]\n").as_str()
            }
            Expr::Assign(expr, assignment_op, expr1) => {
                expr.display(level + 1)
                    + "\t".repeat(level).as_str()
                    + format!("{:?}\n{}", assignment_op, expr1.display(level + 1)).as_str()
            }
            Expr::If(expr, expr1, expr2) => {
                expr.display(level + 1)
                    + "\t".repeat(level).as_str()
                    + format!("If {}", expr1.display(level + 1)).as_str()
                    + "\t".repeat(level).as_str()
                    + format!("Else {:?}", expr2.as_ref().map(|v| v.display(level + 1))).as_str()
            }
        }
    }
}

type PrefixParseFunc = fn(&mut Parser) -> Option<Expr>;
type InfixParseFunc = fn(&mut Parser, Expr) -> Option<Expr>;
impl TokenType {
    pub fn rule(
        &self,
    ) -> Result<(Option<PrefixParseFunc>, Option<InfixParseFunc>, Precedence), ParseErrorType> {
        Ok(match self {
            TokenType::Or => (None, Some(Parser::logical), Precedence::Or),
            TokenType::And => (None, Some(Parser::logical), Precedence::And),
            TokenType::Minus => (Some(Parser::unary), Some(Parser::binary), Precedence::Term),
            TokenType::Bang => (Some(Parser::unary), None, Precedence::Term),
            TokenType::Plus => (None, Some(Parser::binary), Precedence::Term),
            TokenType::Star => (None, Some(Parser::binary), Precedence::Factor),
            TokenType::Slash => (None, Some(Parser::binary), Precedence::Factor),
            TokenType::Percent => (None, Some(Parser::binary), Precedence::Factor),
            TokenType::Integer => (Some(Parser::integer), None, Precedence::Primary),
            TokenType::Float => (Some(Parser::float), None, Precedence::Primary),
            TokenType::String => (Some(Parser::string), None, Precedence::Primary),
            TokenType::True => (Some(Parser::bool), None, Precedence::Primary),
            TokenType::False => (Some(Parser::bool), None, Precedence::Primary),
            TokenType::Identifier => (Some(Parser::variable), None, Precedence::Primary),
            TokenType::LeftParenthese => {
                (Some(Parser::grouping), Some(Parser::call), Precedence::Call)
            }
            TokenType::Dot => (None, Some(Parser::method_call), Precedence::Call),
            TokenType::Scope => (None, Some(Parser::qualified_access), Precedence::Primary),
            TokenType::RightParenthese => (None, None, Precedence::None),
            TokenType::Comma => (None, None, Precedence::None),
            TokenType::Greater
            | TokenType::GreaterEqual
            | TokenType::Less
            | TokenType::LessEqual => (None, Some(Parser::binary), Precedence::Comparison),
            TokenType::EqualEqual | TokenType::BangEqual => {
                (None, Some(Parser::binary), Precedence::Equality)
            }
            TokenType::Equal
            | TokenType::PlusEqual
            | TokenType::MinusEqual
            | TokenType::StarEqual
            | TokenType::SlashEqual
            | TokenType::PercentEqual => (None, Some(Parser::assign), Precedence::AssignTest),
            TokenType::Semicolon => (None, None, Precedence::None),
            TokenType::LeftBrace => (Some(Parser::block_expression), None, Precedence::None),
            TokenType::RightBrace => (None, None, Precedence::None),
            TokenType::DotDot | TokenType::DotDotEqual => {
                (Some(Parser::unary), Some(Parser::binary), Precedence::Range)
            }
            TokenType::LeftSquare => (Some(Parser::array), Some(Parser::index), Precedence::Call),
            TokenType::RightSquare => (None, None, Precedence::None),
            TokenType::If => (Some(Parser::if_expression), None, Precedence::Call),
            // TODO: Convert variable declarations to expressions!!!
            TokenType::Let => (None, None, Precedence::None),
            TokenType::Const => (None, None, Precedence::None),
            TokenType::While
            | TokenType::For
            | TokenType::Fn
            | TokenType::Return
            | TokenType::Break
            | TokenType::Continue
            | TokenType::Import
            | TokenType::Struct
            | TokenType::Loop
            | TokenType::Colon
            | TokenType::Arrow
            | TokenType::In
            | TokenType::As
            | TokenType::DocComment
            | TokenType::Eof
            | TokenType::BinaryAnd
            | TokenType::BinaryOr
            | TokenType::Else
            | TokenType::Mut => (None, None, Precedence::None),
        })
    }
}
