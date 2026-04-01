use std::collections::HashMap;

use crate::{
    Dynamic, ExecutionError, ExecutionErrorType, Location, Scope, Scoper,
    parser::{Expr, Parser, Stmt, VariableMutability},
    scanner::{Scanner, Token, TokenType},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StmtResult {
    None,
    Value(Dynamic),
    Return(Dynamic),
}

pub struct ExecutionContext {
    scoper: Scoper,
}

impl ExecutionContext {
    pub fn new() -> ExecutionContext {
        ExecutionContext {
            scoper: Scoper::new(None),
        }
    }

    pub fn new_with_scope(scope: Scope) -> ExecutionContext {
        ExecutionContext {
            scoper: Scoper::new(Some(scope)),
        }
    }

    pub fn run(mut self, stmts: &[Stmt]) -> Result<Option<Dynamic>, ExecutionError> {
        Ok(match self.statements(stmts)? {
            StmtResult::None => None,
            StmtResult::Value(dynamic) => Some(dynamic),
            StmtResult::Return(dynamic) => Some(dynamic),
        })
    }

    fn statements(&mut self, stmts: &[Stmt]) -> Result<StmtResult, ExecutionError> {
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
                self.scoper.push(None);

                last = self.statements(stmts)?;

                // Went out of scope
                self.scoper.pop();
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

        self.scoper
            .define_variable(&token.lexeme, mutability, value, &token.location)?;

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

        self.scoper
            .assign_variable(&token.lexeme, result, &token.location)
    }

    fn variable(&mut self, token: &Token) -> Result<Dynamic, ExecutionError> {
        self.scoper.get_variable(&token.lexeme, &token.location)
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
                (Dynamic::String(a), Dynamic::Integer(b)) => {
                    Dynamic::String(a + b.to_string().as_str())
                }
                (Dynamic::String(a), Dynamic::Float(b)) => {
                    Dynamic::String(a + b.to_string().as_str())
                }
                (Dynamic::String(a), Dynamic::Bool(b)) => {
                    Dynamic::String(a + b.to_string().as_str())
                }
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
