use std::{any::Any, sync::Arc};

use crate::{
    Dynamic, ExecutionError, ExecutionErrorType, Executor, FuncInfo, Location, Module, Scope,
    Scoper, TempScuff, VariableState,
    parser::{AssignmentOp, BinaryOp, Expr, Stmt, UnaryOp, VariableMutability},
    scanner::{Token, TokenType},
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

impl Executor for ExecutionContext {}

impl ExecutionContext {
    pub fn new(module: Arc<Module>) -> ExecutionContext {
        ExecutionContext {
            scoper: Scoper::new(module, None),
        }
    }

    pub fn new_with_scope(module: Arc<Module>, scope: Scope) -> ExecutionContext {
        ExecutionContext {
            scoper: Scoper::new(module, Some(scope)),
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
            Stmt::Return(location, stmt) => {
                // A
                return Ok(if let Some(stmt) = stmt {
                    match self.statement(stmt)? {
                        StmtResult::None => {
                            return Err(ExecutionError::new(
                                *location,
                                ExecutionErrorType::NoValue,
                            ));
                        }
                        StmtResult::Value(dynamic) => StmtResult::Return(dynamic),
                        StmtResult::Return(dynamic) => StmtResult::Return(dynamic),
                    }
                } else {
                    StmtResult::Return(Dynamic::Nil)
                });
            }
            Stmt::Variable(name, initializer, mutability) => {
                self.define_variable(name, initializer, *mutability)?;
            }
            Stmt::Assign(name, op, assignment) => {
                self.assign(name, *op, assignment).map(|_| Dynamic::Nil)?;
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
            Stmt::Function(name, params, block) => {
                self.scoper.define_variable(
                    &name.lexeme,
                    VariableMutability::Constant,
                    Dynamic::Func(Box::new(Arc::new(FuncInfo {
                        name: name.lexeme.to_string(),
                        param_info: params
                            .iter()
                            .map(|v| (v.0, v.1.lexeme.to_string()))
                            .collect(),
                        call: TempScuff::FuncParse(block.clone()),
                    }))),
                    name.location,
                )?;
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
            .define_variable(&token.lexeme, mutability, value, token.location)?;

        Ok(())
    }

    fn expression(&mut self, expr: &Expr) -> Result<Dynamic, ExecutionError> {
        //println!("Expr: {expr:?}");
        match expr {
            Expr::Bool(_, val) => Ok(Dynamic::Bool(*val)),
            Expr::Float(_, val) => Ok(Dynamic::Float(*val)),
            Expr::Integer(_, val) => Ok(Dynamic::Integer(*val)),
            Expr::String(_, val) => Ok(Dynamic::String(val.clone())),
            Expr::Unary(op, expr) => self.unary(*op, expr),
            Expr::Binary(expr, op, expr1) => self.binary(op, expr, expr1),
            Expr::Condition(expr, token, expr1) => self.conditional(token, expr, expr1),
            Expr::Variable(location, identifier) => self.variable(*location, identifier),
            Expr::Nil(_) => Ok(Dynamic::Nil),
            Expr::Call(callee, params) => self.call(callee, params),
        }
    }

    fn call(&mut self, expr: &Expr, params: &[Box<Expr>]) -> Result<Dynamic, ExecutionError> {
        let callee = self.expression(expr)?;
        let mut evaled_params = Vec::with_capacity(params.len());
        for i in 0..params.len() {
            evaled_params.push(self.expression(&params[i])?);
        }

        match callee {
            Dynamic::NativeFunc(native_func_info) => {
                if params.len() != native_func_info.signature.len() {
                    return Err(ExecutionError::new(
                        expr.get_location(),
                        ExecutionErrorType::MismatchedNativeSignature(
                            native_func_info.param_info.clone(),
                            evaled_params,
                        ),
                    ));
                }

                for i in 0..evaled_params.len() {
                    if evaled_params[i].get_type() != native_func_info.signature[i] {
                        return Err(ExecutionError::new(
                            expr.get_location(),
                            ExecutionErrorType::MismatchedNativeSignature(
                                native_func_info.param_info.clone(),
                                evaled_params,
                            ),
                        ));
                    }
                }

                (native_func_info.call)(self, evaled_params).map(|v| v.unwrap_or(Dynamic::Nil))
            }
            Dynamic::Func(func_info) => {
                if params.len() != func_info.param_info.len() {
                    return Err(ExecutionError::new(
                        expr.get_location(),
                        ExecutionErrorType::MismatchedSignature(
                            func_info
                                .param_info
                                .iter()
                                .map(|v| v.1.to_string())
                                .collect(),
                            evaled_params,
                        ),
                    ));
                }

                // Push variables into a new frame with a new scope
                let mut evaled_params = evaled_params.into_iter();

                // We don't care about a return address, we're an AST walker.
                self.scoper.enter(0);

                // Stuff params
                for i in 0..params.len() {
                    let (mutability, name) = &func_info.param_info[i];
                    self.scoper.define_variable(
                        &name,
                        *mutability,
                        evaled_params.next().unwrap(),
                        params[i].get_location(),
                    )?;
                }

                // Execute
                let result = self.statement(func_info.call.unwrap_parse())?;

                // Dump the garbage
                self.scoper.exit();

                let result = match result {
                    StmtResult::None => return Ok(Dynamic::Nil),
                    StmtResult::Value(dynamic) => dynamic,
                    StmtResult::Return(dynamic) => dynamic,
                };

                Ok(result)
            }
            a => {
                return Err(ExecutionError::new(
                    expr.get_location(),
                    ExecutionErrorType::NotAFunction(a),
                ));
            }
        }
    }

    fn assign(&mut self, name: &str, op: AssignmentOp, stmt: &Stmt) -> Result<(), ExecutionError> {
        let value = match self.statement(stmt)? {
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
            .mut_variable(&name, stmt.get_location(), move |variable, location| {
                match op {
                    AssignmentOp::Equal => variable.value = value,
                    op => {
                        // Make sure it's the same variant, we don't care about value
                        if std::mem::discriminant(&variable.value) != std::mem::discriminant(&value)
                        {
                            return Err(ExecutionError::new(
                                location,
                                ExecutionErrorType::AssignmentOpInvalid(
                                    variable.value.clone(),
                                    op,
                                    value,
                                ),
                            ));
                        }

                        match &mut variable.value {
                            Dynamic::Integer(a) => match op {
                                AssignmentOp::PlusEqual => *a += value.unwrap_integer(),
                                AssignmentOp::MinusEqual => *a -= value.unwrap_integer(),
                                AssignmentOp::MultiplyEqual => *a *= value.unwrap_integer(),
                                AssignmentOp::DivideEqual => *a /= value.unwrap_integer(),
                                AssignmentOp::Equal => unreachable!(),
                            },
                            Dynamic::Float(a) => match op {
                                AssignmentOp::PlusEqual => *a += value.unwrap_float(),
                                AssignmentOp::MinusEqual => *a -= value.unwrap_float(),
                                AssignmentOp::MultiplyEqual => *a *= value.unwrap_float(),
                                AssignmentOp::DivideEqual => *a /= value.unwrap_float(),
                                AssignmentOp::Equal => unreachable!(),
                            },
                            Dynamic::String(a) => match op {
                                AssignmentOp::PlusEqual => *a += &value.unwrap_string(),
                                AssignmentOp::Equal => unreachable!(),
                                op => {
                                    return Err(ExecutionError::new(
                                        location,
                                        ExecutionErrorType::AssignmentOpInvalid(
                                            variable.value.clone(),
                                            op,
                                            value.clone(),
                                        ),
                                    ));
                                }
                            },
                            variable => {
                                return Err(ExecutionError::new(
                                    location,
                                    ExecutionErrorType::AssignmentOpInvalid(
                                        variable.clone(),
                                        op,
                                        value.clone(),
                                    ),
                                ));
                            }
                        }
                    }
                }

                Ok(())
            })
    }

    fn variable(
        &mut self,
        location: Location,
        identifier: &str,
    ) -> Result<Dynamic, ExecutionError> {
        self.scoper.get_variable(identifier, location)
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
        op: &BinaryOp,
        expr: &Expr,
        expr1: &Expr,
    ) -> Result<Dynamic, ExecutionError> {
        let left = self.expression(expr)?;
        let right = self.expression(expr1)?;
        Ok(match op {
            BinaryOp::Minus => match (left, right) {
                (Dynamic::Integer(a), Dynamic::Integer(b)) => Dynamic::Integer(a - b),
                (Dynamic::Float(a), Dynamic::Float(b)) => Dynamic::Float(a - b),
                (a, b) => {
                    return Err(ExecutionError::new(
                        expr.get_location(),
                        ExecutionErrorType::InvalidOperator(a, *op, b),
                    ));
                }
            },
            BinaryOp::Plus => match (left, right) {
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
                        expr.get_location(),
                        ExecutionErrorType::InvalidOperator(a, *op, b),
                    ));
                }
            },
            BinaryOp::Multiply => match (left, right) {
                (Dynamic::Integer(a), Dynamic::Integer(b)) => Dynamic::Integer(a * b),
                (Dynamic::Float(a), Dynamic::Float(b)) => Dynamic::Float(a * b),
                (a, b) => {
                    return Err(ExecutionError::new(
                        expr.get_location(),
                        ExecutionErrorType::InvalidOperator(a, *op, b),
                    ));
                }
            },
            BinaryOp::Divide => match (left, right) {
                (Dynamic::Integer(a), Dynamic::Integer(b)) => {
                    if b != 0 {
                        Dynamic::Integer(a / b)
                    } else {
                        return Err(ExecutionError::new(
                            expr.get_location(),
                            ExecutionErrorType::DivideByZero,
                        ));
                    }
                }
                (Dynamic::Float(a), Dynamic::Float(b)) => {
                    if b != 0. {
                        Dynamic::Float(a / b)
                    } else {
                        return Err(ExecutionError::new(
                            expr.get_location(),
                            ExecutionErrorType::DivideByZero,
                        ));
                    }
                }
                (a, b) => {
                    return Err(ExecutionError::new(
                        expr.get_location(),
                        ExecutionErrorType::InvalidOperator(a, *op, b),
                    ));
                }
            },
            BinaryOp::Greater => match (left, right) {
                (Dynamic::Integer(a), Dynamic::Integer(b)) => Dynamic::Bool(a > b),
                (Dynamic::Float(a), Dynamic::Float(b)) => Dynamic::Bool(a > b),
                (a, b) => {
                    return Err(ExecutionError::new(
                        expr.get_location(),
                        ExecutionErrorType::InvalidOperator(a, *op, b),
                    ));
                }
            },
            BinaryOp::GreaterEqual => match (left, right) {
                (Dynamic::Integer(a), Dynamic::Integer(b)) => Dynamic::Bool(a >= b),
                (Dynamic::Float(a), Dynamic::Float(b)) => Dynamic::Bool(a >= b),
                (a, b) => {
                    return Err(ExecutionError::new(
                        expr.get_location(),
                        ExecutionErrorType::InvalidOperator(a, *op, b),
                    ));
                }
            },
            BinaryOp::Less => match (left, right) {
                (Dynamic::Integer(a), Dynamic::Integer(b)) => Dynamic::Bool(a < b),
                (Dynamic::Float(a), Dynamic::Float(b)) => Dynamic::Bool(a < b),
                (a, b) => {
                    return Err(ExecutionError::new(
                        expr.get_location(),
                        ExecutionErrorType::InvalidOperator(a, *op, b),
                    ));
                }
            },
            BinaryOp::LessEqual => match (left, right) {
                (Dynamic::Integer(a), Dynamic::Integer(b)) => Dynamic::Bool(a <= b),
                (Dynamic::Float(a), Dynamic::Float(b)) => Dynamic::Bool(a <= b),
                (a, b) => {
                    return Err(ExecutionError::new(
                        expr.get_location(),
                        ExecutionErrorType::InvalidOperator(a, *op, b),
                    ));
                }
            },
            BinaryOp::EqualEqual => Dynamic::Bool(left == right),
            BinaryOp::BangEqual => Dynamic::Bool(left != right),
        })
    }

    fn unary(&mut self, op: UnaryOp, expr: &Expr) -> Result<Dynamic, ExecutionError> {
        let left = self.expression(expr)?;
        Ok(match op {
            UnaryOp::Negate => match left {
                Dynamic::Integer(val) => Dynamic::Integer(-val),
                Dynamic::Float(val) => Dynamic::Float(-val),
                a => {
                    return Err(ExecutionError::new(
                        expr.get_location(),
                        ExecutionErrorType::InvalidUnaryOperator(op, a),
                    ));
                }
            },
            UnaryOp::Invert => match left {
                Dynamic::Bool(val) => Dynamic::Bool(!val),
                a => {
                    return Err(ExecutionError::new(
                        expr.get_location(),
                        ExecutionErrorType::InvalidUnaryOperator(op, a),
                    ));
                }
            },
        })
    }
}
