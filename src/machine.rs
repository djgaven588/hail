use std::{any::Any, sync::Arc};

use crate::{
    Dynamic, ExecutionError, ExecutionErrorType, Executor, Location, Module, Scope, Scoper,
    parser::{AssignmentOp, BinaryOp, Expr, Stmt, UnaryOp, VariableMutability},
};

#[derive(Debug)]
struct Instruction {
    location: Location,
    value: InstructionType,
}

impl Instruction {
    pub fn new(location: Location, value: InstructionType) -> Instruction {
        Instruction { location, value }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum InstructionType {
    // Weirdo
    Print,

    // Values
    Constant(Dynamic),
    Variable(String),

    // Ops
    Unary(UnaryOp),
    Binary(BinaryOp),

    // Variables
    // Name, has initializer, mutability
    DefineVariable(String, bool, VariableMutability),
    ModifyVariable(String, AssignmentOp),

    // Jumps
    JumpOnFalse(usize),
    Jump(usize),

    // Manage stack
    PushScope,
    PopScope,

    // Call a function with X parameters
    Call(usize),
}

pub struct Vm {
    program: Vec<Instruction>,
    module: Arc<Module>,
}

impl Default for Vm {
    fn default() -> Self {
        Self {
            program: Default::default(),
            module: Arc::new(Module::default()),
        }
    }
}

impl Vm {
    pub fn new(module: Arc<Module>, stmts: &[Stmt]) -> Vm {
        let mut vm = Vm {
            program: vec![],
            module,
        };

        vm.statements(stmts);

        //println!("VM Instructions:");
        //for instruction in &vm.program {
        //    println!("{instruction:?}");
        //}

        vm
    }

    fn statements(&mut self, stmts: &[Stmt]) {
        for stmt in stmts {
            self.statement(stmt);
        }
    }

    fn statement(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Variable(token, stmt, variable_mutability) => {
                // Stmt comes first as it needs to go on the stack
                if let Some(stmt) = stmt {
                    self.statement(stmt);
                }

                // *Then* take from the stack
                self.program.push(Instruction::new(
                    token.location,
                    InstructionType::DefineVariable(
                        token.lexeme.clone(),
                        stmt.is_some(),
                        *variable_mutability,
                    ),
                ));
            }
            Stmt::Expression(expr, _) => self.expression(expr),
            Stmt::Print(stmt) => {
                // Push variable to stack
                self.statement(stmt);

                // Then print
                self.program.push(Instruction::new(
                    stmt.get_location(),
                    InstructionType::Print,
                ));
            }
            Stmt::Return(location, stmt) => todo!(),
            Stmt::Assign(name, op, stmt) => {
                // Push value to stack
                self.statement(stmt);

                // Update variable
                self.program.push(Instruction::new(
                    stmt.get_location(),
                    InstructionType::ModifyVariable(name.to_string(), *op),
                ));
            }
            Stmt::Block(stmts) => {
                self.program.push(Instruction::new(
                    stmts.first().unwrap().get_location(),
                    InstructionType::PushScope,
                ));

                self.statements(stmts);

                self.program.push(Instruction::new(
                    stmts.last().unwrap().get_location(),
                    InstructionType::PopScope,
                ));
            }
            Stmt::If(stmt, stmt1, stmt2) => {
                // Evaluate condition
                self.statement(stmt);

                // Check if we need to go to the end
                let else_jump = self.program.len();
                self.program.push(Instruction::new(
                    stmt.get_location(),
                    // Initialize to 0 as we don't know how long this is going to be
                    InstructionType::JumpOnFalse(0),
                ));

                // If we don't jump, evaluate the if
                self.statement(stmt1);

                // Check if we need to go to the end
                if let Some(stmt2) = stmt2 {
                    // If we've got an else, jump over it
                    let end_jump_instruction = self.program.len();
                    self.program.push(Instruction::new(
                        stmt.get_location(),
                        // Initialize to 0 as we don't know how long this is going to be
                        InstructionType::Jump(0),
                    ));

                    let else_begin = self.program.len();
                    self.program[else_jump].value = InstructionType::JumpOnFalse(else_begin);

                    // Else
                    self.statement(stmt2);

                    // Update original jump over to refer to the end of this statement
                    let else_end_instruction = self.program.len();
                    self.program[end_jump_instruction].value =
                        InstructionType::Jump(else_end_instruction);
                } else {
                    let if_end = self.program.len();
                    self.program[else_jump].value = InstructionType::JumpOnFalse(if_end);
                }
            }
            Stmt::While(stmt, stmt1) => {
                // Mark where this begins
                let loop_start = self.program.len();

                // Push condition value
                self.statement(stmt);

                // Check if we need to go to the end
                let jump_instruction = self.program.len();
                self.program.push(Instruction::new(
                    stmt.get_location(),
                    // Initialize to 0 as we don't know how long this is going to be
                    InstructionType::JumpOnFalse(0),
                ));

                // Body
                self.statement(stmt1);

                // Go back to the beginning of the loop
                self.program.push(Instruction::new(
                    stmt.get_location(),
                    InstructionType::Jump(loop_start),
                ));

                // Modify previous instruction with final address
                let next_instruction = self.program.len();
                self.program[jump_instruction].value =
                    InstructionType::JumpOnFalse(next_instruction);
            }
        }
    }

    fn expression(&mut self, expr: &Expr) {
        match expr {
            Expr::Float(location, value) => self.program.push(Instruction::new(
                *location,
                InstructionType::Constant(Dynamic::Float(*value)),
            )),
            Expr::Integer(location, value) => self.program.push(Instruction::new(
                *location,
                InstructionType::Constant(Dynamic::Integer(*value)),
            )),
            Expr::Bool(location, value) => self.program.push(Instruction::new(
                *location,
                InstructionType::Constant(Dynamic::Bool(*value)),
            )),
            Expr::String(location, value) => self.program.push(Instruction::new(
                *location,
                InstructionType::Constant(Dynamic::String(value.clone())),
            )),
            Expr::Nil(location) => self.program.push(Instruction::new(
                *location,
                InstructionType::Constant(Dynamic::Nil),
            )),
            Expr::Unary(op, expr) => {
                // Push expression to stack
                self.expression(expr);

                self.program.push(Instruction::new(
                    expr.get_location(),
                    InstructionType::Unary(*op),
                ));
            }
            Expr::Binary(expr, op, expr1) => {
                // Push A to stack
                self.expression(expr);
                // Push B to stack
                self.expression(expr1);

                // Operate
                self.program.push(Instruction::new(
                    expr.get_location(),
                    InstructionType::Binary(*op),
                ));
            }
            Expr::Variable(location, identifier) => {
                self.program.push(Instruction::new(
                    *location,
                    InstructionType::Variable(identifier.to_string()),
                ));
            }
            Expr::Condition(expr, token, expr1) => {
                // Push A to stack (it's always evaluated)
                self.expression(expr);

                todo!()
                //self::condition(program, expr, token, expr1);
            }
            Expr::Call(callee, params) => {
                // Push callee to stack
                self.expression(callee);

                // Each parameter is loaded in order
                // The call function is special and "snacks" on the stack instead of popping each element
                for param in params {
                    self.expression(param);
                }

                // Tell it we want to call the callee with X params
                // TODO: Could this be ignored if this is all type checked?
                self.program.push(Instruction::new(
                    callee.get_location(),
                    InstructionType::Call(params.len()),
                ));
            }
        }
    }

    pub fn run(&self) -> Result<Option<Dynamic>, ExecutionError> {
        VmContext::new(self.module.clone(), None).run(&self.program)
    }
}

struct VmContext {
    scoper: Scoper,
    stack: Vec<Dynamic>,
    counter: usize,
    location: Location,
}

impl Executor for VmContext {}

impl VmContext {
    fn new(module: Arc<Module>, scope: Option<Scope>) -> VmContext {
        VmContext {
            scoper: Scoper::new(module, scope),
            stack: vec![],
            counter: 0,
            location: Location::default(),
        }
    }

    fn run(mut self, program: &[Instruction]) -> Result<Option<Dynamic>, ExecutionError> {
        while let Some(instruction) = program.get(self.counter) {
            //println!("Instruction: {instruction:?}");
            self.location = instruction.location;
            match &instruction.value {
                InstructionType::Print => println!("Print: {:?}", self.stack.pop().unwrap()),
                InstructionType::Constant(dynamic) => self.stack.push(dynamic.clone()),
                InstructionType::DefineVariable(name, initialized, mutability) => {
                    self.scoper.define_variable(
                        &name,
                        *mutability,
                        if *initialized {
                            self.stack.pop().unwrap()
                        } else {
                            Dynamic::Nil
                        },
                        instruction.location,
                    )?;
                }
                InstructionType::Unary(op) => self.unary_op(op),
                InstructionType::Binary(op) => {
                    self.binary_op(op);
                }
                InstructionType::Variable(name) => self
                    .stack
                    .push(self.scoper.get_variable(name, self.location)?),
                InstructionType::ModifyVariable(name, op) => {
                    self.assign(name, *op)?;
                }
                InstructionType::JumpOnFalse(address) => {
                    let Dynamic::Bool(cond) = self.stack.pop().unwrap() else {
                        unimplemented!();
                    };

                    if !cond {
                        self.counter = *address;
                        // Continue as we've modified the address ourselves
                        continue;
                    }
                }
                InstructionType::Jump(address) => {
                    self.counter = *address;
                    // Continue as we've modified the address ourselves
                    continue;
                }
                InstructionType::PushScope => self.scoper.push(None),
                InstructionType::PopScope => self.scoper.pop(),
                InstructionType::Call(params) => {
                    // Snack on the stack
                    let params = self.stack.split_off(self.stack.len() - params);
                    let callee = self.stack.pop().expect("Should have callee after params");

                    match callee {
                        Dynamic::NativeFunc(native_func_info) => {
                            if params.len() != native_func_info.signature.len() {
                                return Err(ExecutionError::new(
                                    self.location,
                                    ExecutionErrorType::MismatchedSignature(
                                        native_func_info.param_info.clone(),
                                        params,
                                    ),
                                ));
                            }

                            for i in 0..params.len() {
                                if params[i].get_type() != native_func_info.signature[i] {
                                    return Err(ExecutionError::new(
                                        self.location,
                                        ExecutionErrorType::MismatchedSignature(
                                            native_func_info.param_info.clone(),
                                            params,
                                        ),
                                    ));
                                }
                            }

                            // Execute the function and conditionally push to the stack
                            // TODO: Is this correct? Or should Nil return?
                            if let Some(val) = (native_func_info.call)(&mut self, params)? {
                                self.stack.push(val);
                            }
                        }
                        a => {
                            return Err(ExecutionError::new(
                                self.location,
                                ExecutionErrorType::NotAFunction(a),
                            ));
                        }
                    }
                }
            }

            self.counter += 1;
        }

        Ok(self.stack.pop())
    }

    fn assign(&mut self, name: &str, op: AssignmentOp) -> Result<(), ExecutionError> {
        let value = self.stack.pop().unwrap();

        self.scoper
            .mut_variable(name, self.location, move |variable, location| {
                if op == AssignmentOp::Equal {
                    variable.value = value;
                    return Ok(());
                }

                // Make sure it's the same variant, we don't care about value
                if std::mem::discriminant(&variable.value) != std::mem::discriminant(&value) {
                    return Err(ExecutionError::new(
                        location,
                        ExecutionErrorType::AssignmentOpInvalid(variable.value.clone(), op, value),
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

                Ok(())
            })
    }

    fn unary_op(&mut self, op: &UnaryOp) {
        match (op, self.stack.last_mut().unwrap()) {
            (UnaryOp::Negate, Dynamic::Integer(value)) => *value = -*value,
            (UnaryOp::Negate, Dynamic::Float(value)) => *value = -*value,
            (UnaryOp::Invert, Dynamic::Bool(value)) => *value = !*value,
            a => unimplemented!("Compile error: {a:?}"),
        }
    }

    fn binary_op(&mut self, op: &BinaryOp) {
        let var_b = self.stack.pop().unwrap();
        let mut var_a = self.stack.last_mut().unwrap();

        match (op, &mut var_a, var_b) {
            (BinaryOp::Plus, Dynamic::Integer(a), Dynamic::Integer(b)) => *a += b,
            (BinaryOp::Plus, Dynamic::Float(a), Dynamic::Float(b)) => *a += b,
            (BinaryOp::Plus, Dynamic::String(a), b) => match b {
                Dynamic::Bool(val) => *a += &val.to_string(),
                Dynamic::Integer(val) => *a += &val.to_string(),
                Dynamic::Float(val) => *a += &val.to_string(),
                Dynamic::String(val) => *a += &val,
                Dynamic::Nil => *a += "Nil",
                Dynamic::NativeFunc(func_info) => unimplemented!("Compile error: {func_info:?}"),
            },
            (BinaryOp::Minus, Dynamic::Integer(a), Dynamic::Integer(b)) => *a -= b,
            (BinaryOp::Minus, Dynamic::Float(a), Dynamic::Float(b)) => *a -= b,
            (BinaryOp::Multiply, Dynamic::Integer(a), Dynamic::Integer(b)) => *a *= b,
            (BinaryOp::Multiply, Dynamic::Float(a), Dynamic::Float(b)) => *a *= b,
            (BinaryOp::Divide, Dynamic::Integer(a), Dynamic::Integer(b)) => *a /= b,
            (BinaryOp::Divide, Dynamic::Float(a), Dynamic::Float(b)) => *a /= b,
            (BinaryOp::Greater, Dynamic::Integer(a), Dynamic::Integer(b)) => {
                *var_a = Dynamic::Bool(*a > b)
            }
            (BinaryOp::Greater, Dynamic::Float(a), Dynamic::Float(b)) => {
                *var_a = Dynamic::Bool(*a > b)
            }
            (BinaryOp::GreaterEqual, Dynamic::Integer(a), Dynamic::Integer(b)) => {
                *var_a = Dynamic::Bool(*a >= b)
            }
            (BinaryOp::GreaterEqual, Dynamic::Float(a), Dynamic::Float(b)) => {
                *var_a = Dynamic::Bool(*a >= b)
            }
            (BinaryOp::Less, Dynamic::Integer(a), Dynamic::Integer(b)) => {
                *var_a = Dynamic::Bool(*a < b)
            }
            (BinaryOp::Less, Dynamic::Float(a), Dynamic::Float(b)) => {
                *var_a = Dynamic::Bool(*a < b)
            }
            (BinaryOp::LessEqual, Dynamic::Integer(a), Dynamic::Integer(b)) => {
                *var_a = Dynamic::Bool(*a <= b)
            }
            (BinaryOp::LessEqual, Dynamic::Float(a), Dynamic::Float(b)) => {
                *var_a = Dynamic::Bool(*a <= b)
            }
            (BinaryOp::EqualEqual, Dynamic::Bool(a), Dynamic::Bool(b)) => *a = *a == b,
            (BinaryOp::EqualEqual, Dynamic::Integer(a), Dynamic::Integer(b)) => {
                *var_a = Dynamic::Bool(*a == b)
            }
            (BinaryOp::EqualEqual, Dynamic::Float(a), Dynamic::Float(b)) => {
                *var_a = Dynamic::Bool(*a == b)
            }
            (BinaryOp::EqualEqual, Dynamic::String(a), Dynamic::String(b)) => {
                *var_a = Dynamic::Bool(*a == b)
            }
            (BinaryOp::EqualEqual, Dynamic::NativeFunc(a), Dynamic::NativeFunc(b)) => {
                *var_a = Dynamic::Bool(Arc::ptr_eq(a, &b))
            }
            (BinaryOp::EqualEqual, Dynamic::Nil, b) => *var_a = Dynamic::Bool(Dynamic::Nil == b),
            (BinaryOp::EqualEqual, a, Dynamic::Nil) => **a = Dynamic::Bool(&Dynamic::Nil == *a),
            (BinaryOp::BangEqual, Dynamic::Bool(a), Dynamic::Bool(b)) => *a = *a != b,
            (BinaryOp::BangEqual, Dynamic::Integer(a), Dynamic::Integer(b)) => {
                *var_a = Dynamic::Bool(*a != b)
            }
            (BinaryOp::BangEqual, Dynamic::Float(a), Dynamic::Float(b)) => {
                *var_a = Dynamic::Bool(*a != b)
            }
            (BinaryOp::BangEqual, Dynamic::String(a), Dynamic::String(b)) => {
                *var_a = Dynamic::Bool(*a != b)
            }
            (BinaryOp::BangEqual, Dynamic::NativeFunc(a), Dynamic::NativeFunc(b)) => {
                *var_a = Dynamic::Bool(!Arc::ptr_eq(a, &b))
            }
            (BinaryOp::BangEqual, Dynamic::Nil, b) => *var_a = Dynamic::Bool(Dynamic::Nil != b),
            (BinaryOp::BangEqual, a, Dynamic::Nil) => **a = Dynamic::Bool(&Dynamic::Nil != *a),
            (op, a, b) => unimplemented!("Compile error: {op:?} {a:?} {b:?}"),
        }
    }
}
