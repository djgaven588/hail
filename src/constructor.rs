use std::sync::Arc;

use hashbrown::HashMap;

use crate::{
    Location, Module,
    instructor::ProgramValue,
    module::{BinaryFn, BinarySignature, UnaryFn, UnarySignature},
    parser::{AssignmentOp, BinaryOp, Expr, Stmt, UnaryOp, VariableMutability},
};

#[derive(Debug)]
pub struct Scope {
    // Variable index, current type (unavailable = undefined)
    variable_state: HashMap<usize, ValueType>,
    starting_variable_count: usize,
    is_global: bool,
}

impl Scope {
    pub fn new(start_count: usize, is_global: bool) -> Scope {
        Scope {
            variable_state: Default::default(),
            starting_variable_count: start_count,
            is_global,
        }
    }
}

struct Frame {
    scope: Vec<Scope>,
    floating_stack: Vec<(Location, ValueType)>,
    is_global: bool,
    variable_slots: Vec<(Location, String, VariableMutability)>,
}

impl Frame {
    fn new(is_global: bool) -> Frame {
        Frame {
            scope: vec![Scope::new(0, is_global)],
            floating_stack: vec![],
            variable_slots: vec![],
            is_global,
        }
    }
}

pub struct Constructor {
    module: Arc<Module>,
    stmts: Vec<AStmt>,

    call_frames: Vec<Frame>,
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
pub enum ValueType {
    Float,
    Int,
    Bool,
    String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConstantValue {
    Float(f64),
    Int(i64),
    Bool(bool),
    String(String),
}

impl ConstantValue {
    pub fn to_program_value(&self) -> Box<dyn ProgramValue> {
        match self {
            ConstantValue::Float(val) => Box::new(*val) as Box<dyn ProgramValue>,
            ConstantValue::Int(val) => Box::new(*val) as Box<dyn ProgramValue>,
            ConstantValue::Bool(val) => Box::new(*val) as Box<dyn ProgramValue>,
            ConstantValue::String(val) => Box::new(val.to_string()) as Box<dyn ProgramValue>,
        }
    }

    pub fn get_value_type(&self) -> ValueType {
        match self {
            ConstantValue::Float(_) => ValueType::Float,
            ConstantValue::Int(_) => ValueType::Int,
            ConstantValue::Bool(_) => ValueType::Bool,
            ConstantValue::String(_) => ValueType::String,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

    // Slot info, name, initializer
    SetVariable(Location, VariableSlot, String, Option<Box<AStmt>>),
    // Slot info, name, assignment op, value stmt
    AssignVariable(Location, VariableSlot, String, AssignmentOp, Box<AStmt>),

    // Condition, body
    While(Location, Box<AStmt>, Box<AStmt>),
    Expression(Location, AExpr),
    Block(Location, Vec<AStmt>, Scope),

    // Function name, parameters, body, return type (if any)
    Function(
        Location,
        String,
        Vec<(VariableSlot, String, ValueType)>,
        Box<AStmt>,
        Option<ValueType>,
    ),
}

impl AStmt {
    fn get_location(&self) -> Location {
        match self {
            AStmt::SetVariable(location, _, _, _) => *location,
            AStmt::Expression(location, _) => *location,
            AStmt::While(location, _, _) => *location,
            AStmt::Block(location, _, _) => *location,
            AStmt::Print(location, _) => *location,
            AStmt::AssignVariable(location, _, _, _, _) => *location,
            AStmt::Function(location, _, _, _, _) => *location,
        }
    }

    /// Recursively search for the value a statment returns
    pub fn get_value_type(&self) -> Option<ValueType> {
        match self {
            AStmt::Print(_, _) => None,
            AStmt::Expression(_, aexpr) => Some(aexpr.get_value_type()),
            AStmt::While(_, _, astmt1) => astmt1.get_value_type(),
            AStmt::Block(_, astmts, _) => astmts.last().map(|v| v.get_value_type()).flatten(),
            AStmt::SetVariable(_, _, _, _) => None,
            AStmt::AssignVariable(_, _, _, _, _) => None,
            AStmt::Function(_, _, _, _, value_type) => *value_type,
        }
    }

    /// Recursively search for any return statements and get their return types
    pub fn get_returns(&self) -> Vec<(Location, Option<ValueType>)> {
        match self {
            AStmt::Print(_, astmt) => astmt.get_returns(),
            AStmt::SetVariable(_, _, _, astmt) => {
                astmt.as_ref().map_or(vec![], |v| v.get_returns())
            }
            AStmt::AssignVariable(_, _, _, _, astmt) => astmt.get_returns(),
            AStmt::Expression(_, _) => vec![],
            AStmt::While(_, astmt0, astmt1) => {
                let mut cond = astmt0.get_returns();
                cond.append(&mut astmt1.get_returns());

                cond
            }
            AStmt::Block(_, astmts, _) => {
                astmts.iter().map(|v| v.get_returns()).flatten().collect()
            }
            AStmt::Function(_, _, _, body, _) => body.get_returns(),
        }
    }

    pub fn retrieved_variables(&self) -> Vec<(VariableSlot, ValueType)> {
        match self {
            AStmt::Print(_, _)
            | AStmt::SetVariable(_, _, _, _)
            | AStmt::AssignVariable(_, _, _, _, _)
            | AStmt::While(_, _, _) => return vec![],
            AStmt::Expression(_, aexpr) => aexpr.retrieved_variables(),
            AStmt::Block(_, astmts, _) => {
                let mut results = vec![];
                for stmt in astmts {
                    for var in stmt.retrieved_variables() {
                        results.push(var);
                    }
                }

                results
            }
            AStmt::Function(_, _, _, astmt, _) => astmt.retrieved_variables(),
        }
    }

    pub fn modified_variables(&self) -> Vec<(VariableSlot, ValueType)> {
        match self {
            AStmt::Expression(_, _) => vec![],
            AStmt::SetVariable(_, _, _, astmt) => {
                astmt.as_ref().map_or(vec![], |v| v.modified_variables())
            }
            AStmt::While(_, astmt_cond, astmt_body) => {
                let mut cond = astmt_cond.modified_variables();
                let mut body = astmt_body.modified_variables();
                cond.append(&mut body);

                cond
            }
            AStmt::Print(_, astmt) => astmt.modified_variables(),
            AStmt::AssignVariable(_, slot, _, _, aexpr) => {
                vec![(*slot, aexpr.get_value_type().expect("Should be valid"))]
            }
            AStmt::Block(_, _, scope) => {
                let mut values = vec![];
                for var in &scope.variable_state {
                    let slot = VariableSlot::new(*var.0, scope.is_global);
                    values.push((slot, *var.1));
                }

                values
            }
            AStmt::Function(_, _, _, astmt, _) => astmt.modified_variables(),
        }
    }
}

#[derive(Debug)]
pub enum AExpr {
    Constant(Location, ConstantValue),
    // [OP] A -> B
    Unary(Location, UnaryFn, Box<AExpr>, ValueType),
    // A [OP] B -> C
    Binary(Location, Box<AExpr>, BinaryFn, Box<AExpr>, ValueType),
    // Slot info, name, type
    RetrieveVariable(Location, VariableSlot, String, ValueType),
}

impl AExpr {
    /// What variables has this interacted with?
    fn retrieved_variables(&self) -> Vec<(VariableSlot, ValueType)> {
        match self {
            AExpr::Constant(_, _) => vec![],
            AExpr::Unary(_, _, aexpr, _) => aexpr.retrieved_variables(),
            AExpr::Binary(_, aexpr_a, _, aexpr_b, _) => {
                let mut a_vars = aexpr_a.retrieved_variables();
                let mut b_vars = aexpr_b.retrieved_variables();
                a_vars.append(&mut b_vars);

                a_vars
            }
            AExpr::RetrieveVariable(_, variable_slot, _, value_type) => {
                vec![(*variable_slot, *value_type)]
            }
        }
    }

    /// Where in the source is this located?
    fn get_location(&self) -> Location {
        match self {
            AExpr::Constant(location, _) => *location,
            AExpr::Unary(location, _, _, _) => *location,
            AExpr::Binary(location, _, _, _, _) => *location,
            AExpr::RetrieveVariable(location, _, _, _) => *location,
        }
    }

    /// What value does this produce?
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
    // Expected type, modified type
    VariableTypeAltered(ValueType, ValueType),
    ExpectedBoolean,
    NoValue,
    UnexpectedValue,
    CantStringify(ValueType),
    UnknownType(String),
    // Expected, provided
    UnexpectedReturnValue(Option<ValueType>, Option<ValueType>),
}

impl Constructor {
    pub fn new(module: Arc<Module>) -> Constructor {
        Self {
            module,
            stmts: vec![],

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
        println!("Stmt: {stmt:?}");
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
                    initializer,
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

                let condition_variables = condition.retrieved_variables();
                println!("Conditional: {condition_variables:?}");

                let body = self.statement(body)?;
                println!("Modified: {:?}", body.modified_variables());
                for modified in body.modified_variables() {
                    if let Some(original) = condition_variables.iter().find(|v| v.0 == modified.0) {
                        // Make sure we've not fucked it up
                        if original.1 != modified.1 {
                            return Err(ConstructError::new(
                                body.get_location(),
                                ConstructErrorType::VariableTypeAltered(original.1, modified.1),
                            ));
                        }
                    }
                }

                if body.get_value_type().is_some() {
                    println!("Value type: {:?}", body.get_value_type());
                    // If we have an output, we shouldn't.
                    return Err(ConstructError::new(
                        body.get_location(),
                        ConstructErrorType::UnexpectedValue,
                    ));
                }

                // Ensure condition variables aren't type altered
                for var in condition_variables {
                    match self.type_check_variable(&var.0, var.1) {
                        Ok(_) => {}
                        Err(err) => {
                            return Err(ConstructError::new(
                                condition.get_location(),
                                if let Some(err) = err {
                                    ConstructErrorType::VariableTypeAltered(var.1, err)
                                } else {
                                    ConstructErrorType::VariableUndefined
                                },
                            ));
                        }
                    }
                }

                // Variable scope typing is handled by blocks
                AStmt::While(
                    condition.get_location(),
                    Box::new(condition),
                    Box::new(body),
                )
            }
            Stmt::Block(location, stmts) => {
                self.push_scope();

                let mut values = Vec::with_capacity(stmts.len());
                for stmt in stmts {
                    values.push(self.statement(stmt)?);
                }

                let scope = self.pop_scope();

                AStmt::Block(*location, values, scope)
            }
            Stmt::Print(stmt) => {
                let stmt = self.statement(stmt)?;
                let Some(value_type) = stmt.get_value_type() else {
                    return Err(ConstructError::new(
                        stmt.get_location(),
                        ConstructErrorType::NoValue,
                    ));
                };

                if value_type != ValueType::String {
                    // Make sure we can do the string conversion
                    if self.module.stringify_ops.get(&value_type).is_none() {
                        return Err(ConstructError::new(
                            stmt.get_location(),
                            ConstructErrorType::CantStringify(value_type),
                        ));
                    }
                }

                AStmt::Print(stmt.get_location(), Box::new(stmt))
            }
            Stmt::Function(name, params, body, specified_return) => {
                // Enter new framing
                self.push_frame();

                // Push all parameters
                let mut built_params = Vec::with_capacity(params.len());
                for param in params {
                    let Some(param_type) = self.module.resolve_type(&param.typing.lexeme) else {
                        return Err(ConstructError::new(
                            param.typing.location,
                            ConstructErrorType::UnknownType(param.typing.lexeme.clone()),
                        ));
                    };

                    // Add them to the stack (we know the typing)
                    let slot = self.push_variable(
                        param.name.location,
                        &param.name.lexeme,
                        param.mutability,
                        Some(param_type),
                    )?;

                    built_params.push((slot, param.name.lexeme.clone(), param_type));
                }

                // Parse the body
                let body = Box::new(self.statement(body)?);

                // Figure out what this returns
                // TODO: Return statements aren't considered here!
                let return_type = if let Some(specified) = specified_return {
                    let Some(specified_return) = self.module.resolve_type(&specified.lexeme) else {
                        return Err(ConstructError::new(
                            specified.location,
                            ConstructErrorType::UnknownType(specified.lexeme.clone()),
                        ));
                    };

                    Some(specified_return)
                } else {
                    // If we have no specified return, get the body return
                    body.get_value_type()
                };

                // Check the return type against the body
                if return_type != body.get_value_type() {
                    return Err(ConstructError::new(
                        body.get_location(),
                        ConstructErrorType::UnexpectedReturnValue(
                            return_type,
                            body.get_value_type(),
                        ),
                    ));
                }

                // Ensure returns align with expected typing
                for entry in body.get_returns() {
                    if entry.1 != return_type {
                        return Err(ConstructError::new(
                            entry.0,
                            ConstructErrorType::UnexpectedReturnValue(return_type, entry.1),
                        ));
                    }
                }

                // End our framing
                self.pop_frame();

                AStmt::Function(
                    name.location,
                    name.lexeme.to_string(),
                    built_params,
                    body,
                    return_type,
                )
            }
            a => todo!("{a:?}"),
        };
        Ok(stmt)
    }

    fn push_frame(&mut self) {
        self.call_frames.push(Frame::new(false));
    }

    fn pop_frame(&mut self) {
        self.call_frames.pop().expect("Should have call frame");
    }

    fn push_scope(&mut self) {
        let call_frame = self.call_frames.last_mut().expect("Should have call frame");
        call_frame.scope.push(Scope::new(
            call_frame.variable_slots.len(),
            call_frame.is_global,
        ));
    }

    fn pop_scope(&mut self) -> Scope {
        let call_frame = self.call_frames.last_mut().expect("Should have call frame");
        // Get scope, we need to handle variable typing
        let scope = call_frame.scope.pop().expect("Should have scope");

        // Clear slots
        call_frame
            .variable_slots
            .truncate(scope.starting_variable_count);

        let calling_scope = call_frame.scope.last_mut().expect("Expected scope");
        for var in &scope.variable_state {
            calling_scope.variable_state.insert(*var.0, *var.1);
        }

        // TODO: This needs to apply the scope's variable changes
        scope
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

        self.push_variable(source, name, mutability, initialized_type)
    }

    fn push_variable(
        &mut self,
        source: Location,
        name: &str,
        mutability: VariableMutability,
        initialized_type: Option<ValueType>,
    ) -> Result<VariableSlot, ConstructError> {
        let current_frame = self.call_frames.last_mut().unwrap();

        let index = current_frame.variable_slots.len();
        // Mark typing info for this scope
        if let Some(initialized_type) = initialized_type {
            current_frame
                .scope
                .last_mut()
                .expect("Should have scope")
                .variable_state
                .insert(current_frame.variable_slots.len(), initialized_type);
        }

        current_frame
            .variable_slots
            .push((source, name.to_string(), mutability));

        Ok(VariableSlot::new(index, self.call_frames.len() <= 1))
    }

    fn find_variable(
        &mut self,
        source: Location,
        name: &str,
    ) -> Result<(VariableSlot, VariableMutability, Option<ValueType>), ConstructError> {
        {
            let current_frame = self.call_frames.last_mut().unwrap();
            let is_global = current_frame.is_global;

            for entry in current_frame.variable_slots.iter().enumerate().rev() {
                let i = entry.0;
                let entry = entry.1;

                if entry.1 != name {
                    continue;
                }

                let mut was_found = false;
                for scope in current_frame.scope.iter().rev() {
                    // Try and find the variable, we're looking for the *current* type
                    if let Some(var) = scope.variable_state.get(&i) {
                        return Ok((VariableSlot::new(i, is_global), entry.2, Some(*var)));
                    } else {
                        was_found = true;
                    }
                }

                if was_found {
                    // Oh no! It's not initialized :(
                    return Ok((VariableSlot::new(i, is_global), entry.2, None));
                }

                // We've already found the variable, move on
                break;
            }
        }

        if self.call_frames.len() > 1 {
            let global_frame = self.call_frames.first_mut().unwrap();

            for entry in global_frame.variable_slots.iter().enumerate().rev() {
                let i = entry.0;
                let entry = entry.1;

                if entry.1 != name {
                    continue;
                }

                let mut was_found = false;
                for scope in global_frame.scope.iter().rev() {
                    // Try and find the variable, we're looking for the *current* type
                    if let Some(var) = scope.variable_state.get(&i) {
                        return Ok((VariableSlot::new(i, true), entry.2, Some(*var)));
                    } else {
                        was_found = true;
                    }
                }

                if was_found {
                    // Oh no! It's not initialized :(
                    return Ok((VariableSlot::new(i, true), entry.2, None));
                }

                // We've already found the variable, move on
                break;
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
            global_frame
                .scope
                .last_mut()
                .expect("Should have scope")
                .variable_state
                .insert(slot.index, value);
            //global_frame.variable_slots[slot.index].2 = Some(value);
        } else {
            let current_frame = self.call_frames.last_mut().unwrap();

            current_frame
                .scope
                .last_mut()
                .expect("Should have scope")
                .variable_state
                .insert(slot.index, value);
            //current_frame.variable_slots[slot.index].2 = Some(value);
        }
    }

    fn type_check_variable(
        &mut self,
        slot: &VariableSlot,
        value: ValueType,
    ) -> Result<(), Option<ValueType>> {
        let frame = if slot.is_global {
            self.call_frames.first_mut().unwrap()
        } else {
            self.call_frames.last_mut().unwrap()
        };

        // TODO: Something here is wrong due to the above frame selection
        for scope in frame.scope.iter().rev() {
            // Try and find the variable, we're looking for the *current* type
            if let Some(var) = scope.variable_state.get(&slot.index) {
                if var == &value {
                    return Ok(());
                } else {
                    return Err(Some(*var));
                }
            }
        }

        Err(None)
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
        println!("Expr: {expr:?}");
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
            Expr::Unary(location, op, a) => {
                let expr_a = self.expression(a)?;

                let stack_a = self.pop_stack().unwrap();

                let Some(result_type) = self
                    .module
                    .unary_ops
                    .get(&UnarySignature::new(*op, stack_a.1))
                    .copied()
                else {
                    // We don't support this
                    return Err(ConstructError::new(
                        a.get_location(),
                        ConstructErrorType::NoUnaryOperator(*op, stack_a.1),
                    ));
                };

                self.push_stack(expr_a.get_location(), result_type.1);

                AExpr::Unary(*location, result_type.0, Box::new(expr_a), result_type.1)
            }
            Expr::Binary(a, op, b) => {
                let expr_a = self.expression(a)?;
                let expr_b = self.expression(b)?;

                let stack_b = self.pop_stack().unwrap();
                let stack_a = self.pop_stack().unwrap();

                let Some(result_type) = self
                    .module
                    .binary_ops
                    .get(&BinarySignature::new(stack_a.1, *op, stack_b.1))
                    .copied()
                else {
                    // We don't support this
                    return Err(ConstructError::new(
                        a.get_location(),
                        ConstructErrorType::NoBinaryOperator(stack_a.1, *op, stack_b.1),
                    ));
                };

                self.push_stack(expr_a.get_location(), result_type.1);

                AExpr::Binary(
                    expr.get_location(),
                    Box::new(expr_a),
                    result_type.0,
                    Box::new(expr_b),
                    result_type.1,
                )
            }
            Expr::Variable(location, name) => {
                let (slot, _, variable) = self.find_variable(*location, name)?;
                println!(
                    "Variable [{name}]: {slot:?} : {variable:?}, Scope: {:?}",
                    self.call_frames.last().unwrap().scope
                );
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
