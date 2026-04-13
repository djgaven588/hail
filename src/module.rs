use std::sync::Arc;

use hashbrown::HashMap;

use crate::{
    constructor::ValueType,
    instructor::ProgramValue,
    parser::{AssignmentOp, BinaryOp, UnaryOp},
};

pub type UnaryFn = fn(&mut Box<dyn ProgramValue>);
pub type BinaryFn = fn(&mut Box<dyn ProgramValue>, Box<dyn ProgramValue>);

#[derive(PartialEq, Eq, Clone, Copy, Hash, Debug)]
pub struct UnarySignature {
    op: UnaryOp,
    value: ValueType,
}

impl UnarySignature {
    pub fn new(op: UnaryOp, value: ValueType) -> UnarySignature {
        Self { op, value }
    }
}

#[derive(PartialEq, Eq, Clone, Copy, Hash, Debug)]
pub struct BinarySignature {
    a: ValueType,
    op: BinaryOp,
    b: ValueType,
}

impl BinarySignature {
    pub fn new(a: ValueType, op: BinaryOp, b: ValueType) -> BinarySignature {
        Self { a, op, b }
    }
}

#[derive(Default)]
pub struct Module {
    typing: HashMap<String, ValueType>,
    globals: HashMap<String, Box<dyn ProgramValue>>,

    pub(crate) unary_ops: HashMap<UnarySignature, (UnaryFn, ValueType)>,
    pub(crate) binary_ops: HashMap<BinarySignature, (BinaryFn, ValueType)>,

    pub(crate) stringify_ops: HashMap<ValueType, UnaryFn>,
    pub(crate) assignment_ops: HashMap<(ValueType, AssignmentOp), BinaryFn>,
}

impl Module {
    pub fn resolve_type(&self, name: &str) -> Option<ValueType> {
        match name {
            "i64" => Some(ValueType::Int),
            "f64" => Some(ValueType::Float),
            "bool" => Some(ValueType::Bool),
            "String" => Some(ValueType::String),
            name => self.typing.get(name).copied(),
        }
    }

    /*
    pub fn define_type<T: ProgramValue>(&mut self, friendly: &str) -> Result<(), String> {
        //self.typing.insert(friendly, ValueType::)
    } */

    pub fn global(&self, name: &str) -> Option<&Box<dyn ProgramValue>> {
        self.globals.get(name)
    }

    /// Define constants, functions, anything implementing ``Scripting`` that are made available to scripts
    pub fn define<T: ProgramValue>(&mut self, name: &str, value: T) -> Result<(), String> {
        let Some(existing) = self.globals.insert(name.to_string(), Box::new(value)) else {
            return Ok(());
        };

        Err(format!("'{name}' was occupied with '{existing:?}'"))
    }

    pub fn std() -> Arc<Module> {
        let mut module = Module::default();

        get_unary_ops(&mut module);
        get_binary_ops(&mut module);
        get_assign_ops(&mut module);
        get_stringify_ops(&mut module);

        Arc::new(module)
    }
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

fn binary_string_plus<A: ProgramValue, B: ProgramValue>(
    a: &mut Box<dyn ProgramValue>,
    b: Box<dyn ProgramValue>,
) where
    A: ToString + for<'a> std::ops::AddAssign<&'a str>,
    B: ToString,
{
    let a = unsafe { a.as_any_mut().downcast_unchecked_mut::<A>() };
    let b = unsafe { b.as_any().downcast_unchecked_ref::<B>() };

    *a += b.to_string().as_str();
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

fn stringify<T: ProgramValue>(val_orig: &mut Box<dyn ProgramValue>)
where
    T: ToString,
{
    let val = unsafe { val_orig.as_any_mut().downcast_unchecked_mut::<T>() };

    *val_orig = Box::new(val.to_string());
}

fn get_unary_ops(module: &mut Module) {
    module.unary_ops.insert(
        UnarySignature::new(UnaryOp::Negate, ValueType::Int),
        (unary_negate::<i64>, ValueType::Int),
    );
    module.unary_ops.insert(
        UnarySignature::new(UnaryOp::Negate, ValueType::Float),
        (unary_negate::<f64>, ValueType::Float),
    );

    module.unary_ops.insert(
        UnarySignature::new(UnaryOp::Invert, ValueType::Bool),
        (unary_invert::<bool>, ValueType::Bool),
    );
}

fn get_binary_ops(module: &mut Module) {
    // Math Int
    module.binary_ops.insert(
        BinarySignature::new(ValueType::Int, BinaryOp::Plus, ValueType::Int),
        (binary_plus::<i64, i64>, ValueType::Int),
    );
    module.binary_ops.insert(
        BinarySignature::new(ValueType::Int, BinaryOp::Minus, ValueType::Int),
        (binary_minus::<i64, i64>, ValueType::Int),
    );
    module.binary_ops.insert(
        BinarySignature::new(ValueType::Int, BinaryOp::Multiply, ValueType::Int),
        (binary_multiply::<i64, i64>, ValueType::Int),
    );
    module.binary_ops.insert(
        BinarySignature::new(ValueType::Int, BinaryOp::Divide, ValueType::Int),
        (binary_divide::<i64, i64>, ValueType::Int),
    );

    // Math Float
    module.binary_ops.insert(
        BinarySignature::new(ValueType::Float, BinaryOp::Plus, ValueType::Float),
        (binary_plus::<f64, f64>, ValueType::Float),
    );
    module.binary_ops.insert(
        BinarySignature::new(ValueType::Float, BinaryOp::Minus, ValueType::Float),
        (binary_minus::<f64, f64>, ValueType::Float),
    );
    module.binary_ops.insert(
        BinarySignature::new(ValueType::Float, BinaryOp::Multiply, ValueType::Float),
        (binary_multiply::<f64, f64>, ValueType::Float),
    );
    module.binary_ops.insert(
        BinarySignature::new(ValueType::Float, BinaryOp::Divide, ValueType::Float),
        (binary_divide::<f64, f64>, ValueType::Float),
    );

    // Compare Int
    module.binary_ops.insert(
        BinarySignature::new(ValueType::Int, BinaryOp::Greater, ValueType::Int),
        (binary_greater::<i64>, ValueType::Bool),
    );
    module.binary_ops.insert(
        BinarySignature::new(ValueType::Int, BinaryOp::GreaterEqual, ValueType::Int),
        (binary_greater_equal::<i64>, ValueType::Bool),
    );
    module.binary_ops.insert(
        BinarySignature::new(ValueType::Int, BinaryOp::Less, ValueType::Int),
        (binary_lesser::<i64>, ValueType::Bool),
    );
    module.binary_ops.insert(
        BinarySignature::new(ValueType::Int, BinaryOp::LessEqual, ValueType::Int),
        (binary_lesser_equal::<i64>, ValueType::Bool),
    );
    module.binary_ops.insert(
        BinarySignature::new(ValueType::Int, BinaryOp::EqualEqual, ValueType::Int),
        (binary_equal::<i64, i64>, ValueType::Bool),
    );
    module.binary_ops.insert(
        BinarySignature::new(ValueType::Int, BinaryOp::BangEqual, ValueType::Int),
        (binary_not_equal::<i64, i64>, ValueType::Bool),
    );

    // Compare Float
    module.binary_ops.insert(
        BinarySignature::new(ValueType::Float, BinaryOp::Greater, ValueType::Float),
        (binary_greater::<f64>, ValueType::Bool),
    );
    module.binary_ops.insert(
        BinarySignature::new(ValueType::Float, BinaryOp::GreaterEqual, ValueType::Float),
        (binary_greater_equal::<f64>, ValueType::Bool),
    );
    module.binary_ops.insert(
        BinarySignature::new(ValueType::Float, BinaryOp::Less, ValueType::Float),
        (binary_lesser::<f64>, ValueType::Bool),
    );
    module.binary_ops.insert(
        BinarySignature::new(ValueType::Float, BinaryOp::LessEqual, ValueType::Float),
        (binary_lesser_equal::<f64>, ValueType::Bool),
    );
    module.binary_ops.insert(
        BinarySignature::new(ValueType::Float, BinaryOp::EqualEqual, ValueType::Float),
        (binary_equal::<f64, f64>, ValueType::Bool),
    );
    module.binary_ops.insert(
        BinarySignature::new(ValueType::Float, BinaryOp::BangEqual, ValueType::Float),
        (binary_not_equal::<f64, f64>, ValueType::Bool),
    );

    // Compare bool
    module.binary_ops.insert(
        BinarySignature::new(ValueType::Bool, BinaryOp::EqualEqual, ValueType::Bool),
        (binary_equal::<bool, bool>, ValueType::Bool),
    );
    module.binary_ops.insert(
        BinarySignature::new(ValueType::Bool, BinaryOp::BangEqual, ValueType::Bool),
        (binary_not_equal::<bool, bool>, ValueType::Bool),
    );

    // Strings
    module.binary_ops.insert(
        BinarySignature::new(ValueType::String, BinaryOp::Plus, ValueType::String),
        (binary_string_plus::<String, String>, ValueType::String),
    );
    module.binary_ops.insert(
        BinarySignature::new(ValueType::String, BinaryOp::Plus, ValueType::Int),
        (binary_string_plus::<String, i64>, ValueType::String),
    );
    module.binary_ops.insert(
        BinarySignature::new(ValueType::String, BinaryOp::Plus, ValueType::Float),
        (binary_string_plus::<String, i64>, ValueType::String),
    );
    module.binary_ops.insert(
        BinarySignature::new(ValueType::String, BinaryOp::Plus, ValueType::Bool),
        (binary_string_plus::<String, i64>, ValueType::String),
    );
}

fn get_assign_ops(module: &mut Module) {
    // Int
    module.assignment_ops.insert(
        (ValueType::Int, AssignmentOp::PlusEqual),
        binary_plus::<i64, i64>,
    );
    module.assignment_ops.insert(
        (ValueType::Int, AssignmentOp::MinusEqual),
        binary_minus::<i64, i64>,
    );
    module.assignment_ops.insert(
        (ValueType::Int, AssignmentOp::MultiplyEqual),
        binary_multiply::<i64, i64>,
    );
    module.assignment_ops.insert(
        (ValueType::Int, AssignmentOp::DivideEqual),
        binary_divide::<i64, i64>,
    );

    // Float
    module.assignment_ops.insert(
        (ValueType::Int, AssignmentOp::PlusEqual),
        binary_plus::<i64, i64>,
    );
    module.assignment_ops.insert(
        (ValueType::Int, AssignmentOp::MinusEqual),
        binary_minus::<i64, i64>,
    );
    module.assignment_ops.insert(
        (ValueType::Int, AssignmentOp::MultiplyEqual),
        binary_multiply::<i64, i64>,
    );
    module.assignment_ops.insert(
        (ValueType::Int, AssignmentOp::DivideEqual),
        binary_divide::<i64, i64>,
    );

    // String
    /*
    values.insert(
        (ValueType::String, AssignmentOp::PlusEqual),
        binary_plus::<String, String>,
    );*/
}

fn get_stringify_ops(module: &mut Module) {
    module
        .stringify_ops
        .insert(ValueType::Bool, stringify::<bool>);
    module
        .stringify_ops
        .insert(ValueType::Int, stringify::<i64>);
    module
        .stringify_ops
        .insert(ValueType::Float, stringify::<f64>);
}
