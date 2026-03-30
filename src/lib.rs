mod parser;
mod scanner;

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
        let result = Walker::default().run(stmts);
        println!("Result: {result:?}");
    } else {
        println!("Failed to get statements.");
    }
}

#[derive(Debug)]
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
struct ExecutionError {
    location: Location,
    message: String,
}

impl ExecutionError {
    pub fn new(location: Location, message: &str) -> ExecutionError {
        ExecutionError {
            location,
            message: message.to_string(),
        }
    }
}

#[derive(Default)]
struct Walker {
    //execution_errors: Vec<String>,
}

impl Walker {
    pub fn run(&mut self, stmts: &[Stmt]) -> Result<Dynamic, ExecutionError> {
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
            }
        }

        Ok(last.unwrap_or(Dynamic::Nil))
    }

    fn step(&mut self, expr: &Expr) -> Result<Dynamic, ExecutionError> {
        //println!("Expr: {expr:?}");
        match expr {
            Expr::Bool(val) => Ok(Dynamic::Bool(*val)),
            Expr::Float(val) => Ok(Dynamic::Float(*val)),
            Expr::Integer(val) => Ok(Dynamic::Integer(*val)),
            Expr::String(val) => Ok(Dynamic::String(val.clone())),
            Expr::Unary(token, expr) => self.unary(token, expr),
            Expr::Binary(expr, token, expr1) => self.binary(token, expr, expr1),
            a => unimplemented!("{a:?}"),
        }
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
                        &format!(
                            "Value A ({a:?}) and B ({b:?}) have no binary operator for '{token:?}'"
                        ),
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
                        &format!(
                            "Value A ({a:?}) and B ({b:?}) have no binary operator for '{token:?}'"
                        ),
                    ));
                }
            },
            TokenType::Star => match (left, right) {
                (Dynamic::Integer(a), Dynamic::Integer(b)) => Dynamic::Integer(a * b),
                (Dynamic::Float(a), Dynamic::Float(b)) => Dynamic::Float(a * b),
                (a, b) => {
                    return Err(ExecutionError::new(
                        token.location.clone(),
                        &format!(
                            "Value A ({a:?}) and B ({b:?}) have no binary operator for '{token:?}'"
                        ),
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
                            "Divide by zero.",
                        ));
                    }
                }
                (Dynamic::Float(a), Dynamic::Float(b)) => {
                    if b != 0. {
                        Dynamic::Float(a / b)
                    } else {
                        return Err(ExecutionError::new(
                            token.location.clone(),
                            "Divide by zero.",
                        ));
                    }
                }
                (a, b) => {
                    return Err(ExecutionError::new(
                        token.location.clone(),
                        &format!(
                            "Value A ({a:?}) and B ({b:?}) have no binary operator for '{token:?}'"
                        ),
                    ));
                }
            },
            TokenType::Greater => match (left, right) {
                (Dynamic::Integer(a), Dynamic::Integer(b)) => Dynamic::Bool(a > b),
                (Dynamic::Float(a), Dynamic::Float(b)) => Dynamic::Bool(a > b),
                (a, b) => {
                    return Err(ExecutionError::new(
                        token.location.clone(),
                        &format!(
                            "Value A ({a:?}) and B ({b:?}) have no binary operator for '{token:?}'"
                        ),
                    ));
                }
            },
            TokenType::GreaterEqual => match (left, right) {
                (Dynamic::Integer(a), Dynamic::Integer(b)) => Dynamic::Bool(a >= b),
                (Dynamic::Float(a), Dynamic::Float(b)) => Dynamic::Bool(a >= b),
                (a, b) => {
                    return Err(ExecutionError::new(
                        token.location.clone(),
                        &format!(
                            "Value A ({a:?}) and B ({b:?}) have no binary operator for '{token:?}'"
                        ),
                    ));
                }
            },
            TokenType::Less => match (left, right) {
                (Dynamic::Integer(a), Dynamic::Integer(b)) => Dynamic::Bool(a < b),
                (Dynamic::Float(a), Dynamic::Float(b)) => Dynamic::Bool(a < b),
                (a, b) => {
                    return Err(ExecutionError::new(
                        token.location.clone(),
                        &format!(
                            "Value A ({a:?}) and B ({b:?}) have no binary operator for '{token:?}'"
                        ),
                    ));
                }
            },
            TokenType::LessEqual => match (left, right) {
                (Dynamic::Integer(a), Dynamic::Integer(b)) => Dynamic::Bool(a <= b),
                (Dynamic::Float(a), Dynamic::Float(b)) => Dynamic::Bool(a <= b),
                (a, b) => {
                    return Err(ExecutionError::new(
                        token.location.clone(),
                        &format!(
                            "Value A ({a:?}) and B ({b:?}) have no binary operator for '{token:?}'"
                        ),
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
                        &format!("Value A ({a:?}) has no unary operator for '{token:?}'"),
                    ));
                }
            },
            TokenType::Bang => match left {
                Dynamic::Bool(val) => Dynamic::Bool(!val),
                a => {
                    return Err(ExecutionError::new(
                        token.location.clone(),
                        &format!("Value A ({a:?}) has no unary operator for '{token:?}'"),
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
