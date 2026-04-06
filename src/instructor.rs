use std::{
    any::{Any, TypeId, type_name},
    fmt::Debug,
};

use hashbrown::HashMap;

use crate::{
    Location,
    constructor::{AExpr, AStmt, ConstantValue, ValueType, VariableSlot},
    parser::{BinaryOp, UnaryOp},
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
    Constant(usize),
    Unary(fn(&mut Box<dyn ProgramValue>)),
    Binary(fn(&mut Box<dyn ProgramValue>, Box<dyn ProgramValue>)),
    SetVariable(Box<VariableSlot>),
}

pub struct Instructor {
    program: Program,
    constant_cache: Vec<ConstantValue>,
    unary_ops: HashMap<(UnaryOp, ValueType), fn(&mut Box<dyn ProgramValue>)>,
    binary_ops: HashMap<
        (ValueType, BinaryOp, ValueType),
        fn(&mut Box<dyn ProgramValue>, Box<dyn ProgramValue>),
    >,
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
                self.push_instruction(
                    *location,
                    Instruction::SetVariable(Box::new(*variable_slot)),
                );
            }
            AStmt::While(location, astmt, astmt1) => todo!(),
            AStmt::Expression(location, aexpr) => {
                self.expression(aexpr);
            }
            AStmt::Block(location, astmts) => todo!(),
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
                println!("Program const: {:?}", program_value.as_ref().type_id());
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
            AExpr::RetrieveVariable(location, variable_slot, _, value_type) => todo!(),
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

    /*
    for numeric in [ValueType::Int, ValueType::Float] {
        // Math
        // Comparison
        binary_operators.insert((numeric, BinaryOp::Greater, numeric), ValueType::Bool);
        binary_operators.insert((numeric, BinaryOp::GreaterEqual, numeric), ValueType::Bool);
        binary_operators.insert((numeric, BinaryOp::Less, numeric), ValueType::Bool);
        binary_operators.insert((numeric, BinaryOp::LessEqual, numeric), ValueType::Bool);
        binary_operators.insert((numeric, BinaryOp::EqualEqual, numeric), ValueType::Bool);
        binary_operators.insert((numeric, BinaryOp::BangEqual, numeric), ValueType::Bool);

        // Strings
        binary_operators.insert(
            (ValueType::String, BinaryOp::Plus, numeric),
            ValueType::String,
        );

        // Nil handle
        binary_operators.insert(
            (ValueType::Nil, BinaryOp::EqualEqual, numeric),
            ValueType::Bool,
        );
        binary_operators.insert(
            (ValueType::Nil, BinaryOp::BangEqual, numeric),
            ValueType::Bool,
        );
        binary_operators.insert(
            (numeric, BinaryOp::EqualEqual, ValueType::Nil),
            ValueType::Bool,
        );
        binary_operators.insert(
            (numeric, BinaryOp::BangEqual, ValueType::Nil),
            ValueType::Bool,
        );
    }

    // Bool
    binary_operators.insert(
        (ValueType::Bool, BinaryOp::EqualEqual, ValueType::Bool),
        ValueType::Bool,
    );
    binary_operators.insert(
        (ValueType::Bool, BinaryOp::BangEqual, ValueType::Bool),
        ValueType::Bool,
    );

    // Nil handle
    binary_operators.insert(
        (ValueType::Nil, BinaryOp::EqualEqual, ValueType::Bool),
        ValueType::Bool,
    );
    binary_operators.insert(
        (ValueType::Nil, BinaryOp::BangEqual, ValueType::Bool),
        ValueType::Bool,
    );
    binary_operators.insert(
        (ValueType::Bool, BinaryOp::EqualEqual, ValueType::Nil),
        ValueType::Bool,
    );
    binary_operators.insert(
        (ValueType::Bool, BinaryOp::BangEqual, ValueType::Nil),
        ValueType::Bool,
    );*/

    binary_operators
}

fn binary_equal<A: ProgramValue, B: ProgramValue>(
    a_orig: &mut Box<dyn ProgramValue>,
    b_orig: Box<dyn ProgramValue>,
) where
    A: PartialEq<B>,
{
    let a = a_orig.as_any().downcast_ref::<A>().unwrap();
    let b = b_orig.as_any().downcast_ref::<B>().unwrap();

    *a_orig = Box::new(a == b);
}

fn binary_not_equal<A: ProgramValue, B: ProgramValue>(
    a_orig: &mut Box<dyn ProgramValue>,
    b_orig: Box<dyn ProgramValue>,
) where
    A: PartialEq<B>,
{
    let a = a_orig.as_any().downcast_ref::<A>().unwrap();
    let b = b_orig.as_any().downcast_ref::<B>().unwrap();

    *a_orig = Box::new(a != b);
}

fn binary_greater<T: ProgramValue>(
    a_orig: &mut Box<dyn ProgramValue>,
    b_orig: Box<dyn ProgramValue>,
) where
    T: PartialOrd,
{
    let a = a_orig.as_any().downcast_ref::<T>().unwrap();
    let b = b_orig.as_any().downcast_ref::<T>().unwrap();

    *a_orig = Box::new(a > b);
}

fn binary_greater_equal<T: ProgramValue>(
    a_orig: &mut Box<dyn ProgramValue>,
    b_orig: Box<dyn ProgramValue>,
) where
    T: PartialOrd,
{
    let a = a_orig.as_any().downcast_ref::<T>().unwrap();
    let b = b_orig.as_any().downcast_ref::<T>().unwrap();

    *a_orig = Box::new(a >= b);
}

fn binary_lesser<T: ProgramValue>(a_orig: &mut Box<dyn ProgramValue>, b_orig: Box<dyn ProgramValue>)
where
    T: PartialOrd,
{
    let a = a_orig.as_any().downcast_ref::<T>().unwrap();
    let b = b_orig.as_any().downcast_ref::<T>().unwrap();

    *a_orig = Box::new(a < b);
}

fn binary_lesser_equal<T: ProgramValue>(
    a_orig: &mut Box<dyn ProgramValue>,
    b_orig: Box<dyn ProgramValue>,
) where
    T: PartialOrd,
{
    let a = a_orig.as_any().downcast_ref::<T>().unwrap();
    let b = b_orig.as_any().downcast_ref::<T>().unwrap();

    *a_orig = Box::new(a <= b);
}

fn binary_plus<A: ProgramValue, B: ProgramValue>(
    a: &mut Box<dyn ProgramValue>,
    b: Box<dyn ProgramValue>,
) where
    A: std::ops::AddAssign<B>,
    B: Copy,
{
    let a = a.as_any_mut().downcast_mut::<A>().unwrap();
    let b = b.as_any().downcast_ref::<B>().unwrap();

    *a += *b;
}

fn binary_minus<A: ProgramValue, B: ProgramValue>(
    a: &mut Box<dyn ProgramValue>,
    b: Box<dyn ProgramValue>,
) where
    A: std::ops::SubAssign<B>,
    B: Copy,
{
    let a = a.as_any_mut().downcast_mut::<A>().unwrap();
    let b = b.as_any().downcast_ref::<B>().unwrap();

    *a -= *b;
}

fn binary_multiply<A: ProgramValue, B: ProgramValue>(
    a: &mut Box<dyn ProgramValue>,
    b: Box<dyn ProgramValue>,
) where
    A: std::ops::MulAssign<B>,
    B: Copy,
{
    let a = a.as_any_mut().downcast_mut::<A>().unwrap();
    let b = b.as_any().downcast_ref::<B>().unwrap();

    *a *= *b;
}

fn binary_divide<A: ProgramValue, B: ProgramValue>(
    a: &mut Box<dyn ProgramValue>,
    b: Box<dyn ProgramValue>,
) where
    A: std::ops::DivAssign<B>,
    B: Copy,
{
    let a = a.as_any_mut().downcast_mut::<A>().unwrap();
    let b = b.as_any().downcast_ref::<B>().unwrap();

    *a /= *b;
}
