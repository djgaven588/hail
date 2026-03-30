mod parser;
mod scanner;

use crate::{
    parser::{Expr, Parser},
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
    if let Some(expr) = parser.test_expression() {
        println!("Expr:\n{}", expr.display(0));

        println!("Running...");
        let result = Walker::default().run(expr);
        println!("Result: {result:?}");
    } else {
        println!("Failed to get expression.");
    }

    println!("Parse Errors:");
    for error in parser.errors() {
        println!("Error: {error:?}");
    }
}

#[derive(Debug)]
enum Dynamic {
    Integer(i64),
    Float(f64),
}

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
    pub fn run(&mut self, expr: Expr) -> Result<Dynamic, ExecutionError> {
        self.step(&expr)
    }

    fn step(&mut self, expr: &Expr) -> Result<Dynamic, ExecutionError> {
        println!("Expr: {expr:?}");
        match expr {
            Expr::Float(val) => Ok(Dynamic::Float(*val)),
            Expr::Integer(val) => Ok(Dynamic::Integer(*val)),
            Expr::Unary(token, expr) => self.unary(token, expr),
            Expr::Binary(expr, token, expr1) => self.binary(token, expr, expr1),
            Expr::Grouping(expr) => self.step(expr),
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
                (Dynamic::Integer(a), Dynamic::Integer(b)) => Dynamic::Integer(a / b),
                (Dynamic::Float(a), Dynamic::Float(b)) => Dynamic::Float(a / b),
                (a, b) => {
                    return Err(ExecutionError::new(
                        token.location.clone(),
                        &format!(
                            "Value A ({a:?}) and B ({b:?}) have no binary operator for '{token:?}'"
                        ),
                    ));
                }
            },
            a => unimplemented!("{a:?}"),
        })
    }

    fn unary(&mut self, token: &Token, expr: &Expr) -> Result<Dynamic, ExecutionError> {
        let left = self.step(expr)?;
        Ok(match token.token_type {
            TokenType::Minus => match left {
                Dynamic::Integer(val) => Dynamic::Integer(-val),
                Dynamic::Float(val) => Dynamic::Float(-val),
            },
            a => unimplemented!("{a:?}"),
        })
    }
}

const KEYWORDS: [(&str, TokenType); 11] = [
    ("let", TokenType::Let),
    ("if", TokenType::If),
    ("else", TokenType::Else),
    ("while", TokenType::While),
    ("fn", TokenType::Fn),
    ("return", TokenType::Return),
    ("nil", TokenType::Nil),
    ("true", TokenType::True),
    ("false", TokenType::False),
    ("struct", TokenType::Struct),
    ("import", TokenType::Import),
];

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
