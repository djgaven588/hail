mod parser;
mod scanner;

use std::collections::HashMap;

use crate::{
    parser::{Expr, Parser, Stmt},
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
    mutable: bool,
}

impl VariableState {
    pub fn new(value: Dynamic, mutable: bool) -> VariableState {
        VariableState { value, mutable }
    }
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

    pub fn run(mut self, stmts: &[Stmt]) -> Result<Dynamic, ExecutionError> {
        self.statements(stmts)
    }

    pub fn statements(&mut self, stmts: &[Stmt]) -> Result<Dynamic, ExecutionError> {
        let mut last = None;
        for stmt in stmts {
            last = None;
            match stmt {
                Stmt::Expression(expr) => {
                    self.step(expr)?;
                }
                Stmt::Print(expr) => {
                    println!("Print: {:?}", self.step(expr)?)
                }
                Stmt::Return(expr) => return self.step(expr),
                Stmt::Variable(name, initializer, mutable) => {
                    let value = if let Some(initializer) = initializer {
                        self.step(initializer)?
                    } else {
                        Dynamic::Nil
                    };
                    self.scopes
                        .last_mut()
                        .expect("A scope should always exist.")
                        .variables
                        .insert(name.to_string(), VariableState::new(value, *mutable));
                }
                Stmt::Assign(name, assignment) => {
                    self.assign(name, assignment).map(|_| Dynamic::Nil)?;
                }
                Stmt::Block(stmts) => {
                    self.scopes.push(Scoped::default());

                    last = Some(self.statements(stmts)?);

                    // Went out of scope
                    let _ = self.scopes.pop();
                }
            }
        }

        Ok(last.unwrap_or(Dynamic::Nil))
    }

    fn step(&mut self, expr: &Expr) -> Result<Dynamic, ExecutionError> {
        println!("Expr: {expr:?}");
        match expr {
            Expr::Bool(val) => Ok(Dynamic::Bool(*val)),
            Expr::Float(val) => Ok(Dynamic::Float(*val)),
            Expr::Integer(val) => Ok(Dynamic::Integer(*val)),
            Expr::String(val) => Ok(Dynamic::String(val.clone())),
            Expr::Unary(token, expr) => self.unary(token, expr),
            Expr::Binary(expr, token, expr1) => self.binary(token, expr, expr1),
            Expr::Variable(identifier) => self.variable(identifier),
            a => unimplemented!("{a:?}"),
        }
    }

    fn assign(&mut self, token: &Token, expr: &Expr) -> Result<(), ExecutionError> {
        let result = self.step(expr)?;

        let len = self.scopes.len();
        for i in 0..len {
            let i = len - i - 1;
            if let Some(found) = self.scopes[i].variables.get_mut(&token.lexeme) {
                if !found.mutable {
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

    fn binary(
        &mut self,
        token: &Token,
        expr: &Expr,
        expr1: &Expr,
    ) -> Result<Dynamic, ExecutionError> {
        let left = self.step(expr)?;
        let right = self.step(expr1)?;
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
        let left = self.step(expr)?;
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
