use std::{any::Any, sync::Arc};

use hashbrown::HashMap;

use crate::{
    Dynamic, Location, Module,
    instructor::ProgramValue,
    parser::{AssignmentOp, BinaryOp, Expr, Stmt, UnaryOp, VariableMutability},
};

struct Frame {
    scope: Vec<Scope>,
    floating_stack: Vec<(Location, ValueType)>,
    variable_slots: Vec<(Location, String, Option<ValueType>, VariableMutability)>,
    is_global: bool,
}

impl Frame {
    fn new(is_global: bool) -> Frame {
        Frame {
            scope: vec![Scope::default()],
            floating_stack: vec![],
            variable_slots: vec![],
            is_global,
        }
    }
}

#[derive(Default)]
struct Scope {}

pub struct Constructor {
    module: Arc<Module>,
    stmts: Vec<AStmt>,

    // [OP] A
    unary_operators: HashMap<(UnaryOp, ValueType), ValueType>,
    // A [OP] [B] -> [C]
    binary_operators: HashMap<(ValueType, BinaryOp, ValueType), ValueType>,

    call_frames: Vec<Frame>,
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
pub enum ValueType {
    Nil,
    Float,
    Int,
    Bool,
    String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConstantValue {
    Nil,
    Float(f64),
    Int(i64),
    Bool(bool),
    String(String),
}

impl ConstantValue {
    pub fn to_program_value(&self) -> Box<dyn ProgramValue> {
        match self {
            ConstantValue::Nil => todo!(),
            ConstantValue::Float(val) => Box::new(*val) as Box<dyn ProgramValue>,
            ConstantValue::Int(val) => Box::new(*val) as Box<dyn ProgramValue>,
            ConstantValue::Bool(val) => Box::new(*val) as Box<dyn ProgramValue>,
            ConstantValue::String(val) => Box::new(val.to_string()) as Box<dyn ProgramValue>,
        }
    }

    pub fn get_value_type(&self) -> ValueType {
        match self {
            ConstantValue::Nil => ValueType::Nil,
            ConstantValue::Float(_) => ValueType::Float,
            ConstantValue::Int(_) => ValueType::Int,
            ConstantValue::Bool(_) => ValueType::Bool,
            ConstantValue::String(_) => ValueType::String,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct VariableSlot {
    pub index: usize,
    pub is_global: bool,
}

impl VariableSlot {
    fn new(index: usize, is_global: bool) -> VariableSlot {
        VariableSlot { index, is_global }
    }
}

#[derive(Debug)]
pub enum AStmt {
    // Special
    Print(Location, Box<AStmt>),

    // Slot info, name, value stmt
    SetVariable(Location, VariableSlot, String, Box<AStmt>),
    // Slot info, name, assignment op, value stmt
    AssignVariable(Location, VariableSlot, String, AssignmentOp, Box<AStmt>),

    // Condition, body
    While(Location, Box<AStmt>, Box<AStmt>),
    Expression(Location, AExpr),
    Block(Location, Vec<AStmt>),
}

impl AStmt {
    fn get_location(&self) -> Location {
        match self {
            AStmt::SetVariable(location, _, _, _) => *location,
            AStmt::Expression(location, _) => *location,
            AStmt::While(location, _, _) => *location,
            AStmt::Block(location, _) => *location,
            AStmt::Print(location, _) => *location,
            AStmt::AssignVariable(location, _, _, _, _) => *location,
        }
    }

    pub fn get_value_type(&self) -> Option<ValueType> {
        match self {
            AStmt::Print(_, _) => None,
            AStmt::SetVariable(_, _, _, astmt) => astmt.get_value_type(),
            AStmt::Expression(_, aexpr) => Some(aexpr.get_value_type()),
            AStmt::While(_, _, astmt1) => astmt1.get_value_type(),
            AStmt::Block(_, astmts) => astmts.last().map(|v| v.get_value_type()).flatten(),
            AStmt::AssignVariable(_, _, _, _, astmt) => astmt.get_value_type(),
        }
    }
}

#[derive(Debug)]
pub enum AExpr {
    Constant(Location, ConstantValue),
    // [OP] A -> B
    Unary(Location, UnaryOp, Box<AExpr>, ValueType),
    // A [OP] B -> C
    Binary(Location, Box<AExpr>, BinaryOp, Box<AExpr>, ValueType),
    // Slot info, name, type
    RetrieveVariable(Location, VariableSlot, String, ValueType),
}

impl AExpr {
    fn get_location(&self) -> Location {
        match self {
            AExpr::Constant(location, _) => *location,
            AExpr::Unary(location, _, _, _) => *location,
            AExpr::Binary(location, _, _, _, _) => *location,
            AExpr::RetrieveVariable(location, _, _, _) => *location,
        }
    }

    pub fn get_value_type(&self) -> ValueType {
        match self {
            AExpr::Constant(_, constant_value) => constant_value.get_value_type(),
            AExpr::Unary(_, _, _, value_type) => *value_type,
            AExpr::Binary(_, _, _, _, value_type) => *value_type,
            AExpr::RetrieveVariable(_, _, _, value_type) => *value_type,
        }
    }
}

#[derive(Debug)]
pub struct ConstructError {
    location: Location,
    kind: ConstructErrorType,
}

impl ConstructError {
    fn new(location: Location, kind: ConstructErrorType) -> ConstructError {
        ConstructError { location, kind }
    }
}

#[derive(Debug)]
pub enum ConstructErrorType {
    NoUnaryOperator(UnaryOp, ValueType),
    NoBinaryOperator(ValueType, BinaryOp, ValueType),
    ConstantMustBeInitialized,
    InitializerDoesntProduce,
    AssignerDoesntProduce,
    VariableUndefined,
    VariableUninitialized,
    VariableImmutable,
    ExpectedBoolean,
    NoValue,
}

impl Constructor {
    pub fn new(module: Arc<Module>) -> Constructor {
        Self {
            module,
            stmts: vec![],

            unary_operators: get_unary_ops(),
            binary_operators: get_binary_ops(),

            call_frames: vec![Frame::new(true)],
        }
    }

    pub fn generate(mut self, stmts: &[Stmt]) -> Result<Vec<AStmt>, ConstructError> {
        for stmt in stmts {
            let stmt = self.statement(stmt)?;

            self.stmts.push(stmt);
        }

        Ok(self.stmts)
    }

    fn statement(&mut self, stmt: &Stmt) -> Result<AStmt, ConstructError> {
        let stmt = match stmt {
            Stmt::DefineVariable(token, initializer, mutability) => {
                let initializer = if let Some(initializer) = initializer {
                    Some(Box::new(self.statement(initializer)?))
                } else {
                    None
                };
                let slot = self.alloc_variable(
                    token.location,
                    &token.lexeme,
                    initializer.as_ref(),
                    *mutability,
                )?;

                AStmt::SetVariable(
                    token.location,
                    slot,
                    token.lexeme.to_owned(),
                    // Set it to the expression, *OR* an empty slot
                    initializer.unwrap_or_else(|| {
                        Box::new(AStmt::Expression(
                            token.location,
                            AExpr::Constant(token.location, ConstantValue::Nil),
                        ))
                    }),
                )
            }
            Stmt::Expression(expr, _) => {
                let expr = self.expression(expr)?;
                AStmt::Expression(expr.get_location(), expr)
            }
            Stmt::Assign(name, op, stmt) => {
                let stmt = self.statement(stmt)?;
                let Some(stmt_type) = stmt.get_value_type() else {
                    return Err(ConstructError::new(
                        stmt.get_location(),
                        ConstructErrorType::AssignerDoesntProduce,
                    ));
                };

                let (slot, mutability, value_type) =
                    self.find_variable(stmt.get_location(), name)?;

                // Make sure mutability iss followed
                match mutability {
                    VariableMutability::Constant => {
                        return Err(ConstructError::new(
                            stmt.get_location(),
                            ConstructErrorType::VariableImmutable,
                        ));
                    }
                    VariableMutability::Immutable => {
                        if value_type.is_some() {
                            return Err(ConstructError::new(
                                stmt.get_location(),
                                ConstructErrorType::VariableImmutable,
                            ));
                        }
                    }
                    VariableMutability::Mutable => {}
                }

                self.modify_variable(&slot, stmt_type);

                AStmt::AssignVariable(
                    stmt.get_location(),
                    slot,
                    name.to_owned(),
                    *op,
                    Box::new(stmt),
                )
            }
            Stmt::While(condition, body) => {
                let condition = self.statement(condition)?;
                if condition.get_value_type() != Some(ValueType::Bool) {
                    return Err(ConstructError::new(
                        condition.get_location(),
                        ConstructErrorType::ExpectedBoolean,
                    ));
                }
                let body = self.statement(body)?;

                AStmt::While(
                    condition.get_location(),
                    Box::new(condition),
                    Box::new(body),
                )
            }
            Stmt::Block(stmts) => {
                let mut values = Vec::with_capacity(stmts.len());
                for stmt in stmts {
                    values.push(self.statement(stmt)?);
                }

                AStmt::Block(values.first().unwrap().get_location(), values)
            }
            Stmt::Print(stmt) => {
                let stmt = self.statement(stmt)?;
                if stmt.get_value_type().is_none() {
                    return Err(ConstructError::new(
                        stmt.get_location(),
                        ConstructErrorType::NoValue,
                    ));
                }

                AStmt::Print(stmt.get_location(), Box::new(stmt))
            }
            a => todo!("{a:?}"),
        };
        Ok(stmt)
    }

    fn alloc_variable(
        &mut self,
        source: Location,
        name: &str,
        initializer: Option<&Box<AStmt>>,
        mutability: VariableMutability,
    ) -> Result<VariableSlot, ConstructError> {
        if initializer.is_none() && mutability == VariableMutability::Constant {
            return Err(ConstructError::new(
                source,
                ConstructErrorType::ConstantMustBeInitialized,
            ));
        }

        let current_frame = self.call_frames.last_mut().unwrap();

        let initialized_type = if let Some(initializer) = initializer {
            if let Some(value) = initializer.get_value_type() {
                Some(value)
            } else {
                return Err(ConstructError::new(
                    source,
                    ConstructErrorType::InitializerDoesntProduce,
                ));
            }
        } else {
            None
        };

        let index = current_frame.variable_slots.len();
        current_frame
            .variable_slots
            .push((source, name.to_string(), initialized_type, mutability));

        Ok(VariableSlot::new(index, self.call_frames.len() <= 1))
    }

    fn find_variable(
        &mut self,
        source: Location,
        name: &str,
    ) -> Result<(VariableSlot, VariableMutability, Option<ValueType>), ConstructError> {
        let current_frame = self.call_frames.last_mut().unwrap();
        let is_global = current_frame.is_global;

        for entry in current_frame.variable_slots.iter().enumerate().rev() {
            let i = entry.0;
            let entry = entry.1;

            if entry.1 == name {
                return Ok((VariableSlot::new(i, is_global), entry.3, entry.2));
            }
        }

        if self.call_frames.len() > 1 {
            let global_frame = self.call_frames.first_mut().unwrap();

            for entry in global_frame.variable_slots.iter().enumerate().rev() {
                let i = entry.0;
                let entry = entry.1;

                if entry.1 == name {
                    return Ok((VariableSlot::new(i, true), entry.3, entry.2));
                }
            }
        }

        Err(ConstructError::new(
            source,
            ConstructErrorType::VariableUndefined,
        ))
    }

    fn modify_variable(&mut self, slot: &VariableSlot, value: ValueType) {
        if slot.is_global {
            let global_frame = self.call_frames.first_mut().unwrap();
            global_frame.variable_slots[slot.index].2 = Some(value);
        } else {
            let current_frame = self.call_frames.last_mut().unwrap();
            current_frame.variable_slots[slot.index].2 = Some(value);
        }
    }

    fn push_stack(&mut self, source: Location, value: ValueType) {
        self.call_frames
            .last_mut()
            .unwrap()
            .floating_stack
            .push((source, value));
    }

    fn pop_stack(&mut self) -> Option<(Location, ValueType)> {
        self.call_frames.last_mut().unwrap().floating_stack.pop()
    }

    fn expression(&mut self, expr: &Expr) -> Result<AExpr, ConstructError> {
        Ok(match expr {
            Expr::Float(location, val) => {
                self.push_stack(*location, ValueType::Float);
                AExpr::Constant(*location, ConstantValue::Float(*val))
            }
            Expr::Integer(location, val) => {
                self.push_stack(*location, ValueType::Int);
                AExpr::Constant(*location, ConstantValue::Int(*val))
            }
            Expr::Bool(location, val) => {
                self.push_stack(*location, ValueType::Bool);
                AExpr::Constant(*location, ConstantValue::Bool(*val))
            }
            Expr::String(location, val) => {
                self.push_stack(*location, ValueType::String);
                AExpr::Constant(*location, ConstantValue::String(val.to_owned()))
            }
            Expr::Nil(location) => {
                self.push_stack(*location, ValueType::Nil);
                AExpr::Constant(*location, ConstantValue::Nil)
            }
            Expr::Unary(op, a) => {
                let expr_a = self.expression(a)?;

                let stack_a = self.pop_stack().unwrap();

                let Some(result_type) = self.unary_operators.get(&(*op, stack_a.1)).copied() else {
                    // We don't support this
                    return Err(ConstructError::new(
                        a.get_location(),
                        ConstructErrorType::NoUnaryOperator(*op, stack_a.1),
                    ));
                };

                self.push_stack(expr_a.get_location(), result_type);

                AExpr::Unary(a.get_location(), *op, Box::new(expr_a), result_type)
            }
            Expr::Binary(a, op, b) => {
                let expr_a = self.expression(a)?;
                let expr_b = self.expression(b)?;

                let stack_b = self.pop_stack().unwrap();
                let stack_a = self.pop_stack().unwrap();

                let Some(result_type) = self
                    .binary_operators
                    .get(&(stack_a.1, *op, stack_b.1))
                    .copied()
                else {
                    // We don't support this
                    return Err(ConstructError::new(
                        a.get_location(),
                        ConstructErrorType::NoBinaryOperator(stack_a.1, *op, stack_b.1),
                    ));
                };

                self.push_stack(expr_a.get_location(), result_type);

                AExpr::Binary(
                    expr.get_location(),
                    Box::new(expr_a),
                    *op,
                    Box::new(expr_b),
                    result_type,
                )
            }
            Expr::Variable(location, name) => {
                let (slot, _, variable) = self.find_variable(*location, name)?;

                let Some(value_type) = variable else {
                    return Err(ConstructError::new(
                        *location,
                        ConstructErrorType::VariableUninitialized,
                    ));
                };

                self.push_stack(*location, value_type);

                AExpr::RetrieveVariable(*location, slot, name.to_owned(), value_type)
            }
            Expr::Condition(expr, token, expr1) => todo!(),
            Expr::Call(expr, exprs) => todo!(),
        })
    }
}

fn get_unary_ops() -> HashMap<(UnaryOp, ValueType), ValueType> {
    let mut unary_operators: HashMap<(UnaryOp, ValueType), ValueType> = Default::default();

    for numeric in [ValueType::Int, ValueType::Float] {
        // Math
        unary_operators.insert((UnaryOp::Negate, numeric), numeric);
    }

    unary_operators.insert((UnaryOp::Invert, ValueType::Bool), ValueType::Bool);

    unary_operators
}

fn get_binary_ops() -> HashMap<(ValueType, BinaryOp, ValueType), ValueType> {
    let mut binary_operators: HashMap<(ValueType, BinaryOp, ValueType), ValueType> =
        Default::default();

    for numeric in [ValueType::Int, ValueType::Float] {
        // Math
        binary_operators.insert((numeric, BinaryOp::Plus, numeric), numeric);
        binary_operators.insert((numeric, BinaryOp::Minus, numeric), numeric);
        binary_operators.insert((numeric, BinaryOp::Multiply, numeric), numeric);
        binary_operators.insert((numeric, BinaryOp::Divide, numeric), numeric);

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
    );

    binary_operators
}
