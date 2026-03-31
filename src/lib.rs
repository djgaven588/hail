mod parser;
mod scanner;

use std::collections::HashMap;

use crate::{
    parser::{Expr, Parser, Stmt, VariableMutability},
    scanner::{Scanner, Token, TokenType},
};

pub fn run(source: String) {
    let mut scanner = Scanner::new(source);
    scanner.scan();

    println!("Tokens:");
    for token in scanner.get().unwrap() {
        println!("Token: {token:?}");
    }

    println!("Syntax Errors:");
    for error in scanner.errors() {
        println!("Error: {error:?}");
    }

    let mut parser = Parser::new(scanner.get().unwrap());
    parser.parse();
    println!("Parse Errors:");
    for error in parser.errors() {
        println!("Error: {error:?}");
    }

    if let Some(stmts) = parser.get() {
        println!("Running...");
        let result = ExecutionContext::new().run(stmts);
        println!("Result: {result:?}");
    } else {
        println!("Failed to get statements.");
    }
}

#[derive(Debug, Clone)]
enum Dynamic {
    Bool(bool),
    Integer(i64),
    Float(f64),
    String(String),
    Nil,
}

impl PartialEq for Dynamic {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Bool(l0), Self::Bool(r0)) => l0 == r0,
            (Self::Integer(l0), Self::Integer(r0)) => l0 == r0,
            (Self::Float(l0), Self::Float(r0)) => l0 == r0,
            (Self::String(l0), Self::String(r0)) => l0 == r0,
            _ => false,
        }
    }
}

impl Eq for Dynamic {}

#[derive(Debug)]
enum ExecutionErrorType {
    VariableImmutable(String),
    VariableUndefined(String),
    InvalidOperator(Dynamic, Token, Dynamic),
    InvalidUnaryOperator(Token, Dynamic),
    DivideByZero,
    NoValue,
    UnexpectedReturningStatement,
    RedefinedConstant,
    ExpectedBoolean,
}

#[derive(Debug)]
struct ExecutionError {
    location: Location,
    error_type: ExecutionErrorType,
}

impl ExecutionError {
    pub fn new(location: Location, error_type: ExecutionErrorType) -> ExecutionError {
        ExecutionError {
            location,
            error_type,
        }
    }
}

struct VariableState {
    value: Dynamic,
    mutability: VariableMutability,
}

impl VariableState {
    pub fn new(value: Dynamic, mutability: VariableMutability) -> VariableState {
        VariableState { value, mutability }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum StmtResult {
    None,
    Value(Dynamic),
    Return(Dynamic),
}

#[derive(Default)]
struct Scoped {
    variables: HashMap<String, VariableState>,
}

struct ExecutionContext {
    scopes: Vec<Scoped>,
}

impl ExecutionContext {
    pub fn new() -> ExecutionContext {
        ExecutionContext {
            scopes: vec![Scoped::default()],
        }
    }

    pub fn new_with_scope(scope: Scoped) -> ExecutionContext {
        ExecutionContext {
            scopes: vec![scope],
        }
    }

    pub fn run(mut self, stmts: &[Stmt]) -> Result<Option<Dynamic>, ExecutionError> {
        Ok(match self.statements(stmts)? {
            StmtResult::None => None,
            StmtResult::Value(dynamic) => Some(dynamic),
            StmtResult::Return(dynamic) => Some(dynamic),
        })
    }

    pub fn statements(&mut self, stmts: &[Stmt]) -> Result<StmtResult, ExecutionError> {
        let mut last = None;
        for stmt in stmts {
            last = match self.statement(stmt)? {
                StmtResult::None => None,
                StmtResult::Value(dynamic) => Some(dynamic),
                StmtResult::Return(dynamic) => {
                    return Ok(StmtResult::Return(dynamic));
                }
            };
        }

        Ok(if let Some(last) = last {
            StmtResult::Value(last)
        } else {
            StmtResult::None
        })
    }

    fn statement(&mut self, stmt: &Stmt) -> Result<StmtResult, ExecutionError> {
        let mut last = StmtResult::None;
        match stmt {
            Stmt::Expression(expr, can_return) => {
                let result = self.expression(expr)?;
                if *can_return {
                    last = StmtResult::Value(result);
                }
            }
            Stmt::Print(stmt) => {
                println!("Print: {:?}", self.statement(stmt)?)
            }
            Stmt::Return(stmt) => {
                // A
                match self.statement(stmt)? {
                    StmtResult::None => {
                        return Err(ExecutionError::new(
                            stmt.get_location(),
                            ExecutionErrorType::NoValue,
                        ));
                    }
                    StmtResult::Value(dynamic) => return Ok(StmtResult::Return(dynamic)),
                    StmtResult::Return(dynamic) => return Ok(StmtResult::Return(dynamic)),
                }
            }
            Stmt::Variable(name, initializer, mutability) => {
                self.define_variable(name, initializer, *mutability)?;
            }
            Stmt::Assign(name, assignment) => {
                self.assign(name, assignment).map(|_| Dynamic::Nil)?;
            }
            Stmt::Block(stmts) => {
                self.scopes.push(Scoped::default());

                last = self.statements(stmts)?;

                // Went out of scope
                let _ = self.scopes.pop();
            }
            Stmt::If(expr, body, otherwise) => {
                // Get the expression's value to see which branch to take
                let result = match self.statement(expr)? {
                    StmtResult::None => {
                        return Err(ExecutionError::new(
                            expr.get_location(),
                            ExecutionErrorType::ExpectedBoolean,
                        ));
                    }
                    StmtResult::Value(dynamic) => dynamic,
                    StmtResult::Return(_) => {
                        return Err(ExecutionError::new(
                            expr.get_location(),
                            ExecutionErrorType::UnexpectedReturningStatement,
                        ));
                    }
                };

                // If it's not a bool, this is an error
                // No "truthy" bullshit, it's true, false, or not a bool.
                if !matches!(result, Dynamic::Bool(_)) {
                    return Err(ExecutionError::new(
                        expr.get_location(),
                        ExecutionErrorType::ExpectedBoolean,
                    ));
                }

                if result == Dynamic::Bool(true) {
                    last = self.statement(body)?;
                } else if let Some(otherwise) = otherwise {
                    last = self.statement(otherwise)?;
                }
            }
            Stmt::While(condition, body) => {
                while {
                    // Get the expression's value to see which branch to take
                    let result = match self.statement(condition)? {
                        StmtResult::None => {
                            return Err(ExecutionError::new(
                                condition.get_location(),
                                ExecutionErrorType::ExpectedBoolean,
                            ));
                        }
                        StmtResult::Value(dynamic) => dynamic,
                        StmtResult::Return(_) => {
                            return Err(ExecutionError::new(
                                condition.get_location(),
                                ExecutionErrorType::UnexpectedReturningStatement,
                            ));
                        }
                    };

                    // If it's not a bool, this is an error
                    // No "truthy" bullshit, it's true, false, or not a bool.
                    if !matches!(result, Dynamic::Bool(_)) {
                        return Err(ExecutionError::new(
                            condition.get_location(),
                            ExecutionErrorType::ExpectedBoolean,
                        ));
                    }

                    result == Dynamic::Bool(true)
                } {
                    last = self.statement(body)?;
                }
            }
        };

        Ok(last)
    }

    fn define_variable(
        &mut self,
        token: &Token,
        initializer: &Option<Box<Stmt>>,
        mutability: VariableMutability,
    ) -> Result<(), ExecutionError> {
        let value = if let Some(initializer) = initializer {
            match self.statement(initializer)? {
                StmtResult::None => {
                    return Err(ExecutionError::new(
                        initializer.get_location(),
                        ExecutionErrorType::NoValue,
                    ));
                }
                StmtResult::Value(dynamic) => dynamic,
                StmtResult::Return(_) => {
                    return Err(ExecutionError::new(
                        initializer.get_location(),
                        ExecutionErrorType::UnexpectedReturningStatement,
                    ));
                }
            }
        } else {
            Dynamic::Nil
        };

        // If it's a constant, make sure we're not bypassing the fact it's a constant by redefining it
        // Constants are still *scoped*, this is more of a "enforce good behavior" that can be removed if needed
        let len = self.scopes.len();
        for i in 0..len {
            let i = len - i - 1;
            if let Some(found) = self.scopes[i].variables.get_mut(&token.lexeme)
                && found.mutability == VariableMutability::Constant
            {
                return Err(ExecutionError::new(
                    token.location.clone(),
                    ExecutionErrorType::RedefinedConstant,
                ));
            }
        }

        self.scopes
            .last_mut()
            .expect("A scope should always exist.")
            .variables
            .insert(
                token.lexeme.to_string(),
                VariableState::new(value, mutability),
            );
        Ok(())
    }

    fn expression(&mut self, expr: &Expr) -> Result<Dynamic, ExecutionError> {
        //println!("Expr: {expr:?}");
        match expr {
            Expr::Bool(_, val) => Ok(Dynamic::Bool(*val)),
            Expr::Float(_, val) => Ok(Dynamic::Float(*val)),
            Expr::Integer(_, val) => Ok(Dynamic::Integer(*val)),
            Expr::String(_, val) => Ok(Dynamic::String(val.clone())),
            Expr::Unary(token, expr) => self.unary(token, expr),
            Expr::Binary(expr, token, expr1) => self.binary(token, expr, expr1),
            Expr::Condition(expr, token, expr1) => self.conditional(token, expr, expr1),
            Expr::Variable(identifier) => self.variable(identifier),
            Expr::Nil(_) => Ok(Dynamic::Nil),
            a => unimplemented!("{a:?}"),
        }
    }

    fn assign(&mut self, token: &Token, stmt: &Stmt) -> Result<(), ExecutionError> {
        let result = match self.statement(stmt)? {
            StmtResult::None => {
                return Err(ExecutionError::new(
                    stmt.get_location(),
                    ExecutionErrorType::NoValue,
                ));
            }
            StmtResult::Value(dynamic) => dynamic,
            StmtResult::Return(_) => {
                return Err(ExecutionError::new(
                    stmt.get_location(),
                    ExecutionErrorType::UnexpectedReturningStatement,
                ));
            }
        };

        let len = self.scopes.len();
        for i in 0..len {
            let i = len - i - 1;
            if let Some(found) = self.scopes[i].variables.get_mut(&token.lexeme) {
                if found.mutability != VariableMutability::Mutable {
                    return Err(ExecutionError::new(
                        token.location.clone(),
                        ExecutionErrorType::VariableImmutable(token.lexeme.clone()),
                    ));
                }
                found.value = result;
                return Ok(());
            }
        }

        Err(ExecutionError::new(
            token.location.clone(),
            ExecutionErrorType::VariableUndefined(token.lexeme.clone()),
        ))
    }

    fn variable(&mut self, token: &Token) -> Result<Dynamic, ExecutionError> {
        let len = self.scopes.len();
        for i in 0..len {
            let i = len - i - 1;
            if let Some(found) = self.scopes[i].variables.get_mut(&token.lexeme) {
                return Ok(found.value.clone());
            }
        }

        Err(ExecutionError::new(
            token.location.clone(),
            ExecutionErrorType::VariableUndefined(token.lexeme.clone()),
        ))
    }

    fn conditional(
        &mut self,
        token: &Token,
        expr: &Expr,
        expr1: &Expr,
    ) -> Result<Dynamic, ExecutionError> {
        let Dynamic::Bool(left) = self.expression(expr)? else {
            return Err(ExecutionError::new(
                expr.get_location(),
                ExecutionErrorType::ExpectedBoolean,
            ));
        };

        // A conditional "short circuits" it's expression if it's a known outcome
        match token.token_type {
            TokenType::And => {
                if !left {
                    return Ok(Dynamic::Bool(false));
                }
            }
            TokenType::Or => {
                if left {
                    return Ok(Dynamic::Bool(true));
                }
            }
            a => unimplemented!("{a:?}"),
        }

        // Evaluate right hand side
        let Dynamic::Bool(right) = self.expression(expr1)? else {
            return Err(ExecutionError::new(
                expr1.get_location(),
                ExecutionErrorType::ExpectedBoolean,
            ));
        };

        // Return
        Ok(Dynamic::Bool(match token.token_type {
            TokenType::And => left && right,
            TokenType::Or => left || right,
            a => unimplemented!("{a:?}"),
        }))
    }

    fn binary(
        &mut self,
        token: &Token,
        expr: &Expr,
        expr1: &Expr,
    ) -> Result<Dynamic, ExecutionError> {
        let left = self.expression(expr)?;
        let right = self.expression(expr1)?;
        Ok(match token.token_type {
            TokenType::Minus => match (left, right) {
                (Dynamic::Integer(a), Dynamic::Integer(b)) => Dynamic::Integer(a - b),
                (Dynamic::Float(a), Dynamic::Float(b)) => Dynamic::Float(a - b),
                (a, b) => {
                    return Err(ExecutionError::new(
                        token.location.clone(),
                        ExecutionErrorType::InvalidOperator(a, token.clone(), b),
                    ));
                }
            },
            TokenType::Plus => match (left, right) {
                (Dynamic::Integer(a), Dynamic::Integer(b)) => Dynamic::Integer(a + b),
                (Dynamic::Float(a), Dynamic::Float(b)) => Dynamic::Float(a + b),
                (Dynamic::String(a), Dynamic::Integer(b)) => Dynamic::String(a + &b.to_string()),
                (Dynamic::String(a), Dynamic::Float(b)) => Dynamic::String(a + &b.to_string()),
                (Dynamic::String(a), Dynamic::Bool(b)) => Dynamic::String(a + &b.to_string()),
                (Dynamic::String(a), Dynamic::String(b)) => Dynamic::String(a + b.as_str()),
                (a, b) => {
                    return Err(ExecutionError::new(
                        token.location.clone(),
                        ExecutionErrorType::InvalidOperator(a, token.clone(), b),
                    ));
                }
            },
            TokenType::Star => match (left, right) {
                (Dynamic::Integer(a), Dynamic::Integer(b)) => Dynamic::Integer(a * b),
                (Dynamic::Float(a), Dynamic::Float(b)) => Dynamic::Float(a * b),
                (a, b) => {
                    return Err(ExecutionError::new(
                        token.location.clone(),
                        ExecutionErrorType::InvalidOperator(a, token.clone(), b),
                    ));
                }
            },
            TokenType::Slash => match (left, right) {
                (Dynamic::Integer(a), Dynamic::Integer(b)) => {
                    if b != 0 {
                        Dynamic::Integer(a / b)
                    } else {
                        return Err(ExecutionError::new(
                            token.location.clone(),
                            ExecutionErrorType::DivideByZero,
                        ));
                    }
                }
                (Dynamic::Float(a), Dynamic::Float(b)) => {
                    if b != 0. {
                        Dynamic::Float(a / b)
                    } else {
                        return Err(ExecutionError::new(
                            token.location.clone(),
                            ExecutionErrorType::DivideByZero,
                        ));
                    }
                }
                (a, b) => {
                    return Err(ExecutionError::new(
                        token.location.clone(),
                        ExecutionErrorType::InvalidOperator(a, token.clone(), b),
                    ));
                }
            },
            TokenType::Greater => match (left, right) {
                (Dynamic::Integer(a), Dynamic::Integer(b)) => Dynamic::Bool(a > b),
                (Dynamic::Float(a), Dynamic::Float(b)) => Dynamic::Bool(a > b),
                (a, b) => {
                    return Err(ExecutionError::new(
                        token.location.clone(),
                        ExecutionErrorType::InvalidOperator(a, token.clone(), b),
                    ));
                }
            },
            TokenType::GreaterEqual => match (left, right) {
                (Dynamic::Integer(a), Dynamic::Integer(b)) => Dynamic::Bool(a >= b),
                (Dynamic::Float(a), Dynamic::Float(b)) => Dynamic::Bool(a >= b),
                (a, b) => {
                    return Err(ExecutionError::new(
                        token.location.clone(),
                        ExecutionErrorType::InvalidOperator(a, token.clone(), b),
                    ));
                }
            },
            TokenType::Less => match (left, right) {
                (Dynamic::Integer(a), Dynamic::Integer(b)) => Dynamic::Bool(a < b),
                (Dynamic::Float(a), Dynamic::Float(b)) => Dynamic::Bool(a < b),
                (a, b) => {
                    return Err(ExecutionError::new(
                        token.location.clone(),
                        ExecutionErrorType::InvalidOperator(a, token.clone(), b),
                    ));
                }
            },
            TokenType::LessEqual => match (left, right) {
                (Dynamic::Integer(a), Dynamic::Integer(b)) => Dynamic::Bool(a <= b),
                (Dynamic::Float(a), Dynamic::Float(b)) => Dynamic::Bool(a <= b),
                (a, b) => {
                    return Err(ExecutionError::new(
                        token.location.clone(),
                        ExecutionErrorType::InvalidOperator(a, token.clone(), b),
                    ));
                }
            },
            TokenType::EqualEqual => Dynamic::Bool(left == right),
            TokenType::BangEqual => Dynamic::Bool(left != right),
            a => unimplemented!("{a:?}"),
        })
    }

    fn unary(&mut self, token: &Token, expr: &Expr) -> Result<Dynamic, ExecutionError> {
        let left = self.expression(expr)?;
        Ok(match token.token_type {
            TokenType::Minus => match left {
                Dynamic::Integer(val) => Dynamic::Integer(-val),
                Dynamic::Float(val) => Dynamic::Float(-val),
                a => {
                    return Err(ExecutionError::new(
                        token.location.clone(),
                        ExecutionErrorType::InvalidUnaryOperator(token.clone(), a),
                    ));
                }
            },
            TokenType::Bang => match left {
                Dynamic::Bool(val) => Dynamic::Bool(!val),
                a => {
                    return Err(ExecutionError::new(
                        token.location.clone(),
                        ExecutionErrorType::InvalidUnaryOperator(token.clone(), a),
                    ));
                }
            },
            a => unimplemented!("{a:?}"),
        })
    }
}

#[derive(Debug, Clone)]
struct Location {
    line: usize,
    column: usize,
    length: usize,
}

impl Location {
    pub fn new(line: usize, column: usize, length: usize) -> Location {
        Location {
            line,
            column,
            length,
        }
    }
}
