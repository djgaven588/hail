use crate::{
    Dynamic, ExecutionError, Location, Scope, Scoper,
    parser::{Expr, Stmt, VariableMutability},
    scanner::{Token, TokenType},
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
    Constant(Dynamic),
    Variable(String),
    Unary(UnaryOp),
    Binary(BinaryOp),
    // Name, has initializer, mutability
    DefineVariable(String, bool, VariableMutability),
    ModifyVariable(String),
    Print,
    JumpOnFalse(usize),
    Jump(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UnaryOp {
    Negate,
    Invert,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BinaryOp {
    Plus,
    Minus,
    Multiply,
    Divide,
    Greater,
    GreaterEqual,
    Less,
    LessEqual,
    EqualEqual,
    BangEqual,
}

#[derive(Default)]
pub struct Vm {
    program: Vec<Instruction>,
}

impl Vm {
    pub fn new(stmts: &[Stmt]) -> Vm {
        let mut vm = Vm::default();
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
                    token.location.clone(),
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
            Stmt::Return(stmt) => todo!(),
            Stmt::Assign(token, stmt) => {
                // Push value to stack
                self.statement(stmt);

                // Update variable
                self.program.push(Instruction::new(
                    token.location.clone(),
                    InstructionType::ModifyVariable(token.lexeme.to_string()),
                ));
            }
            Stmt::Block(stmts) => self.statements(stmts),
            Stmt::If(stmt, stmt1, stmt2) => todo!(),
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
                location.clone(),
                InstructionType::Constant(Dynamic::Float(*value)),
            )),
            Expr::Integer(location, value) => self.program.push(Instruction::new(
                location.clone(),
                InstructionType::Constant(Dynamic::Integer(*value)),
            )),
            Expr::Bool(location, value) => self.program.push(Instruction::new(
                location.clone(),
                InstructionType::Constant(Dynamic::Bool(*value)),
            )),
            Expr::String(location, value) => self.program.push(Instruction::new(
                location.clone(),
                InstructionType::Constant(Dynamic::String(value.clone())),
            )),
            Expr::Nil(location) => self.program.push(Instruction::new(
                location.clone(),
                InstructionType::Constant(Dynamic::Nil),
            )),
            Expr::Unary(token, expr) => {
                // Push expression to stack
                self.expression(expr);

                self.unary(token);
            }
            Expr::Binary(expr, token, expr1) => {
                // Push A to stack
                self.expression(expr);
                // Push B to stack
                self.expression(expr1);

                // Operate
                self.binary(token);
            }
            Expr::Variable(token) => {
                self.program.push(Instruction::new(
                    token.location.clone(),
                    InstructionType::Variable(token.lexeme.to_string()),
                ));
            }
            Expr::Condition(expr, token, expr1) => {
                // Push A to stack (it's always evaluated)
                self.expression(expr);

                todo!()
                //self::condition(program, expr, token, expr1);
            }
        }
    }

    fn unary(&mut self, op: &Token) {
        self.program.push(Instruction::new(
            op.location.clone(),
            InstructionType::Unary(match op.token_type {
                TokenType::Minus => UnaryOp::Negate,
                TokenType::Bang => UnaryOp::Invert,
                a => unimplemented!("{a:?}"),
            }),
        ));
    }

    fn binary(&mut self, op: &Token) {
        self.program.push(Instruction::new(
            op.location.clone(),
            InstructionType::Binary(match op.token_type {
                TokenType::Minus => BinaryOp::Minus,
                TokenType::Plus => BinaryOp::Plus,
                TokenType::Slash => BinaryOp::Divide,
                TokenType::Star => BinaryOp::Multiply,
                TokenType::BangEqual => BinaryOp::BangEqual,
                TokenType::EqualEqual => BinaryOp::EqualEqual,
                TokenType::Greater => BinaryOp::Greater,
                TokenType::GreaterEqual => BinaryOp::GreaterEqual,
                TokenType::Less => BinaryOp::Less,
                TokenType::LessEqual => BinaryOp::LessEqual,
                a => unimplemented!("{a:?}"),
            }),
        ));
    }

    pub fn run(&self) -> Result<Option<Dynamic>, ExecutionError> {
        VmContext::new(None).run(&self.program)
    }
}

struct VmContext {
    scoper: Scoper,
    stack: Vec<Dynamic>,
    counter: usize,
    location: Location,
}

impl VmContext {
    fn new(scope: Option<Scope>) -> VmContext {
        VmContext {
            scoper: Scoper::new(scope),
            stack: vec![],
            counter: 0,
            location: Location::default(),
        }
    }

    fn run(mut self, program: &[Instruction]) -> Result<Option<Dynamic>, ExecutionError> {
        while let Some(instruction) = program.get(self.counter) {
            //println!("Instruction: {instruction:?}");
            self.location = instruction.location.clone();
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
                        &instruction.location,
                    )?;
                }
                InstructionType::Unary(op) => self.unary_op(op),
                InstructionType::Binary(op) => {
                    self.binary_op(op);
                }
                InstructionType::Variable(name) => self
                    .stack
                    .push(self.scoper.get_variable(name, &self.location)?),
                InstructionType::ModifyVariable(name) => {
                    self.scoper
                        .assign_variable(name, self.stack.pop().unwrap(), &self.location)?;
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
            }

            self.counter += 1;
        }

        Ok(self.stack.pop())
    }

    fn unary_op(&mut self, op: &UnaryOp) {
        match (op, self.stack.last_mut().unwrap()) {
            (UnaryOp::Negate, Dynamic::Integer(value)) => *value = -*value,
            (UnaryOp::Negate, Dynamic::Float(value)) => *value = -*value,
            (UnaryOp::Invert, Dynamic::Bool(value)) => *value = !*value,
            a => unimplemented!("{a:?}"),
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
            (BinaryOp::EqualEqual, Dynamic::Nil, b) => *var_a = Dynamic::Bool(Dynamic::Nil == b),
            (BinaryOp::EqualEqual, a, Dynamic::Nil) => **a = Dynamic::Bool(&Dynamic::Nil == *a),
            (BinaryOp::BangEqual, Dynamic::Bool(a), Dynamic::Bool(b)) => *a = *a == b,
            (BinaryOp::BangEqual, Dynamic::Integer(a), Dynamic::Integer(b)) => {
                *var_a = Dynamic::Bool(*a == b)
            }
            (BinaryOp::BangEqual, Dynamic::Float(a), Dynamic::Float(b)) => {
                *var_a = Dynamic::Bool(*a == b)
            }
            (BinaryOp::BangEqual, Dynamic::String(a), Dynamic::String(b)) => {
                *var_a = Dynamic::Bool(*a == b)
            }
            (BinaryOp::BangEqual, Dynamic::Nil, b) => *var_a = Dynamic::Bool(Dynamic::Nil == b),
            (BinaryOp::BangEqual, a, Dynamic::Nil) => **a = Dynamic::Bool(&Dynamic::Nil == *a),
            (op, a, b) => unimplemented!("{op:?} {a:?} {b:?}"),
        }
    }
}
