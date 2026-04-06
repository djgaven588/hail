use std::{
    any::{Any, type_name},
    fmt::Debug,
};

use hashbrown::HashMap;

use crate::{
    Location,
    constructor::{AExpr, AStmt, ConstantValue, ValueType, VariableSlot},
    parser::{AssignmentOp, BinaryOp, UnaryOp},
};

pub trait ProgramValue: 'static {
    fn clone_box(&self) -> Box<dyn ProgramValue>;
    fn into_any(self: Box<Self>) -> Box<dyn Any>;
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn display_name(&self) -> String;
}

impl Debug for dyn ProgramValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.display_name())
    }
}

impl<T: Clone + 'static> ProgramValue for T {
    /// Clone a program value
    /// NOTE: As this is usually wrapped in a box, it needs to have an .as_ref() to get inside right for typing
    fn clone_box(&self) -> Box<dyn ProgramValue> {
        Box::new(self.clone())
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn display_name(&self) -> String {
        type_name::<T>().to_owned()
    }
}

#[derive(Debug)]
pub struct Program {
    pub constants: Vec<Box<dyn ProgramValue>>,
    pub instructions: Vec<Instruction>,
    pub locations: Vec<Location>,
}

#[derive(Debug)]
pub enum Instruction {
    // Const index
    Constant(usize),
    // Unary op on value
    Unary(fn(&mut Box<dyn ProgramValue>)),
    // Binary op on values
    Binary(fn(&mut Box<dyn ProgramValue>, Box<dyn ProgramValue>)),
    // Pop a variable into a slot
    SetVariable(VariableSlot),
    // Modify an existing variable
    AssignVariable(
        VariableSlot,
        fn(&mut Box<dyn ProgramValue>, Box<dyn ProgramValue>),
    ),

    // Push a variable onto the stack
    GetVariable(VariableSlot),
    // Jump address
    JumpIfFalse(usize),
    Jump(usize),
}

pub struct Instructor {
    program: Program,
    constant_cache: Vec<ConstantValue>,
    unary_ops: HashMap<(UnaryOp, ValueType), fn(&mut Box<dyn ProgramValue>)>,
    binary_ops: HashMap<
        (ValueType, BinaryOp, ValueType),
        fn(&mut Box<dyn ProgramValue>, Box<dyn ProgramValue>),
    >,
    assign_ops:
        HashMap<(ValueType, AssignmentOp), fn(&mut Box<dyn ProgramValue>, Box<dyn ProgramValue>)>,
}

impl Instructor {
    pub fn new() -> Instructor {
        Instructor {
            program: Program {
                instructions: vec![],
                locations: vec![],
                constants: vec![],
            },
            constant_cache: vec![],
            unary_ops: get_unary_ops(),
            binary_ops: get_binary_ops(),
            assign_ops: get_assign_ops(),
        }
    }

    pub fn generate(mut self, stmts: &[AStmt]) -> Program {
        for stmt in stmts {
            self.statement(stmt);
        }

        self.program
    }

    fn push_instruction(&mut self, location: Location, instruction: Instruction) {
        self.program.locations.push(location);
        self.program.instructions.push(instruction);
    }

    fn statement(&mut self, stmt: &AStmt) {
        match stmt {
            AStmt::Print(location, astmt) => todo!(),
            AStmt::SetVariable(location, variable_slot, _, astmt) => {
                // Push the variable to the stack
                self.statement(astmt);

                // Consume the variable into a slot
                self.push_instruction(*location, Instruction::SetVariable(*variable_slot));
            }
            AStmt::AssignVariable(location, variable_slot, _, op, astmt) => {
                // Push the variable to the stack
                self.statement(astmt);

                if op == &AssignmentOp::Equal {
                    // Regular assignment
                    self.push_instruction(*location, Instruction::SetVariable(*variable_slot));
                    return;
                }

                let assign_op = self
                    .assign_ops
                    .get(&(astmt.get_value_type().expect("Should have value type"), *op))
                    .expect("Assign operator should be available?");

                // Consume the variable into a slot
                self.push_instruction(
                    *location,
                    Instruction::AssignVariable(*variable_slot, *assign_op),
                );
            }
            AStmt::While(location, condition, body) => {
                let start_instruction_count = self.program.instructions.len();
                self.statement(condition);

                let jump_if_count = self.program.instructions.len();
                // Jump to after if we fail, we'll setup the address shortly
                self.push_instruction(*location, Instruction::JumpIfFalse(0));

                // The body
                self.statement(body);

                // Go back to condition
                self.push_instruction(*location, Instruction::Jump(start_instruction_count));

                // We're back for that jump instruction
                // Go to the end
                self.program.instructions[jump_if_count] =
                    Instruction::JumpIfFalse(self.program.instructions.len());
            }
            AStmt::Expression(_, aexpr) => {
                self.expression(aexpr);
            }
            AStmt::Block(_, astmts) => {
                // Before, implementations would handle scoping for blocks explicitly
                // Here though, with slots, it *shouldn't* need to as only frames matter
                // So... No realloc of the variables..?

                // Execute
                for stmt in astmts {
                    self.statement(stmt);
                }

                // What would be an exit scope
            }
        }
    }

    fn expression(&mut self, expr: &AExpr) {
        match expr {
            AExpr::Constant(location, constant_value) => {
                // Try and use an existing constant, checked via the cache.
                // This cache is needed since the program is "hot", so we
                // aren't able to tell type info anymore, it's full send.
                for (index, value) in self.constant_cache.iter().enumerate() {
                    if value == constant_value {
                        self.push_instruction(*location, Instruction::Constant(index));
                        return;
                    }
                }

                // Insert a new constant
                let const_index = self.program.constants.len();

                // This pushes to both the cache and the program so we can compare above
                self.constant_cache.push(constant_value.clone());

                let program_value = constant_value.to_program_value();
                self.program.constants.push(program_value);

                self.push_instruction(*location, Instruction::Constant(const_index));
            }
            AExpr::Unary(location, unary_op, aexpr, value_type) => {
                self.expression(aexpr);

                let unary = self
                    .unary_ops
                    .get(&(*unary_op, aexpr.get_value_type()))
                    .expect("Operator should be available?");

                self.push_instruction(*location, Instruction::Unary(*unary));
            }
            AExpr::Binary(location, expr_a, binary_op, expr_b, value_type) => {
                self.expression(expr_a);
                self.expression(expr_b);

                let binary = self
                    .binary_ops
                    .get(&(expr_a.get_value_type(), *binary_op, expr_b.get_value_type()))
                    .expect("Operator should be available?");

                self.push_instruction(*location, Instruction::Binary(*binary));
            }
            AExpr::RetrieveVariable(location, variable_slot, _, _) => {
                self.push_instruction(*location, Instruction::GetVariable(*variable_slot));
            }
        }
    }
}

fn get_unary_ops() -> HashMap<(UnaryOp, ValueType), fn(a: &mut Box<dyn ProgramValue>)> {
    let mut unary_operators: HashMap<(UnaryOp, ValueType), fn(a: &mut Box<dyn ProgramValue>)> =
        Default::default();

    unary_operators.insert((UnaryOp::Negate, ValueType::Int), unary_negate::<i64>);
    unary_operators.insert((UnaryOp::Negate, ValueType::Float), unary_negate::<f64>);

    unary_operators.insert((UnaryOp::Invert, ValueType::Bool), unary_invert::<bool>);

    unary_operators
}

fn unary_negate<T: ProgramValue>(a: &mut Box<dyn ProgramValue>)
where
    T: Copy + std::ops::Neg<Output = T>,
{
    let val = a.as_any_mut().downcast_mut::<T>().unwrap();
    *val = -*val;
}

fn unary_invert<T: ProgramValue>(a: &mut Box<dyn ProgramValue>)
where
    T: Copy + std::ops::Not<Output = T>,
{
    let val = a.as_any_mut().downcast_mut::<T>().unwrap();
    *val = !*val;
}

fn get_binary_ops() -> HashMap<
    (ValueType, BinaryOp, ValueType),
    fn(a: &mut Box<dyn ProgramValue>, b: Box<dyn ProgramValue>),
> {
    let mut binary_operators: HashMap<
        (ValueType, BinaryOp, ValueType),
        fn(&mut Box<dyn ProgramValue>, Box<dyn ProgramValue>),
    > = Default::default();

    // Math Int
    binary_operators.insert(
        (ValueType::Int, BinaryOp::Plus, ValueType::Int),
        binary_plus::<i64, i64>,
    );
    binary_operators.insert(
        (ValueType::Int, BinaryOp::Minus, ValueType::Int),
        binary_minus::<i64, i64>,
    );
    binary_operators.insert(
        (ValueType::Int, BinaryOp::Multiply, ValueType::Int),
        binary_multiply::<i64, i64>,
    );
    binary_operators.insert(
        (ValueType::Int, BinaryOp::Divide, ValueType::Int),
        binary_divide::<i64, i64>,
    );

    // Math Float
    binary_operators.insert(
        (ValueType::Float, BinaryOp::Plus, ValueType::Float),
        binary_plus::<f64, f64>,
    );
    binary_operators.insert(
        (ValueType::Float, BinaryOp::Minus, ValueType::Float),
        binary_minus::<f64, f64>,
    );
    binary_operators.insert(
        (ValueType::Float, BinaryOp::Multiply, ValueType::Float),
        binary_multiply::<f64, f64>,
    );
    binary_operators.insert(
        (ValueType::Float, BinaryOp::Divide, ValueType::Float),
        binary_divide::<f64, f64>,
    );

    // Compare Int
    binary_operators.insert(
        (ValueType::Int, BinaryOp::Greater, ValueType::Int),
        binary_greater::<i64>,
    );
    binary_operators.insert(
        (ValueType::Int, BinaryOp::GreaterEqual, ValueType::Int),
        binary_greater_equal::<i64>,
    );
    binary_operators.insert(
        (ValueType::Int, BinaryOp::Less, ValueType::Int),
        binary_lesser::<i64>,
    );
    binary_operators.insert(
        (ValueType::Int, BinaryOp::LessEqual, ValueType::Int),
        binary_lesser_equal::<i64>,
    );
    binary_operators.insert(
        (ValueType::Int, BinaryOp::EqualEqual, ValueType::Int),
        binary_equal::<i64, i64>,
    );
    binary_operators.insert(
        (ValueType::Int, BinaryOp::BangEqual, ValueType::Int),
        binary_not_equal::<i64, i64>,
    );

    // Compare Float
    binary_operators.insert(
        (ValueType::Float, BinaryOp::Greater, ValueType::Float),
        binary_greater::<f64>,
    );
    binary_operators.insert(
        (ValueType::Float, BinaryOp::GreaterEqual, ValueType::Float),
        binary_greater_equal::<f64>,
    );
    binary_operators.insert(
        (ValueType::Float, BinaryOp::Less, ValueType::Float),
        binary_lesser::<f64>,
    );
    binary_operators.insert(
        (ValueType::Float, BinaryOp::LessEqual, ValueType::Float),
        binary_lesser_equal::<f64>,
    );
    binary_operators.insert(
        (ValueType::Float, BinaryOp::EqualEqual, ValueType::Float),
        binary_equal::<f64, f64>,
    );
    binary_operators.insert(
        (ValueType::Float, BinaryOp::BangEqual, ValueType::Float),
        binary_not_equal::<f64, f64>,
    );

    // Compare bool
    binary_operators.insert(
        (ValueType::Bool, BinaryOp::EqualEqual, ValueType::Bool),
        binary_equal::<bool, bool>,
    );
    binary_operators.insert(
        (ValueType::Bool, BinaryOp::BangEqual, ValueType::Bool),
        binary_not_equal::<bool, bool>,
    );

    binary_operators
}

fn binary_equal<A: ProgramValue, B: ProgramValue>(
    a_orig: &mut Box<dyn ProgramValue>,
    b_orig: Box<dyn ProgramValue>,
) where
    A: PartialEq<B>,
{
    let a = unsafe { a_orig.as_any().downcast_unchecked_ref::<A>() };
    let b = unsafe { b_orig.as_any().downcast_unchecked_ref::<B>() };

    *a_orig = Box::new(a == b);
}

fn binary_not_equal<A: ProgramValue, B: ProgramValue>(
    a_orig: &mut Box<dyn ProgramValue>,
    b_orig: Box<dyn ProgramValue>,
) where
    A: PartialEq<B>,
{
    let a = unsafe { a_orig.as_any().downcast_unchecked_ref::<A>() };
    let b = unsafe { b_orig.as_any().downcast_unchecked_ref::<B>() };

    *a_orig = Box::new(a != b);
}

fn binary_greater<T: ProgramValue>(
    a_orig: &mut Box<dyn ProgramValue>,
    b_orig: Box<dyn ProgramValue>,
) where
    T: PartialOrd,
{
    let a = unsafe { a_orig.as_any().downcast_unchecked_ref::<T>() };
    let b = unsafe { b_orig.as_any().downcast_unchecked_ref::<T>() };

    *a_orig = Box::new(a > b);
}

fn binary_greater_equal<T: ProgramValue>(
    a_orig: &mut Box<dyn ProgramValue>,
    b_orig: Box<dyn ProgramValue>,
) where
    T: PartialOrd,
{
    let a = unsafe { a_orig.as_any().downcast_unchecked_ref::<T>() };
    let b = unsafe { b_orig.as_any().downcast_unchecked_ref::<T>() };

    *a_orig = Box::new(a >= b);
}

fn binary_lesser<T: ProgramValue>(a_orig: &mut Box<dyn ProgramValue>, b_orig: Box<dyn ProgramValue>)
where
    T: PartialOrd,
{
    let a = unsafe { a_orig.as_any().downcast_unchecked_ref::<T>() };
    let b = unsafe { b_orig.as_any().downcast_unchecked_ref::<T>() };

    *a_orig = Box::new(a < b);
}

fn binary_lesser_equal<T: ProgramValue>(
    a_orig: &mut Box<dyn ProgramValue>,
    b_orig: Box<dyn ProgramValue>,
) where
    T: PartialOrd,
{
    let a = unsafe { a_orig.as_any().downcast_unchecked_ref::<T>() };
    let b = unsafe { b_orig.as_any().downcast_unchecked_ref::<T>() };

    *a_orig = Box::new(a <= b);
}

fn binary_plus<A: ProgramValue, B: ProgramValue>(
    a: &mut Box<dyn ProgramValue>,
    b: Box<dyn ProgramValue>,
) where
    A: std::ops::AddAssign<B>,
    B: Copy,
{
    let a = unsafe { a.as_any_mut().downcast_unchecked_mut::<A>() };
    let b = unsafe { b.as_any().downcast_unchecked_ref::<B>() };

    *a += *b;
}

fn binary_minus<A: ProgramValue, B: ProgramValue>(
    a: &mut Box<dyn ProgramValue>,
    b: Box<dyn ProgramValue>,
) where
    A: std::ops::SubAssign<B>,
    B: Copy,
{
    let a = unsafe { a.as_any_mut().downcast_unchecked_mut::<A>() };
    let b = unsafe { b.as_any().downcast_unchecked_ref::<B>() };

    *a -= *b;
}

fn binary_multiply<A: ProgramValue, B: ProgramValue>(
    a: &mut Box<dyn ProgramValue>,
    b: Box<dyn ProgramValue>,
) where
    A: std::ops::MulAssign<B>,
    B: Copy,
{
    let a = unsafe { a.as_any_mut().downcast_unchecked_mut::<A>() };
    let b = unsafe { b.as_any().downcast_unchecked_ref::<B>() };

    *a *= *b;
}

fn binary_divide<A: ProgramValue, B: ProgramValue>(
    a: &mut Box<dyn ProgramValue>,
    b: Box<dyn ProgramValue>,
) where
    A: std::ops::DivAssign<B>,
    B: Copy,
{
    let a = unsafe { a.as_any_mut().downcast_unchecked_mut::<A>() };
    let b = unsafe { b.as_any().downcast_unchecked_ref::<B>() };

    *a /= *b;
}

fn get_assign_ops()
-> HashMap<(ValueType, AssignmentOp), fn(&mut Box<dyn ProgramValue>, Box<dyn ProgramValue>)> {
    let mut values: HashMap<
        (ValueType, AssignmentOp),
        fn(&mut Box<dyn ProgramValue>, Box<dyn ProgramValue>),
    > = Default::default();

    // Int
    values.insert(
        (ValueType::Int, AssignmentOp::PlusEqual),
        binary_plus::<i64, i64>,
    );
    values.insert(
        (ValueType::Int, AssignmentOp::MinusEqual),
        binary_minus::<i64, i64>,
    );
    values.insert(
        (ValueType::Int, AssignmentOp::MultiplyEqual),
        binary_multiply::<i64, i64>,
    );
    values.insert(
        (ValueType::Int, AssignmentOp::DivideEqual),
        binary_divide::<i64, i64>,
    );

    // Float
    values.insert(
        (ValueType::Int, AssignmentOp::PlusEqual),
        binary_plus::<i64, i64>,
    );
    values.insert(
        (ValueType::Int, AssignmentOp::MinusEqual),
        binary_minus::<i64, i64>,
    );
    values.insert(
        (ValueType::Int, AssignmentOp::MultiplyEqual),
        binary_multiply::<i64, i64>,
    );
    values.insert(
        (ValueType::Int, AssignmentOp::DivideEqual),
        binary_divide::<i64, i64>,
    );

    // String
    /*
    values.insert(
        (ValueType::String, AssignmentOp::PlusEqual),
        binary_plus::<String, String>,
    );*/

    values
}
