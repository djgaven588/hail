use crate::SyncProgramValue;
use crate::{FunctionInfo, TokenType, visualize};
use std::fmt::format;
use std::{
    any::TypeId,
    collections::HashSet,
    hash::{DefaultHasher, Hash, Hasher},
    sync::Arc,
};

use hashbrown::HashMap;

use crate::{
    Location, Module,
    instructor::{Instructor, Program},
    module::FunctionKind,
    parser::{
        AssignmentOp, BinaryOp, Expr, FunctionParameter, LogicalOp, ParseError, Parser, Stmt,
        UnaryOp, VariableMutability,
    },
    scanner::{Scanner, SyntaxError, Token},
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MaybeValueType {
    Set(ValueType),
    Maybe(ValueType),
}

impl MaybeValueType {
    pub fn value_type(&self) -> ValueType {
        match self {
            MaybeValueType::Set(value_type) => value_type.clone(),
            MaybeValueType::Maybe(value_type) => value_type.clone(),
        }
    }

    fn to_maybe(&self) -> MaybeValueType {
        match self {
            MaybeValueType::Set(value_type) | MaybeValueType::Maybe(value_type) => {
                MaybeValueType::Maybe(value_type.clone())
            }
        }
    }
}

#[derive(Debug)]
pub struct Scope {
    // Variable index, current type (unavailable = undefined)
    variable_state: HashMap<usize, MaybeValueType>,
    starting_variable_count: usize,
    // Name, index
    available_functions: HashMap<String, usize>,
    is_global: bool,
}

impl Scope {
    pub fn new(start_count: usize, is_global: bool) -> Scope {
        Scope {
            variable_state: Default::default(),
            starting_variable_count: start_count,
            available_functions: Default::default(),
            is_global,
        }
    }
}

#[derive(Debug)]
struct Frame {
    scope: Vec<Scope>,
    floating_stack: Vec<(Location, ValueType)>,
    is_global: bool,
    variable_slots: Vec<(Location, String, VariableMutability)>,
    loop_depth: usize,
}

impl Frame {
    fn new(is_global: bool) -> Frame {
        Frame {
            scope: vec![Scope::new(0, is_global)],
            floating_stack: vec![],
            variable_slots: vec![],
            is_global,
            loop_depth: 0,
        }
    }
}

#[derive(Default, Clone)]
pub struct MemoryImportResolver {
    pub script_files: std::collections::HashMap<String, String>,
}

impl ImportResolver for MemoryImportResolver {
    fn get_script(&self, path: &str) -> Result<String, String> {
        Ok(self
            .script_files
            .get(path)
            .ok_or("Failed to read.")?
            .to_string())
    }

    fn resolve_path(&self, _base: Option<&str>, path: &str) -> Result<String, String> {
        Ok(path.to_string())
    }
}

pub trait ImportResolver
where
    Self: Send + Sync,
{
    fn get_script(&self, path: &str) -> Result<String, String>;

    fn resolve_path(&self, base: Option<&str>, path: &str) -> Result<String, String>;
}

// TODO: Strip this value type of Float, Int, Bool, and String, they can be custom types now
#[derive(Debug, Eq, Clone)]
pub enum ValueType {
    Float,
    Int,
    Bool,
    String,
    Range(Box<ValueType>),
    Option(Box<ValueType>),
    Function(String, Vec<ValueType>, Option<Box<ValueType>>),
    CustomType(TypeId, String),
}

// CustomTypes are stupid
impl PartialEq for ValueType {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::CustomType(self_type, _), Self::CustomType(other_type, _)) => {
                self_type == other_type
            }
            _ => core::mem::discriminant(self) == core::mem::discriminant(other),
        }
    }
}

// CustomTypes are stupid, see above
impl std::hash::Hash for ValueType {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            ValueType::CustomType(type_id, _) => type_id.hash(state),
            _ => core::mem::discriminant(self).hash(state),
        };
    }
}

impl std::fmt::Display for ValueType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValueType::Float => write!(f, "f64"),
            ValueType::Int => write!(f, "i64"),
            ValueType::Bool => write!(f, "bool"),
            ValueType::String => write!(f, "String"),
            ValueType::Range(inner) => write!(f, "Range<{inner}>"),
            ValueType::Option(inner) => write!(f, "Option<{inner}>"),
            ValueType::Function(_name, params, ret) => {
                let params_str: Vec<String> = params.iter().map(|p| p.to_string()).collect();
                let ret_str = ret.as_ref().map(|r| format!(" -> {r}")).unwrap_or_default();
                write!(f, "fn({}){ret_str}", params_str.join(", "))
            }
            ValueType::CustomType(_, name) => write!(f, "{name}"),
        }
    }
}

impl std::fmt::Display for crate::scanner::SyntaxErrorType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            crate::scanner::SyntaxErrorType::UnexpectedCharacter(c) => {
                write!(f, "unexpected character '{c}'")
            }
            crate::scanner::SyntaxErrorType::UnterminatedString => write!(f, "unterminated string"),
            crate::scanner::SyntaxErrorType::UnterminatedComment => {
                write!(f, "unterminated comment")
            }
        }
    }
}

impl std::fmt::Display for MaybeValueType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MaybeValueType::Set(inner) => write!(f, "{inner}"),
            MaybeValueType::Maybe(inner) => write!(f, "maybe {inner}"),
        }
    }
}

impl ValueType {
    pub fn of<T: 'static>() -> ValueType {
        let type_of_t = std::any::TypeId::of::<T>();
        if std::any::TypeId::of::<i64>() == type_of_t {
            ValueType::Int
        } else if std::any::TypeId::of::<f64>() == type_of_t {
            ValueType::Float
        } else if std::any::TypeId::of::<bool>() == type_of_t {
            ValueType::Bool
        } else if std::any::TypeId::of::<String>() == type_of_t {
            ValueType::String
        } else {
            ValueType::CustomType(
                std::any::TypeId::of::<T>(),
                std::any::type_name::<T>().to_string(),
            )
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallSource {
    Variable(VariableSlot),
    Stack,
}

#[derive(Debug)]
pub enum AStmt {
    // Special
    Print(Location, Box<AExpr>),

    // Slot info, name, initializer
    SetVariable(Location, VariableSlot, String, Option<Box<AExpr>>),

    Loop(Box<AExpr>),
    // Condition, body
    While(Location, Box<AExpr>, Box<AExpr>),
    // Expression, can produce
    Expression(Location, AExpr, bool),

    Return(Location, Option<Box<AExpr>>),
    Break(Location),
    Continue(Location),

    // Path, namespace, import index
    Import(Location, String, String, usize),

    // Function name, index, parameters, body, return type (if any), local variable count
    Function(
        Location,
        String,
        usize,
        Vec<(VariableSlot, String, ValueType)>,
        Box<AExpr>,
        Option<ValueType>,
        usize,
    ),
}

impl AStmt {
    pub fn get_location(&self) -> Location {
        match self {
            AStmt::SetVariable(location, _, _, _) => *location,
            AStmt::Expression(location, _, _) => *location,
            AStmt::While(location, _, _) => *location,
            AStmt::Loop(expr) => expr.get_location(),
            AStmt::Print(location, _) => *location,
            AStmt::Return(loc, _) => *loc,
            AStmt::Break(loc) => *loc,
            AStmt::Continue(loc) => *loc,
            AStmt::Function(location, _, _, _, _, _, _) => *location,
            AStmt::Import(location, _, _, _) => *location,
        }
    }

    /// Recursively search for the value a statment returns
    pub fn get_value_type(&self) -> Option<MaybeValueType> {
        match self {
            AStmt::Print(_, _) => None,
            AStmt::Expression(_, aexpr, produces) => {
                if *produces {
                    aexpr.get_value_type()
                } else {
                    None
                }
            }
            AStmt::While(_, _, _) | AStmt::Loop(_) => None,
            AStmt::SetVariable(_, _, _, _) => None,
            AStmt::Return(_, _) => None,
            AStmt::Break(_) => None,
            AStmt::Continue(_) => None,
            AStmt::Function(_, _, _, _, _, value_type, _) => {
                value_type.clone().map(|v| MaybeValueType::Set(v))
            }
            AStmt::Import(_, _, _, _) => None,
        }
    }

    /// Recursively search for any return statements and get their return types
    pub fn get_returns(&self) -> Vec<(Location, Option<ValueType>)> {
        match self {
            AStmt::Print(_, aexpr) => aexpr.get_returns(),
            AStmt::SetVariable(_, _, _, aexpr) => {
                aexpr.as_ref().map_or(vec![], |v| v.get_returns())
            }
            AStmt::Expression(_, aexpr, _) => aexpr.get_returns(),
            AStmt::While(_, cond, body) => {
                let mut returns = cond.get_returns();
                returns.append(&mut body.get_returns());

                returns
            }
            AStmt::Loop(body) => body.get_returns(),
            AStmt::Return(loc, expr) => vec![(
                *loc,
                expr.as_ref()
                    .and_then(|e| e.get_value_type())
                    .map(|v| v.value_type()),
            )],
            AStmt::Break(_) => vec![],
            AStmt::Continue(_) => vec![],
            AStmt::Function(_, _, _, _, body, _, _) => body.get_returns(),
            AStmt::Import(_, _, _, _) => vec![],
        }
    }

    pub fn retrieved_variables(&self) -> Vec<(VariableSlot, MaybeValueType)> {
        match self {
            AStmt::Print(_, astmt) => astmt.retrieved_variables(),
            AStmt::SetVariable(_, _, _, astmt) => {
                if let Some(astmt) = astmt {
                    astmt.retrieved_variables()
                } else {
                    vec![]
                }
            }
            AStmt::While(_, condition, body) => {
                let mut retrieved = condition.retrieved_variables();
                retrieved.append(&mut body.retrieved_variables());

                retrieved
            }
            AStmt::Loop(body) => body.retrieved_variables(),
            AStmt::Expression(_, aexpr, _) => aexpr.retrieved_variables(),
            AStmt::Return(_, s) => s.as_ref().map_or(vec![], |s| s.retrieved_variables()),
            AStmt::Break(_) => vec![],
            AStmt::Continue(_) => vec![],
            AStmt::Function(_, _, _, _, astmt, _, _) => astmt.retrieved_variables(),
            AStmt::Import(_, _, _, _) => vec![],
        }
    }

    pub fn modified_variables(&self) -> Vec<(VariableSlot, MaybeValueType)> {
        match self {
            AStmt::Expression(_, expr, _) => expr.modified_variables(),
            AStmt::SetVariable(_, _, _, astmt) => {
                astmt.as_ref().map_or(vec![], |v| v.modified_variables())
            }
            AStmt::While(_, astmt_cond, astmt_body) => {
                let mut modified = astmt_cond.modified_variables();
                let mut body = astmt_body.modified_variables();

                // The body of the while loop may or may not execute, so it's "maybe" something
                body.iter_mut().for_each(|v| match &v.1 {
                    MaybeValueType::Set(value_type) | MaybeValueType::Maybe(value_type) => {
                        v.1 = MaybeValueType::Maybe(value_type.clone());
                    }
                });
                modified.append(&mut body);

                modified
            }
            AStmt::Loop(expr) => expr.modified_variables(),
            AStmt::Print(_, astmt) => astmt.modified_variables(),
            AStmt::Return(_, s) => s.as_ref().map_or(vec![], |s| s.modified_variables()),
            AStmt::Break(_) => vec![],
            AStmt::Continue(_) => vec![],
            AStmt::Function(_, _, _, _, astmt, _, _) => astmt.modified_variables(),
            AStmt::Import(_, _, _, _) => vec![],
        }
    }

    /// Returns true if this statement unconditionally exits control flow (return, break, continue).
    /// Used for if else type merging where one branch may unconditionally transfer control.
    pub fn is_unconditional_exit(&self) -> bool {
        match self {
            AStmt::Return(_, _) => true,
            AStmt::Break(_) => true,
            AStmt::Continue(_) => true,
            // Expression containing a block that ends with exit
            AStmt::Expression(_, inner_expr, _produces) => {
                match inner_expr {
                    AExpr::Block(_, stmts, _) => {
                        stmts.last().map_or(false, |s| s.is_unconditional_exit())
                    }
                    // Expression containing an if else where all paths exit
                    AExpr::If(_, body, otherwise) => {
                        body.is_unconditional_exit()
                            && match otherwise {
                                Some(o) => o.is_unconditional_exit(),
                                None => false,
                            }
                    }
                    _ => false,
                }
            }
            AStmt::Print(_, _) => false,
            AStmt::SetVariable(_, _, _, _) => false,
            AStmt::While(_, _, _) | AStmt::Loop(_) => false,
            AStmt::Function(_, _, _, _, body, _, _) => body.is_unconditional_exit(),
            AStmt::Import(_, _, _, _) => false,
        }
    }
}

#[derive(Debug)]
pub enum AExpr {
    // The value itself (resolved) and its type
    Constant(Location, Box<dyn SyncProgramValue>, ValueType),
    // Slot info, name, value stmt
    AssignVariable(Location, VariableSlot, String, Box<AExpr>),
    // Slot info, name, type
    RetrieveVariable(Location, VariableSlot, String, ValueType),
    Logical(Box<AExpr>, LogicalOp, Box<AExpr>),
    Block(Location, Vec<AStmt>, Scope),
    // Function name, call location, parameters, call fn, return type
    Call(
        Location,
        String,
        CallSource,
        Vec<Box<AExpr>>,
        FunctionKind,
        Option<ValueType>,
    ),
    // Call user defined script function by index
    ScriptCall(Location, usize, Vec<Box<AExpr>>, Option<ValueType>),
    // Reference to a script function used as a value
    FunctionReference(Location, usize, Vec<ValueType>, Option<ValueType>),
    // Call through a Function-typed variable
    DynamicCall(Location, Box<AExpr>, Vec<Box<AExpr>>, Option<ValueType>),
    // Import index, import function index, parameters, return type, arg exprs
    ImportedFunctionCall(
        Location,
        usize,
        usize,
        Vec<ValueType>,
        Option<ValueType>,
        Vec<Box<AExpr>>,
    ),
    // Condition, body, otherwise
    If(Box<AExpr>, Box<AExpr>, Option<Box<AExpr>>),
}

impl AExpr {
    /// What variables has this interacted with?
    fn retrieved_variables(&self) -> Vec<(VariableSlot, MaybeValueType)> {
        match self {
            AExpr::Constant(_, _, _) => vec![],
            AExpr::AssignVariable(_, _, _, aexpr) => aexpr.retrieved_variables(),
            AExpr::RetrieveVariable(_, variable_slot, _, value_type) => {
                vec![(*variable_slot, MaybeValueType::Set(value_type.clone()))]
            }
            AExpr::Logical(aexpr, _, aexpr1) => {
                let mut vars = aexpr.retrieved_variables();
                vars.append(&mut aexpr1.retrieved_variables());
                vars
            }
            AExpr::Block(_, _, scope) => {
                let mut values = vec![];
                for var in &scope.variable_state {
                    let slot = VariableSlot::new(*var.0, scope.is_global);
                    values.push((slot, var.1.clone()));
                }

                values
            }
            AExpr::Call(_, _, _, params, _, _) => {
                let mut vars = vec![];
                for aexpr in params {
                    vars.append(&mut aexpr.retrieved_variables());
                }

                vars
            }
            AExpr::ScriptCall(_, _, params, _) => {
                let mut vars = vec![];
                for aexpr in params {
                    vars.append(&mut aexpr.retrieved_variables());
                }
                vars
            }
            AExpr::FunctionReference(_, _, _, _) => vec![],
            AExpr::DynamicCall(_, callee, params, _) => {
                let mut vars = callee.retrieved_variables();
                for aexpr in params {
                    vars.append(&mut aexpr.retrieved_variables());
                }
                vars
            }
            AExpr::ImportedFunctionCall(_, _, _, _, _, params) => {
                let mut vars = vec![];
                for aexpr in params {
                    vars.append(&mut aexpr.retrieved_variables());
                }
                vars
            }
            AExpr::If(condition, body, otherwise) => {
                let mut retrieved = condition.retrieved_variables();
                retrieved.append(&mut body.retrieved_variables());

                if let Some(otherwise) = otherwise {
                    retrieved.append(&mut otherwise.retrieved_variables());
                }

                retrieved
            }
        }
    }

    fn modified_variables(&self) -> Vec<(VariableSlot, MaybeValueType)> {
        match self {
            AExpr::Constant(_, _, _) => vec![],
            AExpr::Logical(a, _, b) => {
                let mut vars = vec![];
                for var in a.modified_variables() {
                    vars.push(var);
                }
                for var in b.modified_variables() {
                    vars.push(var);
                }

                vars
            }
            AExpr::Block(_, astmts, _) => {
                let mut vars = vec![];
                for astmt in astmts {
                    for var in astmt.modified_variables() {
                        vars.push(var);
                    }
                }

                vars
            }
            AExpr::Call(_, _, _, aexprs, _, _) => {
                let mut vars = vec![];
                for aexpr in aexprs {
                    for var in aexpr.modified_variables() {
                        vars.push(var);
                    }
                }

                vars
            }
            AExpr::RetrieveVariable(_, _, _, _) => vec![],
            AExpr::ScriptCall(_, _, aexprs, _) => {
                let mut vars = vec![];
                for aexpr in aexprs {
                    for var in aexpr.modified_variables() {
                        vars.push(var);
                    }
                }

                vars
            }
            AExpr::FunctionReference(_, _, _, _) => vec![],
            AExpr::DynamicCall(_, callee, args, _) => {
                let mut vars = vec![];
                for var in callee.modified_variables() {
                    vars.push(var);
                }

                for aexpr in args {
                    for var in aexpr.modified_variables() {
                        vars.push(var);
                    }
                }

                vars
            }
            AExpr::ImportedFunctionCall(_, _, _, _, _, aexprs) => {
                let mut vars = vec![];
                for aexpr in aexprs {
                    for var in aexpr.modified_variables() {
                        vars.push(var);
                    }
                }

                vars
            }
            AExpr::AssignVariable(_, slot, _, value) => {
                let value_type = value
                    .get_value_type()
                    .expect("Should have assignment value.");
                let mut vars = vec![(slot.clone(), value_type)];
                for var in value.modified_variables() {
                    vars.push(var);
                }

                vars
            }
            AExpr::If(condition, body, otherwise) => {
                let mut modified = condition.modified_variables();

                let mut body = body.modified_variables();
                let otherwise = if let Some(otherwise) = otherwise {
                    otherwise.modified_variables()
                } else {
                    vec![]
                };

                // The body of the if may or may not execute, *unless* otherwise contains it as well
                body.iter_mut().for_each(|v| match &v.1 {
                    MaybeValueType::Set(value_type) => {
                        if let Some(entry) = otherwise.iter().find(|entry| entry.0 == v.0) {
                            // The else statement contains it, how do we act?
                            if matches!(entry.1, MaybeValueType::Set(_)) {
                                // YAY! We get to keep it :)
                                return;
                            }

                            // Sad
                        }

                        // Otherwise doesn't contain, it's a maybe at best.
                        v.1 = MaybeValueType::Maybe(value_type.clone());
                    }
                    MaybeValueType::Maybe(_) => {
                        // Can't make a maybe a set without a set :P
                    }
                });
                modified.append(&mut body);

                // For the otherwise, we only apply what we wouldn't have already done
                // For that reason, we manually move over what we want.
                otherwise.into_iter().for_each(|v| {
                    if body.iter().any(|entry| entry.0 == v.0) {
                        // We've already handled this in the body, ignore.
                        return;
                    }

                    // The body doesn't handle this, so this action is *maybe* done.
                    modified.push((v.0, v.1.to_maybe()));
                });

                modified
            }
        }
    }

    /// Recursively search for any return statements and get their return types
    pub fn get_returns(&self) -> Vec<(Location, Option<ValueType>)> {
        match self {
            AExpr::Constant(_, _, _) => vec![],
            AExpr::AssignVariable(_, _, _, aexpr) => aexpr.get_returns(),
            AExpr::RetrieveVariable(_, _, _, _) => vec![],
            AExpr::Logical(aexpr, _, aexpr1) => {
                let mut returns = aexpr.get_returns();
                returns.append(&mut aexpr1.get_returns());

                returns
            }
            AExpr::Block(_, astmts, _) => {
                let mut returns = vec![];
                for astmt in astmts {
                    returns.append(&mut astmt.get_returns());
                }

                returns
            }
            AExpr::Call(_, _, _, aexprs, _, _) => {
                let mut returns = vec![];
                for aexpr in aexprs {
                    returns.append(&mut aexpr.get_returns());
                }

                returns
            }
            AExpr::ScriptCall(_, _, aexprs, _) => {
                let mut returns = vec![];
                for aexpr in aexprs {
                    returns.append(&mut aexpr.get_returns());
                }

                returns
            }
            AExpr::FunctionReference(_, _, _, _) => vec![],
            AExpr::DynamicCall(_, aexpr, aexprs, _) => {
                let mut returns = aexpr.get_returns();
                for aexpr in aexprs {
                    returns.append(&mut aexpr.get_returns());
                }
                returns
            }
            AExpr::ImportedFunctionCall(_, _, _, _, _, aexprs) => {
                let mut returns = vec![];
                for aexpr in aexprs {
                    returns.append(&mut aexpr.get_returns());
                }

                returns
            }
            AExpr::If(cond, body, otherwise) => {
                let mut returns = cond.get_returns();
                returns.append(&mut body.get_returns());
                if let Some(otherwise) = otherwise {
                    returns.append(&mut otherwise.get_returns());
                }

                returns
            }
        }
    }

    /// Where in the source is this located?
    pub fn get_location(&self) -> Location {
        match self {
            AExpr::Constant(location, _, _) => *location,
            AExpr::AssignVariable(location, _, _, _) => *location,
            AExpr::RetrieveVariable(location, _, _, _) => *location,
            AExpr::Logical(aexpr, _, _) => aexpr.get_location(),
            AExpr::Block(location, _, _) => *location,
            AExpr::Call(location, _, _, _, _, _) => *location,
            AExpr::ScriptCall(location, _, _, _) => *location,
            AExpr::FunctionReference(location, _, _, _) => *location,
            AExpr::DynamicCall(location, _, _, _) => *location,
            AExpr::ImportedFunctionCall(location, _, _, _, _, _) => *location,
            AExpr::If(aexpr, _, _) => aexpr.get_location(),
        }
    }

    /// What value does this produce?
    pub fn get_value_type(&self) -> Option<MaybeValueType> {
        match self {
            AExpr::Constant(_, _, value_type) | AExpr::RetrieveVariable(_, _, _, value_type) => {
                Some(MaybeValueType::Set(value_type.clone()))
            }
            // TODO: Maybe this should return the value of the assigned variable
            AExpr::AssignVariable(_, _, _, _aexpr) => None,
            AExpr::Logical(_, _, _) => Some(MaybeValueType::Set(ValueType::Bool)),
            AExpr::Block(_, astmts, _) => astmts.last().map(|v| v.get_value_type()).flatten(),
            AExpr::Call(_, _, _, _, _, return_type) => {
                return_type.clone().map(|v| MaybeValueType::Set(v))
            }
            AExpr::ScriptCall(_, _, _, return_type) => {
                return_type.clone().map(|v| MaybeValueType::Set(v))
            }
            AExpr::FunctionReference(_, name, params, rt) => Some(MaybeValueType::Set(
                ValueType::Function(name.to_string(), params.clone(), rt.clone().map(Box::new)),
            )),
            AExpr::DynamicCall(_, _, _, return_type) => {
                return_type.clone().map(|v| MaybeValueType::Set(v))
            }
            AExpr::ImportedFunctionCall(_, _, _, _, return_type, _) => {
                return_type.clone().map(|v| MaybeValueType::Set(v))
            }
            AExpr::If(_condition, body, otherwise) => {
                let body_type = body.get_value_type();
                match otherwise {
                    Some(other) => {
                        let other_type = other.get_value_type();
                        // If one branch unconditionally exits use the non exiting branch.
                        if body.is_unconditional_exit() {
                            other_type
                        } else if other.is_unconditional_exit() {
                            body_type
                        } else {
                            body_type
                        }
                    }
                    None => body_type,
                }
            }
        }
    }

    /// Return true if all code paths in this expression lead to an unconditional return, break, or continue).
    /// Used for if else type merging where one branch may unconditionally transfer control flow.
    pub fn is_unconditional_exit(&self) -> bool {
        match self {
            AExpr::Block(_, stmts, _) => {
                // Check if the last statement unconditionally exits
                stmts.last().map_or(false, |s| s.is_unconditional_exit())
            }
            AExpr::If(_, body, otherwise) => {
                // If else is an exit only if ALL paths lead to exit
                // Both branches must exit (if there is no else branch, it implicitly returns none).
                body.is_unconditional_exit()
                    && match otherwise {
                        Some(o) => o.is_unconditional_exit(),
                        None => false, // No else means not all paths exit
                    }
            }
            _ => false,
        }
    }

    /// Return true if ANY code path in this expression leads to an unconditional return, break, or continue.
    /// Used for if else type merging where one branch may have a partial exit path (early return).
    pub fn has_exit_path(&self) -> bool {
        match self {
            AExpr::Block(_, stmts, _) => {
                // Check if any statement unconditionally exits
                stmts.iter().any(|s| s.is_unconditional_exit())
            }
            AExpr::If(_, body, otherwise) => {
                // Any path exits if the body exits OR the otherwise branch exits
                body.has_exit_path()
                    || match otherwise {
                        Some(o) => o.has_exit_path(),
                        None => false,
                    }
            }
            _ => false,
        }
    }
}

#[derive(Debug)]
pub struct ConstructError {
    pub location: Location,
    kind: ConstructErrorType,
}

impl ConstructError {
    pub fn new(location: Location, kind: ConstructErrorType) -> ConstructError {
        ConstructError { location, kind }
    }

    pub fn message(&self) -> String {
        match &self.kind {
            ConstructErrorType::NoUnaryOperator(op, ty) => {
                format!("No unary operator `{op:?}` for type `{ty:?}`.")
            }
            ConstructErrorType::NoBinaryOperator(l, op, r) => {
                format!("No binary operator `{op:?}` between types `{l:?}` and `{r:?}`.")
            }
            ConstructErrorType::NoAssignmentOperator(ty, op) => {
                format!("Cannot assign with operator `{op:?}` to type `{ty:?}`.")
            }
            ConstructErrorType::ConstantMustBeInitialized => {
                "Constants must be initialized.".to_string()
            }
            ConstructErrorType::InitializerDoesntProduce => {
                "Initializer expression does not produce a value.".to_string()
            }
            ConstructErrorType::ExpressionDoesntProduce => {
                "Expression does not produce a value.".to_string()
            }
            ConstructErrorType::AssignerDoesntProduce => {
                "Assigner expression does not produce the expected type.".to_string()
            }
            ConstructErrorType::VariableUndefined(name) => {
                format!("Variable `{name}` is not defined.")
            }
            ConstructErrorType::VariableUninitialized => {
                "Variable may be uninitialized.".to_string()
            }
            ConstructErrorType::VariablePotentiallyUninitialized(typed) => {
                format!(
                    "Variable might be uninitialized on this path. Typed: {}",
                    typed.value_type().to_string()
                )
            }
            ConstructErrorType::VariableImmutable => {
                "Cannot assign to an immutable variable.".to_string()
            }
            ConstructErrorType::VariableTypeAltered(expected, actual) => {
                format!("Variable type altered. Expected type `{expected:?}` but got `{actual:?}`.")
            }
            ConstructErrorType::VariableTypeMaybeAltered(expected, actual) => {
                format!(
                    "Variable type maybe altered. Expected type `{expected:?}` but got `{actual:?}`."
                )
            }
            ConstructErrorType::InvalidAssignTarget => {
                "Left hand side must be a valid assignment target.".to_string()
            }
            ConstructErrorType::ArrayTypeUnclear => {
                format!("Can't determine array type. Hint: Type your array variable.")
            }
            ConstructErrorType::ArrayTypeMismatch(expected, found) => {
                format!("Array type mismatch. Expected type `{expected:?}` but found `{found:?}`.")
            }
            ConstructErrorType::TypeHintMismatch(expected, found) => {
                format!("Type hint mismatch. Expected type `{expected:?}` but found `{found:?}`.")
            }
            ConstructErrorType::ExpectedBoolean(found) => {
                format!("Expected a boolean expression but found `{found:?}`.")
            }
            ConstructErrorType::NoValueProduced => {
                "Statement does not produce a value.".to_string()
            }
            ConstructErrorType::UnexpectedValue => "Unexpected value produced.".to_string(),
            ConstructErrorType::ExpectedInitializedRange(maybe_ty) => {
                format!("Expected an initialized range. Got `{maybe_ty:?}`.")
            }
            ConstructErrorType::CantStringify(ty) => {
                format!("Cannot stringify type `{ty:?}`.").into()
            }
            ConstructErrorType::UnknownType(name) => format!("Unknown type: `{name}`.").to_string(),
            ConstructErrorType::UnexpectedReturnValue(expected, actual) => {
                let exp = expected
                    .as_ref()
                    .map(|t| t.to_string())
                    .unwrap_or_else(|| "none".to_string());
                let act = actual
                    .as_ref()
                    .map(|t| t.to_string())
                    .unwrap_or_else(|| "none".to_string());
                format!("Expected return value `{exp}` but got `{act}`.")
            }
            ConstructErrorType::BreakOutsideOfLoop => format!("Break outside of loop."),
            ConstructErrorType::ContinueOutsideOfLoop => format!("Continue outside of loop."),
            ConstructErrorType::IfValueWithoutElse => {
                "If-else expression must have both branches.".to_string()
            }
            ConstructErrorType::IfElseTypeMismatched(l, r) => {
                let left = l
                    .as_ref()
                    .map(|t| t.to_string())
                    .unwrap_or_else(|| "none".to_string());
                let right = r
                    .as_ref()
                    .map(|t| t.to_string())
                    .unwrap_or_else(|| "none".to_string());
                format!("If-else branches have mismatched types: `{left}` vs `{right}`.")
            }
            ConstructErrorType::ArrayTypeMismatch(expected, found) => {
                format!("Array entries have mismatched types: `{expected}` vs `{found}`.")
            }
            ConstructErrorType::NoMethod(ty, name, params) => {
                let exp_types: Vec<String> = params.iter().map(|t| t.to_string()).collect();
                let params = exp_types.join(", ");
                format!("Type `{ty}` has no method named `{name}` with parameters `{params}`.")
            }
            ConstructErrorType::InvalidCall(name, expected, actual) => {
                let exp_types: Vec<String> = expected.iter().map(|t| t.to_string()).collect();
                let act_types: Vec<String> = actual
                    .iter()
                    .map(|m| {
                        m.as_ref()
                            .map(|v| v.to_string())
                            .unwrap_or_else(|| "maybe".to_string())
                    })
                    .collect();
                format!(
                    "Invalid call to `{name}`. Expected params ({}) but got ({}).",
                    exp_types.join(", "),
                    act_types.join(", ")
                )
            }
            ConstructErrorType::PropertyCalledAsMethod(name) => {
                format!("`{name}` is a property, not a method.")
            }
            ConstructErrorType::MethodCalledAsProperty(name) => {
                format!("`{name}` is a method, not a property.")
            }
            ConstructErrorType::ImportPathCantBeResolved(path) => {
                format!("Can't resolve location of import file path '{path:?}'")
            }
            ConstructErrorType::ImportFileNotFound(path) => {
                format!("Import file not found: `{path}`.").to_string()
            }
            ConstructErrorType::ImportSyntaxError(path, source, errors, tokens) => {
                let count = errors.len();
                format!(
                    "Import has {count} syntax error(s):\n{}",
                    errors[0..errors.len().min(3)]
                        .iter()
                        .enumerate()
                        .map(|(index, v)| format!(
                            "Parse error '{index}':\n{}",
                            visualize::format_diagnostic(source, Some(&path.lexeme), v, tokens)
                        ))
                        .collect::<Vec<_>>()
                        .join("\n")
                )
                .to_string()
            }
            ConstructErrorType::ImportParseError(path, source, errors, tokens) => {
                let count = errors.len();
                format!(
                    "Import has {count} parse error(s):\n{}",
                    errors[0..errors.len().min(3)]
                        .iter()
                        .enumerate()
                        .map(|(index, v)| format!(
                            "Parse error '{index}':\n{}",
                            visualize::format_diagnostic(source, Some(&path.lexeme), v, tokens)
                        ))
                        .collect::<Vec<_>>()
                        .join("\n")
                )
                .to_string()
            }
            ConstructErrorType::ImportConstructError(path, source, err, tokens) => format!(
                "Import has construct error: {}\n",
                visualize::format_diagnostic(source, Some(&path), err.as_ref(), tokens)
            )
            .to_string(),
            ConstructErrorType::ImportCircular(_) => "Circular import detected.".to_string(),
            ConstructErrorType::ImportPathInvalid(_) => "Import path is invalid.".to_string(),
            ConstructErrorType::ImportShadowed(name) => {
                format!("Import `{name}` shadows an existing binding.").to_string()
            }
            ConstructErrorType::NamespaceNotImported(ns) => {
                format!("Namespace `{ns}` has not been imported.")
            }
            ConstructErrorType::ImportedFunctionNotFound(name) => {
                format!("Imported function `{name}` not found.")
            }
            ConstructErrorType::UnexpectedCall(_) => "Unexpected call expression.".to_string(),
            ConstructErrorType::UnknownCall(name, vt, args) => {
                format!(
                    "Unknown call attempted on '{name}' for '{vt}' with args: {}",
                    args.iter()
                        .map(|v| v.to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
            ConstructErrorType::IncorrectCallSignature(name, args, func_info) => {
                format!(
                    "Incorrect call signature for '{name}', found args: '{}'. Possibly need this signature '{}'?",
                    args.iter()
                        .map(|v| v.to_string())
                        .collect::<Vec<_>>()
                        .join(", "),
                    func_info
                        .param_types
                        .iter()
                        .map(|v| v.to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
            ConstructErrorType::QualifiedAccessNotCallOrGlobal(name) => {
                "Qualified access is not imported callable or global: '".to_string()
                    + name.as_str()
                    + "'"
            }
            ConstructErrorType::FunctionReturnTypeAndBodyTypeMismatch(expected, actual) => {
                let exp = expected
                    .as_ref()
                    .map(|t| t.to_string())
                    .unwrap_or_else(|| "none".to_string());
                let act = actual
                    .as_ref()
                    .map(|t| t.to_string())
                    .unwrap_or_else(|| "none".to_string());
                format!("Function return type `{exp}` does not match body type `{act}`.")
            }
            ConstructErrorType::LoopNeedsExitCondition => {
                format!("Loop needs to have an exit condition (break, return).")
            }
        }
    }

    pub fn kind(&self) -> &ConstructErrorType {
        &self.kind
    }
}

#[derive(Debug)]
pub enum ConstructErrorType {
    NoUnaryOperator(UnaryOp, ValueType),
    NoBinaryOperator(ValueType, BinaryOp, ValueType),
    NoAssignmentOperator(ValueType, AssignmentOp),
    ConstantMustBeInitialized,
    InitializerDoesntProduce,
    ExpressionDoesntProduce,
    AssignerDoesntProduce,
    VariableUndefined(String),
    VariableUninitialized,
    VariablePotentiallyUninitialized(MaybeValueType),
    VariableImmutable,
    // Expected type, modified type
    VariableTypeAltered(MaybeValueType, MaybeValueType),
    VariableTypeMaybeAltered(ValueType, ValueType),
    InvalidAssignTarget,
    ExpectedBoolean(Option<ValueType>),
    NoValueProduced,
    UnexpectedValue,
    ExpectedInitializedRange(Option<MaybeValueType>),
    CantStringify(ValueType),
    UnknownType(String),
    // Expected, provided
    UnexpectedReturnValue(Option<MaybeValueType>, Option<MaybeValueType>),
    BreakOutsideOfLoop,
    ContinueOutsideOfLoop,
    IfValueWithoutElse,
    IfElseTypeMismatched(Option<MaybeValueType>, Option<MaybeValueType>),
    ArrayTypeMismatch(ValueType, ValueType),
    TypeHintMismatch(Option<ValueType>, Option<MaybeValueType>),
    // Receiver type and member name were not found in the method registry
    NoMethod(ValueType, String, Vec<ValueType>),
    // Name, expected, recieved
    InvalidCall(String, Vec<ValueType>, Vec<Option<MaybeValueType>>),
    PropertyCalledAsMethod(String),
    MethodCalledAsProperty(String),
    ImportPathCantBeResolved(String),
    ImportFileNotFound(String),
    // Path token, source code, errors, tokens
    ImportSyntaxError(Token, String, Vec<SyntaxError>, Vec<Token>),
    // Path token, source code, errors, tokens
    ImportParseError(Token, String, Vec<ParseError>, Vec<Token>),
    // Path, source code, error, tokens
    ImportConstructError(String, String, Box<ConstructError>, Vec<Token>),
    ImportCircular(Token),
    ImportPathInvalid(Token),
    ImportShadowed(String),
    NamespaceNotImported(String),
    ImportedFunctionNotFound(String),
    UnexpectedCall(Box<Expr>),
    // Variable name, type, args
    UnknownCall(String, ValueType, Vec<ValueType>),
    // Name, provided types, possible signature
    IncorrectCallSignature(String, Vec<ValueType>, FunctionInfo),
    // Name
    QualifiedAccessNotCallOrGlobal(String),
    FunctionReturnTypeAndBodyTypeMismatch(Option<ValueType>, Option<ValueType>),
    ArrayTypeUnclear,
    LoopNeedsExitCondition,
}

#[derive(Clone, Debug)]
pub struct ScriptFunctionSignature {
    pub name: String,
    pub param_types: Vec<ValueType>,
    pub return_type: Option<ValueType>,
}

#[derive(Debug)]
pub struct FinalizedConstruct {
    pub stmts: Vec<AStmt>,
    pub imports: Vec<(String, Program)>,

    // Cycle detection return
    visiting_paths: HashSet<String>,
}

pub struct Constructor {
    // Data
    module: Arc<Module>,

    // Building info
    call_frames: Vec<Frame>,
    stmts: Vec<AStmt>,
    functions: Vec<ScriptFunctionSignature>,
    is_import_mode: bool,
    source_path: Option<String>,

    // External dependencies
    import_resolver: Arc<dyn ImportResolver>,
    imports: Vec<(String, Program)>,

    // Cycle detection
    visiting_paths: HashSet<String>,
}

impl Constructor {
    pub fn new(
        module: Arc<Module>,
        import_resolver: Arc<dyn ImportResolver>,
        source_path: Option<String>,
    ) -> Constructor {
        Self {
            module,

            call_frames: vec![Frame::new(true)],
            stmts: vec![],
            functions: Default::default(),
            is_import_mode: true,
            source_path,

            import_resolver,
            imports: vec![],

            visiting_paths: HashSet::new(),
        }
    }

    pub fn generate(mut self, stmts: &[Stmt]) -> Result<FinalizedConstruct, ConstructError> {
        // First pass to register all top level function signatures
        for stmt in stmts {
            // TODO: Does this handle nested function definitions?
            // Maybe it's fine, it could be cool tho
            if let Stmt::Function(name, params, _, specified_return) = stmt {
                let return_type = if let Some(tok) = specified_return {
                    let Some(rt) = self.module.resolve_type(&tok.lexeme) else {
                        return Err(ConstructError::new(
                            tok.location,
                            ConstructErrorType::UnknownType(tok.lexeme.clone()),
                        ));
                    };
                    Some(rt)
                } else {
                    None // Inferred later when the body is constructed
                };
                self.register_script_signature(name, params, return_type)?;
            }
        }

        // Reset call frames (ensure frame is marked "global" like the initializer above)
        self.call_frames.clear();
        self.call_frames = vec![Frame::new(true)];

        // Push all registered function signatures to the global scope so forward references resolve
        for (index, signature) in self.functions.clone().into_iter().enumerate() {
            self.push_script_function(&signature.name, index);
        }

        // Actually generate it knowing all function signatures
        for stmt in stmts {
            let astmt = self.statement(stmt, false, None)?;
            self.stmts.push(astmt);
        }

        Ok(FinalizedConstruct {
            stmts: self.stmts,

            visiting_paths: self.visiting_paths,
            imports: self.imports,
        })
    }

    fn register_script_signature(
        &mut self,
        name: &Token,
        params: &[FunctionParameter],
        specified_return: Option<ValueType>,
    ) -> Result<(), ConstructError> {
        let mut param_types = vec![];
        for param in params {
            let Some(pt) = self.module.resolve_type(&param.typing.lexeme) else {
                return Err(ConstructError::new(
                    param.typing.location,
                    ConstructErrorType::UnknownType(param.typing.lexeme.clone()),
                ));
            };
            param_types.push(pt);
        }

        self.functions.push(ScriptFunctionSignature {
            name: name.lexeme.to_string(),
            param_types,
            return_type: specified_return,
        });
        Ok(())
    }

    fn statement(
        &mut self,
        stmt: &Stmt,
        is_maybe_mode: bool,
        type_hint: Option<&ValueType>,
    ) -> Result<AStmt, ConstructError> {
        //println!("Stmt: {stmt:?}");

        // Only allow imports at the top of the file
        if matches!(stmt, Stmt::Import(_, _)) {
            if !self.is_import_mode {}
        } else {
            self.is_import_mode = false;
        }

        let stmt = match stmt {
            Stmt::DefineVariable(token, initializer, mutability) => {
                let initializer = if let Some(initializer) = initializer {
                    Some(Box::new(self.expression(initializer, false, type_hint)?))
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
            Stmt::Expression(expr, produces_value) => {
                let expr = self.expression(expr, is_maybe_mode, type_hint)?;
                let expr_value_type = expr.get_value_type();

                if !*produces_value && expr_value_type.is_some() {
                    // There's something extra on the stack
                    self.pop_stack();
                }

                AStmt::Expression(
                    expr.get_location(),
                    expr,
                    // We downgrade it if needed
                    *produces_value && expr_value_type.is_some(),
                )
            }
            Stmt::While(condition, body) => {
                // Handle loop breaks and continues
                self.increment_loop_depth();

                let condition = self.expression(condition, false, None)?;
                if condition.get_value_type() != Some(MaybeValueType::Set(ValueType::Bool)) {
                    self.decrement_loop_depth();
                    return Err(ConstructError::new(
                        condition.get_location(),
                        ConstructErrorType::ExpectedBoolean(
                            condition.get_value_type().map(|v| v.value_type()),
                        ),
                    ));
                }

                let condition_retrieved = condition.retrieved_variables();

                let body = self.expression(body, true, None)?;
                let body_modified = body.modified_variables();

                // Ensure the body doesn't attempt to type alter the condition variables
                for modified in body_modified {
                    if let Some(original) = condition_retrieved.iter().find(|v| v.0 == modified.0) {
                        // Make sure we've not fucked it up
                        if original.1.value_type() != modified.1.value_type() {
                            self.decrement_loop_depth();
                            return Err(ConstructError::new(
                                body.get_location(),
                                ConstructErrorType::VariableTypeAltered(
                                    original.1.clone(),
                                    modified.1,
                                ),
                            ));
                        }
                    }
                }

                // Make sure the body returns nothing, why would a while loop do that?
                if body.get_value_type().is_some() {
                    self.decrement_loop_depth();
                    return Err(ConstructError::new(
                        body.get_location(),
                        ConstructErrorType::UnexpectedValue,
                    ));
                }

                // Variable scope typing is handled by blocks
                let result = AStmt::While(
                    condition.get_location(),
                    Box::new(condition),
                    Box::new(body),
                );
                self.decrement_loop_depth();
                result
            }
            Stmt::Loop(body) => {
                // Handle loop breaks and continues
                self.increment_loop_depth();

                let body = self.expression(body, false, None)?;
                // TODO: Is this check correct?
                if !body.has_exit_path() {
                    self.decrement_loop_depth();
                    return Err(ConstructError::new(
                        body.get_location(),
                        ConstructErrorType::LoopNeedsExitCondition,
                    ));
                }

                // Make sure the body returns nothing, why would a loop do that?
                if body.get_value_type().is_some() {
                    self.decrement_loop_depth();
                    return Err(ConstructError::new(
                        body.get_location(),
                        ConstructErrorType::UnexpectedValue,
                    ));
                }

                // Variable scope typing is handled by blocks
                let result = AStmt::Loop(Box::new(body));

                // TODO: Test to make sure loop isn't infinite, it must have a break or return
                self.decrement_loop_depth();
                result
            }
            Stmt::Function(name, params, body, specified_return) => {
                // Resolve parameter types once and store them for both registration and frame setup.
                let mut param_types = Vec::with_capacity(params.len());
                for param in params {
                    let Some(param_type) = self.module.resolve_type(&param.typing.lexeme) else {
                        return Err(ConstructError::new(
                            param.typing.location,
                            ConstructErrorType::UnknownType(param.typing.lexeme.clone()),
                        ));
                    };

                    param_types.push(param_type);
                }

                let return_type = if let Some(tok) = specified_return {
                    let Some(return_type) = self.module.resolve_type(&tok.lexeme) else {
                        return Err(ConstructError::new(
                            tok.location,
                            ConstructErrorType::UnknownType(tok.lexeme.clone()),
                        ));
                    };
                    Some(return_type)
                } else {
                    None // Inferred later when the body is constructed
                };

                // Register the function BEFORE pushing a new frame so recursive calls can resolve.
                let function_index = if let Some(index) = self.resolve_script_function(&name.lexeme)
                {
                    index
                } else {
                    let function_index = self.functions.len();
                    self.functions.push(ScriptFunctionSignature {
                        name: name.lexeme.to_string(),
                        param_types: param_types.clone(),
                        return_type: return_type.clone(),
                    });
                    self.push_script_function(&name.lexeme, function_index);

                    function_index
                };

                // Enter new framing for the function body.
                self.push_frame();

                // Push all parameters into the new frame.
                let mut param_slots = Vec::with_capacity(params.len());
                for (i, param) in params.iter().enumerate() {
                    let slot = self.push_variable(
                        param.name.location,
                        &param.name.lexeme,
                        param.mutability,
                        Some(param_types[i].clone()),
                    )?;

                    param_slots.push((slot, param.name.lexeme.clone(), param_types[i].clone()));
                }

                // Parse the body (recursive calls to this function can now resolve)
                let body = Box::new(self.expression(body, true, None)?);

                // Check the return type against the body
                if return_type.clone().map(|v| MaybeValueType::Set(v)) != body.get_value_type() {
                    return Err(ConstructError::new(
                        name.location,
                        ConstructErrorType::FunctionReturnTypeAndBodyTypeMismatch(
                            return_type,
                            body.get_value_type().map(|v| v.value_type()),
                        ),
                    ));
                }

                // Ensure returns align with expected typing
                for entry in body.get_returns() {
                    if entry.1 != return_type {
                        return Err(ConstructError::new(
                            entry.0,
                            ConstructErrorType::UnexpectedReturnValue(
                                return_type.map(|v| MaybeValueType::Set(v)),
                                entry.1.map(|v| MaybeValueType::Set(v)),
                            ),
                        ));
                    }
                }

                // Capture local count before popping the frame
                let local_count = self.call_frames.last().unwrap().variable_slots.len();

                // End our framing
                self.pop_frame();

                AStmt::Function(
                    name.location,
                    name.lexeme.to_string(),
                    function_index,
                    param_slots,
                    body,
                    return_type,
                    local_count,
                )
            }
            Stmt::Return(location, stmt) => {
                let a_stmt = if let Some(s) = stmt {
                    Some(Box::new(self.expression(s, false, None)?))
                } else {
                    None
                };
                AStmt::Return(*location, a_stmt)
            }
            Stmt::Break(location) => {
                let depth = self.current_loop_depth();
                if depth == 0 {
                    return Err(ConstructError::new(
                        *location,
                        ConstructErrorType::BreakOutsideOfLoop,
                    ));
                }
                AStmt::Break(*location)
            }
            Stmt::Continue(location) => {
                let depth = self.current_loop_depth();
                if depth == 0 {
                    return Err(ConstructError::new(
                        *location,
                        ConstructErrorType::ContinueOutsideOfLoop,
                    ));
                }
                AStmt::Continue(*location)
            }
            Stmt::Import(path, namespace) => {
                // Reject relative paths starting with ./ or ../, as well as improper slashes
                // We're looking for my_folder/my_script or /MyMod::MyName/my_folder/my_script
                if path.lexeme.starts_with("./")
                    || path.lexeme.starts_with("../")
                    || path.lexeme.contains("//")
                {
                    return Err(ConstructError::new(
                        path.location,
                        ConstructErrorType::ImportPathInvalid(path.clone()),
                    ));
                }

                // Make sure this import path isn't already being checked
                if self.visiting_paths.contains(&path.lexeme) {
                    return Err(ConstructError::new(
                        path.location,
                        ConstructErrorType::ImportCircular(path.clone()),
                    ));
                }

                let resolved_path = match self
                    .import_resolver
                    .resolve_path(self.source_path.as_ref().map(|v| v.as_str()), &path.lexeme)
                {
                    Ok(val) => val,
                    Err(err) => {
                        return Err(ConstructError::new(
                            path.location,
                            ConstructErrorType::ImportPathCantBeResolved(err),
                        ));
                    }
                };

                let Ok(script) = self.import_resolver.get_script(&resolved_path) else {
                    return Err(ConstructError::new(
                        path.location,
                        ConstructErrorType::ImportFileNotFound(resolved_path),
                    ));
                };

                // Mark as visiting for cycle detection
                self.visiting_paths.insert(path.lexeme.clone());

                // Tokenize
                let mut scanner = Scanner::new(script.to_owned());
                scanner.scan();

                if !scanner.errors().is_empty() {
                    return Err(ConstructError::new(
                        path.location,
                        ConstructErrorType::ImportSyntaxError(
                            path.clone(),
                            script,
                            scanner.errors().to_vec(),
                            scanner.get().unwrap_or(&[]).to_vec(),
                        ),
                    ));
                }

                let tokens = scanner.get().expect("Should have tokens.");

                // Parse
                let mut parser = Parser::new(tokens);
                parser.parse();

                if !parser.errors().is_empty() {
                    return Err(ConstructError::new(
                        path.location,
                        ConstructErrorType::ImportParseError(
                            path.clone(),
                            script.to_owned(),
                            parser.errors().to_vec(),
                            scanner.get().expect("Already validated above").to_vec(),
                        ),
                    ));
                }

                // Construct
                let child_constructor = Constructor {
                    module: self.module.clone(),

                    stmts: vec![],
                    functions: Default::default(),
                    call_frames: vec![Frame::new(true)],
                    is_import_mode: true,
                    source_path: Some(resolved_path.clone()),

                    // Clone inner value, not Box<Box<T>>
                    import_resolver: self.import_resolver.clone(),
                    imports: vec![],

                    // Pass this off for a moment, we'll take it back
                    visiting_paths: std::mem::take(&mut self.visiting_paths),
                };

                let mut final_construct = match child_constructor
                    .generate(parser.get().expect("Should have statements."))
                {
                    Ok(val) => val,
                    Err(err) => {
                        return Err(ConstructError::new(
                            path.location,
                            ConstructErrorType::ImportConstructError(
                                resolved_path,
                                script,
                                Box::new(err),
                                scanner.get().expect("Already should have tokens").to_vec(),
                            ),
                        ));
                    }
                };

                // Take the paths back, remove the imported path as it's been successful
                self.visiting_paths = std::mem::take(&mut final_construct.visiting_paths);
                self.visiting_paths.remove(&path.lexeme);

                // Program
                let instructor =
                    Instructor::new(self.module.clone(), self.import_resolver.clone(), None);
                let program = instructor.generate(final_construct, script.to_owned());

                let import_index = self.imports.len();
                self.imports.push((namespace.lexeme.to_string(), program));
                AStmt::Import(
                    path.location,
                    path.lexeme.to_string(),
                    namespace.lexeme.to_string(),
                    import_index,
                )
            }
        };
        Ok(stmt)
    }

    fn push_frame(&mut self) {
        self.call_frames.push(Frame::new(false));
    }

    fn pop_frame(&mut self) {
        self.call_frames.pop().expect("Should have call frame");
    }

    fn increment_loop_depth(&mut self) {
        self.current_frame_mut().loop_depth += 1;
    }

    fn decrement_loop_depth(&mut self) {
        self.current_frame_mut().loop_depth -= 1;
    }

    fn current_loop_depth(&self) -> usize {
        self.call_frames
            .last()
            .expect("Should have call frame")
            .loop_depth
    }

    fn current_frame_mut(&mut self) -> &mut Frame {
        self.call_frames.last_mut().expect("Should have call frame")
    }

    fn push_scope(&mut self) {
        //println!("Pushed scope");
        let call_frame = self.call_frames.last_mut().expect("Should have call frame");
        call_frame.scope.push(Scope::new(
            call_frame.variable_slots.len(),
            call_frame.is_global,
        ));
    }

    fn pop_scope(&mut self) -> Scope {
        //println!("Popped scope");
        let call_frame = self.call_frames.last_mut().expect("Should have call frame");
        // Get scope, we need to handle variable typing
        let scope = call_frame.scope.pop().expect("Should have scope");

        // Clear slots
        call_frame
            .variable_slots
            .truncate(scope.starting_variable_count);

        // TODO: This needs to apply the scope's variable changes
        scope
    }

    fn apply_scope_variables(
        &mut self,
        scope: &Scope,
        maybe: bool,
    ) -> Result<(), ConstructErrorType> {
        let call_frame = self.call_frames.last_mut().expect("Should have call frame");
        let calling_scope = call_frame.scope.last_mut().expect("Expected scope");

        // Apply scope but don't leak new bindings
        let source_start = scope.starting_variable_count;

        for (index, value_type) in &scope.variable_state {
            if *index >= source_start {
                continue;
            }

            if let Some(existing) = calling_scope.variable_state.get_mut(index) {
                // Already exists in the calling scope
                match existing {
                    MaybeValueType::Set(_) => {
                        if existing.value_type() == value_type.value_type() {
                            // Don't care, at all.
                        } else {
                            // We can't trust the typing here.
                            return Err(ConstructErrorType::VariableTypeMaybeAltered(
                                existing.value_type(),
                                value_type.value_type(),
                            ));
                        }
                    }
                    MaybeValueType::Maybe(_) => {
                        // Type check, otherwise don't care, it's maybe already.
                        if value_type.value_type() != existing.value_type() {
                            return Err(ConstructErrorType::VariableTypeMaybeAltered(
                                existing.value_type(),
                                value_type.value_type(),
                            ));
                        }
                    }
                }
            } else {
                let final_type = if maybe {
                    value_type.to_maybe()
                } else {
                    value_type.clone()
                };
                calling_scope.variable_state.insert(*index, final_type);
            }
        }

        Ok(())
    }

    fn alloc_variable(
        &mut self,
        source: Location,
        name: &str,
        initializer: Option<&Box<AExpr>>,
        mutability: VariableMutability,
    ) -> Result<VariableSlot, ConstructError> {
        if initializer.is_none() && mutability == VariableMutability::Constant {
            return Err(ConstructError::new(
                source,
                ConstructErrorType::ConstantMustBeInitialized,
            ));
        }

        let initialized_type = if let Some(initializer) = initializer {
            if let Some(MaybeValueType::Set(value)) = initializer.get_value_type() {
                Some(value)
            } else {
                if let Some(MaybeValueType::Maybe(value)) = initializer.get_value_type() {
                    return Err(ConstructError::new(
                        initializer.get_location(),
                        ConstructErrorType::VariablePotentiallyUninitialized(
                            MaybeValueType::Maybe(value),
                        ),
                    ));
                }

                return Err(ConstructError::new(
                    initializer.get_location(),
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
        // Don't allow variables to shadow imports
        if self.imports.iter().any(|v| v.0 == name) {
            return Err(ConstructError::new(
                source,
                ConstructErrorType::ImportShadowed(name.to_string()),
            ));
        }

        let current_frame = self.call_frames.last_mut().unwrap();

        let index = current_frame.variable_slots.len();
        // Mark typing info for this scope
        if let Some(initialized_type) = initialized_type {
            current_frame
                .scope
                .last_mut()
                .expect("Should have scope")
                .variable_state
                .insert(
                    current_frame.variable_slots.len(),
                    MaybeValueType::Set(initialized_type),
                );
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
    ) -> Result<(VariableSlot, VariableMutability, Option<MaybeValueType>), ConstructError> {
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
                        return Ok((VariableSlot::new(i, is_global), entry.2, Some(var.clone())));
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
                        return Ok((VariableSlot::new(i, true), entry.2, Some(var.clone())));
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
            ConstructErrorType::VariableUndefined(name.to_string()),
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
                .insert(slot.index, MaybeValueType::Set(value));
            //global_frame.variable_slots[slot.index].2 = Some(value);
        } else {
            let current_frame = self.call_frames.last_mut().unwrap();

            current_frame
                .scope
                .last_mut()
                .expect("Should have scope")
                .variable_state
                .insert(slot.index, MaybeValueType::Set(value));
            //current_frame.variable_slots[slot.index].2 = Some(value);
        }
    }

    /// Push a script function declaration to the stack
    fn push_script_function(&mut self, name: &str, index: usize) {
        let current_frame = self.call_frames.last_mut().unwrap();
        let current_scope = current_frame.scope.last_mut().unwrap();
        current_scope
            .available_functions
            .insert(name.to_owned(), index);
    }

    /// Find a script function somewhere in the universe
    fn resolve_script_function(&self, name: &str) -> Option<usize> {
        let current_frame = self.call_frames.last().unwrap();
        for scope in current_frame.scope.iter().rev() {
            if let Some(function) = scope.available_functions.get(name) {
                return Some(*function);
            }
        }

        if current_frame.is_global {
            return None;
        }

        // Check global frame
        let global_frame = self.call_frames.first().unwrap();
        for scope in global_frame.scope.iter().rev() {
            if let Some(function) = scope.available_functions.get(name) {
                return Some(*function);
            }
        }

        // RIP
        None
    }

    fn push_stack(&mut self, source: Location, value: ValueType) {
        //println!("Pushed @ {source:?} '{value}'");
        self.call_frames
            .last_mut()
            .unwrap()
            .floating_stack
            .push((source, value));
    }

    fn pop_stack(&mut self) -> Option<(Location, ValueType)> {
        let stack = self.call_frames.last_mut().unwrap().floating_stack.pop();

        //println!("Popped: {stack:?}");

        stack
    }

    fn expression(
        &mut self,
        expr: &Expr,
        is_maybe_mode: bool,
        type_hint: Option<&ValueType>,
    ) -> Result<AExpr, ConstructError> {
        //println!("Expr: {expr:?}");
        let a_expr = match expr {
            Expr::Float(location, val) => {
                self.push_stack(*location, ValueType::Float);
                AExpr::Constant(*location, Box::new(*val), ValueType::Float)
            }
            Expr::Integer(location, val) => {
                self.push_stack(*location, ValueType::Int);
                AExpr::Constant(*location, Box::new(*val), ValueType::Int)
            }
            Expr::Bool(location, val) => {
                self.push_stack(*location, ValueType::Bool);
                AExpr::Constant(*location, Box::new(*val), ValueType::Bool)
            }
            Expr::String(location, val) => {
                self.push_stack(*location, ValueType::String);
                AExpr::Constant(*location, Box::new(val.to_owned()), ValueType::String)
            }
            Expr::Array(location, vals) => {
                // Temp scope so we can find typing
                self.push_scope();

                // Elements
                let mut determined_type: Option<ValueType> = None;
                for val in vals {
                    let aexpr = self.expression(val, false, determined_type.as_ref())?;

                    // Ensure typing is correct
                    match (aexpr.get_value_type(), &determined_type) {
                        (Some(maybe_type), Some(determined)) => {
                            if &maybe_type.value_type() != determined {
                                return Err(ConstructError::new(
                                    *location,
                                    ConstructErrorType::ArrayTypeMismatch(
                                        determined.clone(),
                                        maybe_type.value_type(),
                                    ),
                                ));
                            }
                        }
                        (Some(maybe_type), None) => determined_type = Some(maybe_type.value_type()),
                        (None, _) => {
                            return Err(ConstructError::new(
                                *location,
                                ConstructErrorType::ExpressionDoesntProduce,
                            ));
                        }
                    }
                }

                // Go figure it out
                let Some(determined_type) = determined_type else {
                    return Err(ConstructError::new(
                        *location,
                        ConstructErrorType::ArrayTypeUnclear,
                    ));
                };

                // Drop the above let's build it for real.
                self.pop_scope();

                self.push_scope();

                // Enough space for var definition, pushing values, and outputing var
                let mut astmts = Vec::with_capacity(vals.len() + 2);

                // Make a new vec with unique naming
                let mut hasher = DefaultHasher::new();
                location.hash(&mut hasher);
                let unique_key =
                    "ARRAY_INITIALIZE_".to_string() + hasher.finish().to_string().as_str();

                astmts.push(self.statement(
                    &Stmt::DefineVariable(
                        Token {
                            location: *location,
                            token_type: TokenType::Identifier,
                            lexeme: unique_key.to_owned(),
                        },
                        Some(Box::new(Expr::Call(
                            Box::new(Expr::Variable(
                                *location,
                                // Magic automatic custom type implementation.
                                "Vec<".to_string() + determined_type.to_string().as_str() + ">",
                            )),
                            vec![],
                        ))),
                        VariableMutability::Mutable,
                    ),
                    false,
                    None,
                )?);

                // Push values
                for val in vals {
                    astmts.push(self.statement(
                        &Stmt::Expression(
                            Box::new(Expr::DotAccess(
                                Box::new(Expr::Variable(*location, unique_key.to_owned())),
                                "push".to_string(),
                                Some(vec![Box::new(val.clone())]),
                            )),
                            false,
                        ),
                        false,
                        // TODO: Why must this be None
                        None,
                    )?);
                }

                // Final variable retrieval
                astmts.push(self.statement(
                    &Stmt::Expression(Box::new(Expr::Variable(*location, unique_key)), true),
                    false,
                    None,
                )?);

                let scope = self.pop_scope();

                if let Err(err) = self.apply_scope_variables(&scope, false) {
                    return Err(ConstructError::new(*location, err));
                }

                AExpr::Block(*location, astmts, scope)
            }
            Expr::Unary(op, a) => self.expression(
                &Expr::DotAccess(a.clone(), op.to_string(), Some(vec![])),
                false,
                None,
            )?,
            Expr::Binary(a, op, b) => self.expression(
                &Expr::DotAccess(a.clone(), op.to_string(), Some(vec![b.clone()])),
                false,
                None,
            )?,
            Expr::Variable(location, name) => {
                let find_result = self.find_variable(*location, name);
                match find_result {
                    Ok((slot, _, Some(value_type))) => {
                        let value_type = match value_type {
                            MaybeValueType::Set(vt) => vt,
                            MaybeValueType::Maybe(val) => {
                                return Err(ConstructError::new(
                                    *location,
                                    ConstructErrorType::VariablePotentiallyUninitialized(
                                        MaybeValueType::Maybe(val),
                                    ),
                                ));
                            }
                        };
                        self.push_stack(*location, value_type.clone());
                        AExpr::RetrieveVariable(*location, slot, name.to_owned(), value_type)
                    }
                    Ok((_, _, None)) => {
                        return Err(ConstructError::new(
                            *location,
                            ConstructErrorType::VariableUninitialized,
                        ));
                    }
                    Err(err) => {
                        // Not a variable, check if it's a script function used as a value
                        if let Some(index) = self.resolve_script_function(name.as_str()) {
                            let signature = &self.functions[index];
                            let func_type = ValueType::Function(
                                name.to_string(),
                                signature.param_types.clone(),
                                signature.return_type.clone().map(Box::new),
                            );

                            let aexpr = AExpr::FunctionReference(
                                *location,
                                index,
                                signature.param_types.clone(),
                                signature.return_type.clone(),
                            );

                            self.push_stack(*location, func_type);

                            aexpr
                        } else if let Some((value_type, global)) = self.module.resolve_global(name)
                        {
                            let aexpr = AExpr::Constant(
                                *location,
                                global.as_ref().clone_sync_box(),
                                value_type.clone(),
                            );

                            self.push_stack(*location, value_type.clone());

                            // Convert globals to constants
                            aexpr
                        } else {
                            return Err(err);
                        }
                    }
                }
            }
            Expr::Condition(expr_a, logical_op, expr_b) => {
                let aexpr_a = self.expression(expr_a, is_maybe_mode, Some(&ValueType::Bool))?;
                let aexpr_b = self.expression(expr_b, is_maybe_mode, Some(&ValueType::Bool))?;

                // Both operands should be boolean for logical operations
                let stack_b = self.pop_stack().unwrap();
                let stack_a = self.pop_stack().unwrap();

                if stack_a.1 != ValueType::Bool {
                    return Err(ConstructError::new(
                        aexpr_a.get_location(),
                        ConstructErrorType::ExpectedBoolean(Some(stack_a.1)),
                    ));
                }

                if stack_b.1 != ValueType::Bool {
                    return Err(ConstructError::new(
                        aexpr_b.get_location(),
                        ConstructErrorType::ExpectedBoolean(Some(stack_b.1)),
                    ));
                }

                self.push_stack(aexpr_a.get_location(), ValueType::Bool);

                AExpr::Logical(Box::new(aexpr_a), *logical_op, Box::new(aexpr_b))
            }
            Expr::Call(calling, params) => {
                let location = calling.get_location();

                match calling.as_ref() {
                    Expr::Variable(_, name) => {
                        let mut a_params = Vec::with_capacity(params.len());
                        let mut a_param_types = Vec::with_capacity(params.len());
                        for param in params {
                            let a_param = Box::new(self.expression(param, is_maybe_mode, None)?);
                            let Some(MaybeValueType::Set(typed)) = a_param.get_value_type() else {
                                return Err(ConstructError::new(
                                    a_param.get_location(),
                                    if a_param.get_value_type().is_none() {
                                        ConstructErrorType::NoValueProduced
                                    } else {
                                        ConstructErrorType::VariableUninitialized
                                    },
                                ));
                            };
                            a_param_types.push(typed);
                            a_params.push(a_param);
                        }

                        match self.module.lookup_function(name.as_str(), &a_param_types) {
                            Err(Some(possibly)) => {
                                return Err(ConstructError::new(
                                    location,
                                    ConstructErrorType::IncorrectCallSignature(
                                        name.to_string(),
                                        a_param_types,
                                        possibly.clone(),
                                    ),
                                ));
                            }
                            Err(None) => {}
                            Ok(func_info) => {
                                // Native call

                                // Clone for borrowing
                                let func_name = func_info.name.to_string();
                                // TODO: ... Why is this unused? That feels wrong.
                                let func_param_types = func_info.param_types.clone();
                                let func_kind = func_info.kind.clone();
                                let func_return_type = func_info.return_type.clone();

                                for _ in 0..params.len() {
                                    self.pop_stack()
                                        .expect("Checked above, this is to clear out the args");
                                }

                                if let Some(return_type) = &func_return_type {
                                    self.push_stack(location, return_type.clone());
                                }

                                return Ok(AExpr::Call(
                                    location,
                                    func_name,
                                    CallSource::Stack,
                                    a_params,
                                    func_kind,
                                    func_return_type,
                                ));
                            }
                        }

                        // Script function (user defined)
                        if let Some(index) = self.resolve_script_function(name) {
                            let signature = self.functions[index].clone();

                            if a_params.len() != signature.param_types.len() {
                                return Err(ConstructError::new(
                                    location,
                                    ConstructErrorType::InvalidCall(
                                        name.to_string(),
                                        signature.param_types,
                                        a_params.iter().map(|v| v.get_value_type()).collect(),
                                    ),
                                ));
                            }

                            for (expr, expected_type) in
                                a_params.iter().zip(signature.param_types.iter())
                            {
                                let expr_type = expr
                                    .get_value_type()
                                    .ok_or_else(|| {
                                        ConstructError::new(
                                            expr.get_location(),
                                            ConstructErrorType::NoValueProduced,
                                        )
                                    })?
                                    .value_type();

                                if &expr_type != expected_type {
                                    return Err(ConstructError::new(
                                        expr.get_location(),
                                        ConstructErrorType::InvalidCall(
                                            name.to_string(),
                                            signature.param_types.clone(),
                                            a_params.iter().map(|v| v.get_value_type()).collect(),
                                        ),
                                    ));
                                }
                            }

                            for _ in 0..params.len() {
                                self.pop_stack()
                                    .expect("Checked above, this is to clear out the args");
                            }

                            if let Some(ref rt) = signature.return_type {
                                self.push_stack(location, rt.clone());
                            }

                            return Ok(AExpr::ScriptCall(
                                location,
                                index,
                                a_params,
                                signature.return_type,
                            ));
                        }

                        if let Some((value_type, global)) = self.module.resolve_global(name) {
                            let aexpr = AExpr::Constant(
                                location,
                                global.as_ref().clone_sync_box(),
                                value_type.clone(),
                            );

                            self.push_stack(location, value_type.clone());

                            // Convert globals to constants
                            return Ok(aexpr);
                        }

                        // Dynamic call through a function typed variable
                        let a_calling = self.expression(calling, is_maybe_mode, None)?;
                        let Some(value_type) = a_calling.get_value_type() else {
                            return Err(ConstructError::new(
                                location,
                                ConstructErrorType::InvalidCall(
                                    name.to_string(),
                                    a_param_types,
                                    a_params.iter().map(|v| v.get_value_type()).collect(),
                                ),
                            ));
                        };
                        self.pop_stack()
                            .expect("Checked above, this is to clear out caller");

                        match value_type {
                            MaybeValueType::Set(ValueType::Function(
                                name,
                                param_types,
                                return_type,
                            )) => {
                                let return_type = return_type.map(|b| *b);
                                let mut a_params = Vec::with_capacity(params.len());
                                for param in params {
                                    a_params.push(Box::new(self.expression(
                                        param,
                                        is_maybe_mode,
                                        None,
                                    )?));
                                }

                                if a_params.len() != param_types.len() {
                                    return Err(ConstructError::new(
                                        location,
                                        ConstructErrorType::InvalidCall(
                                            name.to_string(),
                                            param_types,
                                            a_params.iter().map(|v| v.get_value_type()).collect(),
                                        ),
                                    ));
                                }

                                for (expr, expected_type) in a_params.iter().zip(param_types.iter())
                                {
                                    let expr_type = expr
                                        .get_value_type()
                                        .ok_or_else(|| {
                                            ConstructError::new(
                                                expr.get_location(),
                                                ConstructErrorType::NoValueProduced,
                                            )
                                        })?
                                        .value_type();

                                    if &expr_type != expected_type {
                                        return Err(ConstructError::new(
                                            expr.get_location(),
                                            ConstructErrorType::InvalidCall(
                                                name.to_string(),
                                                param_types,
                                                a_params
                                                    .iter()
                                                    .map(|v| v.get_value_type())
                                                    .collect(),
                                            ),
                                        ));
                                    }
                                }

                                for _ in 0..params.len() {
                                    self.pop_stack()
                                        .expect("Checked above, this is to clear out the args");
                                }

                                if let Some(ref rt) = return_type {
                                    self.push_stack(location, rt.clone());
                                }

                                return Ok(AExpr::DynamicCall(
                                    location,
                                    Box::new(a_calling),
                                    a_params,
                                    return_type,
                                ));
                            }
                            MaybeValueType::Set(vt) => {
                                return Err(ConstructError::new(
                                    location,
                                    ConstructErrorType::UnknownCall(
                                        name.to_string(),
                                        vt,
                                        a_param_types,
                                    ),
                                ));
                            }
                            val => {
                                return Err(ConstructError::new(
                                    location,
                                    ConstructErrorType::VariablePotentiallyUninitialized(val),
                                ));
                            }
                        }
                    }
                    Expr::QualifiedAccess(location, namespace, function_name) => {
                        let mut program_index = None;

                        // Find program
                        for (index, import) in self.imports.iter().enumerate() {
                            if import.0.as_str() == namespace.as_str() {
                                program_index = Some(index);
                                break;
                            }
                        }
                        let Some(program_index) = program_index else {
                            return Err(ConstructError::new(
                                *location,
                                ConstructErrorType::NamespaceNotImported(namespace.to_string()),
                            ));
                        };
                        let (_, imported_program) = &self.imports[program_index];

                        // Find function
                        for (function_index, function) in
                            imported_program.functions.clone().iter().enumerate()
                        {
                            if function.name.as_str() == function_name.as_str() {
                                let mut args = Vec::with_capacity(params.len());
                                for param in params {
                                    args.push(Box::new(self.expression(
                                        param,
                                        is_maybe_mode,
                                        None,
                                    )?));
                                }

                                if args.len() != function.params.len() {
                                    return Err(ConstructError::new(
                                        *location,
                                        ConstructErrorType::InvalidCall(
                                            function.name.to_string(),
                                            function.params.clone(),
                                            args.iter().map(|v| v.get_value_type()).collect(),
                                        ),
                                    ));
                                }

                                for (expr, expected_type) in args.iter().zip(function.params.iter())
                                {
                                    let expr_type = expr
                                        .get_value_type()
                                        .ok_or_else(|| {
                                            ConstructError::new(
                                                expr.get_location(),
                                                ConstructErrorType::NoValueProduced,
                                            )
                                        })?
                                        .value_type();

                                    if &expr_type != expected_type {
                                        return Err(ConstructError::new(
                                            expr.get_location(),
                                            ConstructErrorType::InvalidCall(
                                                function.name.to_string(),
                                                function.params.clone(),
                                                args.iter().map(|v| v.get_value_type()).collect(),
                                            ),
                                        ));
                                    }
                                }

                                for _ in 0..args.len() {
                                    self.pop_stack()
                                        .expect("Checked above, this is to clear out the args");
                                }

                                // We're good to go!
                                let result = Ok(AExpr::ImportedFunctionCall(
                                    *location,
                                    program_index,
                                    function_index,
                                    function.params.clone(),
                                    function.return_type.clone(),
                                    args,
                                ))?;

                                if let Some(return_type) = function.return_type.clone() {
                                    self.push_stack(*location, return_type);
                                }

                                return Ok(result);
                            }
                        }

                        return Err(ConstructError::new(
                            *location,
                            ConstructErrorType::ImportedFunctionNotFound(
                                namespace.to_string() + "::" + function_name.as_str(),
                            ),
                        ));
                    }
                    other => {
                        // Perhaps  this needs to be recursive?
                        //self.expression(other, is_maybe_mode, type_hint)?

                        return Err(ConstructError::new(
                            location,
                            ConstructErrorType::UnexpectedCall(Box::new(other.clone())),
                        ));
                    }
                }
            }
            Expr::Block(location, stmts) => {
                self.push_scope();

                let mut values = Vec::with_capacity(stmts.len());
                for stmt in stmts {
                    values.push(self.statement(stmt, false, type_hint)?);
                }

                let scope = self.pop_scope();

                // We *maybe* complete this scope
                if let Err(err) = self.apply_scope_variables(&scope, is_maybe_mode) {
                    return Err(ConstructError::new(*location, err));
                }

                AExpr::Block(*location, values, scope)
            }
            Expr::Index(receiver, index) => {
                // We're expecting this to be a "variable access"
                self.expression(
                    &Expr::DotAccess(
                        receiver.clone(),
                        "index".to_string(),
                        Some(vec![index.clone()]),
                    ),
                    false,
                    None,
                )?
            }
            Expr::DotAccess(receiver, name, args) => {
                // Get location before consuming receiver
                let location = receiver.get_location();

                // Evaluate the receiver expression and pop its concrete type
                let a_receiver = self.expression(receiver, is_maybe_mode, None)?;
                let (_, receiver_type) = self.pop_stack().unwrap();

                let mut source_mutable = true;
                let source = match receiver.as_ref() {
                    Expr::Variable(source, var_name) => {
                        // Find the slot for this variable in current scope
                        if let Ok((slot, mutability, _)) = self.find_variable(*source, var_name) {
                            if mutability != VariableMutability::Mutable {
                                source_mutable = false;
                            }

                            CallSource::Variable(slot)
                        } else {
                            CallSource::Stack
                        }
                    }
                    _ => CallSource::Stack,
                };

                // Build params
                let mut a_params = vec![];
                a_params.push(Box::new(a_receiver));
                let mut a_param_types = vec![];
                a_param_types.push(receiver_type.clone());

                for arg in args.as_deref().unwrap_or_default() {
                    let a_expr = self.expression(arg, is_maybe_mode, None)?;

                    let Some(MaybeValueType::Set(typed)) = a_expr.get_value_type() else {
                        return Err(ConstructError::new(
                            a_expr.get_location(),
                            if a_expr.get_value_type().is_none() {
                                ConstructErrorType::NoValueProduced
                            } else {
                                ConstructErrorType::VariableUninitialized
                            },
                        ));
                    };
                    // Clean stack
                    self.pop_stack();

                    a_params.push(Box::new(a_expr));
                    a_param_types.push(typed);
                }

                // Find matching method
                let Some(func_info) = self
                    .module
                    .find_method(&receiver_type, name, &a_param_types)
                    .cloned()
                else {
                    return Err(ConstructError::new(
                        location,
                        ConstructErrorType::NoMethod(receiver_type, name.clone(), a_param_types),
                    ));
                };

                // Validate call style against FunctionKind
                match func_info.kind {
                    FunctionKind::Property(_) => {
                        if args.is_some() {
                            return Err(ConstructError::new(
                                location,
                                ConstructErrorType::PropertyCalledAsMethod(name.clone()),
                            ));
                        }
                    }
                    FunctionKind::Method(_) => {
                        if args.is_none() {
                            return Err(ConstructError::new(
                                location,
                                ConstructErrorType::MethodCalledAsProperty(name.clone()),
                            ));
                        }
                    }
                    FunctionKind::MethodMut(_) => {
                        if args.is_none() {
                            return Err(ConstructError::new(
                                location,
                                ConstructErrorType::MethodCalledAsProperty(name.clone()),
                            ));
                        }

                        if !source_mutable {
                            return Err(ConstructError::new(
                                location,
                                ConstructErrorType::VariableImmutable,
                            ));
                        }
                    }
                    FunctionKind::Free(_) => {
                        unreachable!("FunctionKind::Free in DotAccess lookup for method '{name}'?");
                    }
                    FunctionKind::Setter(_) => {
                        return Err(ConstructError::new(
                            location,
                            ConstructErrorType::PropertyCalledAsMethod(name.clone()),
                        ));
                    }
                }

                // Push return type to the stack
                if let Some(rt) = &func_info.return_type {
                    self.push_stack(location, rt.clone());
                }

                AExpr::Call(
                    location,
                    func_info.name,
                    source,
                    a_params,
                    func_info.kind,
                    func_info.return_type,
                )
            }
            Expr::QualifiedAccess(location, alias, name) => {
                // Find what we're referencing
                let full_name = alias.to_owned() + "::" + name.as_str();
                let Some((value_type, global)) = self.module.resolve_global(&(full_name)) else {
                    return Err(ConstructError::new(
                        *location,
                        ConstructErrorType::QualifiedAccessNotCallOrGlobal(full_name),
                    ));
                };

                // Pull out returned value (only constants handled here)
                let value_type = value_type.clone();
                let global = global.as_ref().clone_sync_box();

                self.push_stack(*location, value_type.clone());

                AExpr::Constant(*location, global, value_type.clone())

                // TODO: Remove this, it shouldn't be needed?
                /*
                // Get args
                let mut args = Vec::with_capacity(exprs.len());
                for expr in exprs {
                    args.push(Box::new(self.expression(expr, false)?));
                }

                // Find the import corresponding to this access
                let Some((program_index, (_, program))) = self
                    .imports
                    .iter()
                    .enumerate()
                    .find(|v| v.1.0.as_str() == alias.as_str())
                else {
                    return Err(ConstructError::new(
                        *location,
                        ConstructErrorType::NamespaceNotImported(alias.to_string()),
                    ));
                };

                // Find the actual function referenced
                let Some((call_index, call_info)) = program
                    .call_info
                    .iter()
                    .enumerate()
                    .find(|v| v.1.name.as_str() == name.as_str())
                else {
                    return Err(ConstructError::new(
                        *location,
                        ConstructErrorType::ImportedFunctionNotFound(name.to_string()),
                    ));
                };

                AExpr::ImportedFunctionCall(
                    *location,
                    program_index,
                    call_index,
                    call_info.params.clone(),
                    call_info.return_type.clone(),
                    args,
                )*/
            }
            Expr::Assign(destination, op, value) => {
                let is_dot_access = matches!(destination.as_ref(), Expr::DotAccess(_, _, _));
                if !matches!(destination.as_ref(), Expr::Variable(_, _))
                    && !matches!(destination.as_ref(), Expr::Index(_, _))
                    && !is_dot_access
                {
                    // We don't support it
                    return Err(ConstructError::new(
                        destination.get_location(),
                        ConstructErrorType::InvalidAssignTarget,
                    ));
                }

                if *op == AssignmentOp::Equal {
                    match destination.as_ref() {
                        Expr::Index(destination, index) => self.expression(
                            &Expr::DotAccess(
                                destination.clone(),
                                "set".to_string(),
                                Some(vec![index.clone(), value.clone()]),
                            ),
                            false,
                            None,
                        )?,
                        Expr::DotAccess(receiver, field_name, _) => {
                            let value_expr = self.expression(value.as_ref(), false, None)?;

                            // Get expression typing
                            let value_type = match value_expr.get_value_type() {
                                Some(MaybeValueType::Set(t)) => t.clone(),
                                _ => {
                                    return Err(ConstructError::new(
                                        value_expr.get_location(),
                                        ConstructErrorType::NoValueProduced,
                                    ));
                                }
                            };

                            // Get receiver typing
                            let receiver_expr = self.expression(receiver.as_ref(), false, None)?;
                            let (_, receiver_type) = self.pop_stack().unwrap();

                            // Look up setter method
                            let setter_params = vec![receiver_type.clone(), value_type.clone()];
                            let Some(func_info) =
                                self.module
                                    .find_method(&receiver_type, field_name, &setter_params)
                            else {
                                return Err(ConstructError::new(
                                    receiver_expr.get_location(),
                                    ConstructErrorType::NoMethod(
                                        receiver_type.clone(),
                                        field_name.clone(),
                                        vec![value_type],
                                    ),
                                ));
                            };

                            let kind = func_info.kind.clone();

                            // Validate that the found function is a Setter
                            if !matches!(kind, FunctionKind::Setter(_)) {
                                return Err(ConstructError::new(
                                    receiver_expr.get_location(),
                                    match &kind {
                                        FunctionKind::Property(_) => {
                                            ConstructErrorType::MethodCalledAsProperty(
                                                field_name.clone(),
                                            )
                                        }
                                        FunctionKind::Method(_) | FunctionKind::MethodMut(_) => {
                                            ConstructErrorType::PropertyCalledAsMethod(
                                                field_name.clone(),
                                            )
                                        }
                                        _ => ConstructErrorType::NoMethod(
                                            receiver_type,
                                            field_name.clone(),
                                            vec![value_type],
                                        ),
                                    },
                                ));
                            }

                            // Check variable mutability if the receiver is a variable.
                            if let Expr::Variable(_, var_name) = receiver.as_ref() {
                                if let Ok((_, mutability, _)) =
                                    self.find_variable(receiver_expr.get_location(), var_name)
                                {
                                    match mutability {
                                        VariableMutability::Constant => {
                                            return Err(ConstructError::new(
                                                receiver_expr.get_location(),
                                                ConstructErrorType::VariableImmutable,
                                            ));
                                        }
                                        VariableMutability::Immutable => {
                                            return Err(ConstructError::new(
                                                receiver_expr.get_location(),
                                                ConstructErrorType::VariableImmutable,
                                            ));
                                        }
                                        VariableMutability::Mutable => {}
                                    }
                                }
                            }

                            let source = match receiver.as_ref() {
                                Expr::Variable(source_loc, var_name) => {
                                    if let Ok((slot, _, _)) =
                                        self.find_variable(*source_loc, var_name)
                                    {
                                        CallSource::Variable(slot)
                                    } else {
                                        CallSource::Stack
                                    }
                                }
                                _ => CallSource::Stack,
                            };

                            // Setters don't return a value
                            self.pop_stack();

                            AExpr::Call(
                                receiver_expr.get_location(),
                                field_name.clone(),
                                source,
                                vec![Box::new(receiver_expr), Box::new(value_expr)],
                                kind,
                                None,
                            )
                        }
                        Expr::Variable(location, name) => {
                            // "Normal" assignment
                            let value_expr = self.expression(value, false, type_hint)?;
                            let Some(MaybeValueType::Set(stmt_type)) = value_expr.get_value_type()
                            else {
                                return Err(ConstructError::new(
                                    value_expr.get_location(),
                                    ConstructErrorType::AssignerDoesntProduce,
                                ));
                            };

                            let (slot, mutability, value_type) =
                                self.find_variable(*location, name)?;

                            // Make sure mutability is followed
                            match mutability {
                                VariableMutability::Constant => {
                                    return Err(ConstructError::new(
                                        *location,
                                        ConstructErrorType::VariableImmutable,
                                    ));
                                }
                                VariableMutability::Immutable => {
                                    if value_type.is_some() {
                                        return Err(ConstructError::new(
                                            *location,
                                            ConstructErrorType::VariableImmutable,
                                        ));
                                    }
                                }
                                VariableMutability::Mutable => {}
                            }

                            self.modify_variable(&slot, stmt_type);

                            AExpr::AssignVariable(
                                *location,
                                slot,
                                name.to_owned(),
                                Box::new(value_expr),
                            )
                        }
                        _ => unreachable!(),
                    }
                } else {
                    if let Expr::Index(destination, index) = destination.as_ref() {
                        // Convert to a "retrieve, op, restore"
                        let value = Box::new(Expr::DotAccess(
                            Box::new(Expr::DotAccess(
                                destination.clone(),
                                "index".to_string(),
                                Some(vec![index.clone()]),
                            )),
                            op.base_op().to_string(),
                            Some(vec![value.clone()]),
                        ));

                        self.expression(
                            &Expr::DotAccess(
                                destination.clone(),
                                "set".to_owned(),
                                Some(vec![index.clone(), value]),
                            ),
                            false,
                            None,
                        )?
                    } else if is_dot_access {
                        // Compound assignment on dot access (obj.field += value)
                        // Inlined to avoid infinite recursion through expression().

                        // Base operation
                        let compute_expr = self.expression(
                            &Expr::DotAccess(
                                destination.clone(),
                                op.base_op().to_string(),
                                Some(vec![value.clone()]),
                            ),
                            false,
                            None,
                        )?;

                        // Get value
                        let (_, compute_type) = self.pop_stack().unwrap();

                        // Setter assignment, DotAccess setter logic inline
                        // TODO: Validate
                        if let Expr::DotAccess(receiver, field_name, _) = destination.as_ref() {
                            // Get receiver typing
                            let receiver_expr = self.expression(receiver.as_ref(), false, None)?;
                            let (_, receiver_type) = self.pop_stack().unwrap();

                            // Get setter
                            let setter_params = vec![receiver_type.clone(), compute_type.clone()];
                            let Some(func_info) =
                                self.module
                                    .find_method(&receiver_type, field_name, &setter_params)
                            else {
                                return Err(ConstructError::new(
                                    receiver_expr.get_location(),
                                    ConstructErrorType::NoMethod(
                                        receiver_type.clone(),
                                        field_name.clone(),
                                        vec![compute_type],
                                    ),
                                ));
                            };

                            let kind = func_info.kind.clone();

                            if !matches!(kind, FunctionKind::Setter(_)) {
                                return Err(ConstructError::new(
                                    receiver_expr.get_location(),
                                    match &kind {
                                        FunctionKind::Property(_) => {
                                            ConstructErrorType::MethodCalledAsProperty(
                                                field_name.clone(),
                                            )
                                        }
                                        FunctionKind::Method(_) | FunctionKind::MethodMut(_) => {
                                            ConstructErrorType::PropertyCalledAsMethod(
                                                field_name.clone(),
                                            )
                                        }
                                        _ => ConstructErrorType::NoMethod(
                                            receiver_type,
                                            field_name.clone(),
                                            vec![compute_type],
                                        ),
                                    },
                                ));
                            }

                            // Check mutability
                            if let Expr::Variable(_, var_name) = receiver.as_ref() {
                                if let Ok((_, mutability, _)) =
                                    self.find_variable(receiver_expr.get_location(), var_name)
                                {
                                    match mutability {
                                        VariableMutability::Constant
                                        | VariableMutability::Immutable => {
                                            return Err(ConstructError::new(
                                                receiver_expr.get_location(),
                                                ConstructErrorType::VariableImmutable,
                                            ));
                                        }
                                        VariableMutability::Mutable => {}
                                    }
                                }
                            }

                            let source = match receiver.as_ref() {
                                Expr::Variable(source_loc, var_name) => {
                                    if let Ok((slot, _, _)) =
                                        self.find_variable(*source_loc, var_name)
                                    {
                                        CallSource::Variable(slot)
                                    } else {
                                        CallSource::Stack
                                    }
                                }
                                _ => CallSource::Stack,
                            };

                            AExpr::Call(
                                receiver_expr.get_location(),
                                field_name.clone(),
                                source,
                                vec![Box::new(receiver_expr), Box::new(compute_expr)],
                                kind,
                                None,
                            )
                        } else {
                            unreachable!(
                                "'is_dot_access' was true but destination is not DotAccess"
                            )
                        }
                    } else {
                        self.expression(
                            &Expr::DotAccess(
                                destination.clone(),
                                op.to_string(),
                                Some(vec![value.clone()]),
                            ),
                            false,
                            None,
                        )?
                    }
                }
            }
            Expr::If(condition_expr, body_expr, otherwise) => {
                let condition = Box::new(self.expression(condition_expr, false, None)?);
                if condition.get_value_type() != Some(MaybeValueType::Set(ValueType::Bool)) {
                    return Err(ConstructError::new(
                        condition.get_location(),
                        ConstructErrorType::ExpectedBoolean(
                            condition.get_value_type().map(|v| v.value_type()),
                        ),
                    ));
                }

                // Capture the current variable slot count BEFORE pushing child scopes.
                // Variables created inside branches at >= this count should not leak out, but existing variables merged back correctly.
                let call_frame = self.call_frames.last_mut().expect("Should have call frame");
                let pre_if_else_slot_count = call_frame.variable_slots.len();

                // Process body in its own scope
                self.push_scope();
                let body = Box::new(self.expression(body_expr, false, type_hint)?);
                let body_scope = self.pop_scope();

                let otherwise = if let Some(otherwise_expr) = otherwise {
                    // Process otherwise in its own scope
                    self.push_scope();
                    let otherwise = Box::new(self.expression(otherwise_expr, false, type_hint)?);
                    let otherwise_scope = self.pop_scope();

                    // These are optional, so we compare no returns as well as mistyped.
                    // If one branch unconditionally exits use the non exiting branch's type.
                    // A branch with an exit path (partial exit) can coexist with any value producing branch.
                    let body_unconditional_exit = body.is_unconditional_exit();
                    let other_unconditional_exit = otherwise.is_unconditional_exit();
                    let body_has_exit = body.has_exit_path() && !body_unconditional_exit;
                    let other_has_exit = otherwise.has_exit_path() && !other_unconditional_exit;

                    // Types match if:
                    // - Direct type equality, OR
                    // - One branch unconditionally exits (return / break / continue on all paths)
                    // - One branch has a partial exit path and the other produces any value
                    let types_match = body.get_value_type() == otherwise.get_value_type()
                        || body_unconditional_exit
                        || other_unconditional_exit
                        || (body_has_exit && otherwise.get_value_type().is_some())
                        || (other_has_exit && body.get_value_type().is_some());
                    if !types_match {
                        return Err(ConstructError::new(
                            body.get_location(),
                            ConstructErrorType::IfElseTypeMismatched(
                                body.get_value_type(),
                                otherwise.get_value_type(),
                            ),
                        ));
                    }

                    // Merge scopes
                    // A branch unconditionally exiting should set variables in the non exiting branch to Set instead of downgrading to Maybe.
                    let merged = if otherwise.is_unconditional_exit()
                        && !body.is_unconditional_exit()
                    {
                        // Body doesn't exit
                        let mut merged = body_scope.variable_state.clone();
                        for (index, other_type) in &otherwise_scope.variable_state {
                            if !merged.contains_key(index) {
                                merged.insert(*index, other_type.to_maybe());
                            }
                        }
                        merged
                    } else if body.is_unconditional_exit() && !otherwise.is_unconditional_exit() {
                        // Otherwise doesn't exit
                        let mut merged = otherwise_scope.variable_state.clone();
                        for (index, body_type) in &body_scope.variable_state {
                            if !merged.contains_key(index) {
                                merged.insert(*index, body_type.to_maybe());
                            }
                        }
                        merged
                    } else {
                        // Both branches may not execute.
                        let mut merged = body_scope.variable_state.clone();
                        for (index, other_type) in &otherwise_scope.variable_state {
                            match merged.get_mut(index) {
                                Some(body_type) => {
                                    if matches!(body_type, MaybeValueType::Set(_))
                                        && matches!(other_type, MaybeValueType::Set(_))
                                    {
                                        // Both Set, keep Set (type checked above)
                                        if body_type.value_type() != other_type.value_type() {
                                            return Err(ConstructError::new(
                                                condition.get_location(),
                                                ConstructErrorType::VariableTypeMaybeAltered(
                                                    body_type.value_type(),
                                                    other_type.value_type(),
                                                ),
                                            ));
                                        }
                                    } else {
                                        // At least one is Maybe, make it maybe
                                        let body_val = body_type.value_type();
                                        let other_val = other_type.value_type();
                                        if body_val != other_val {
                                            return Err(ConstructError::new(
                                                condition.get_location(),
                                                ConstructErrorType::VariableTypeMaybeAltered(
                                                    body_val, other_val,
                                                ),
                                            ));
                                        }
                                        *body_type = MaybeValueType::Maybe(body_val);
                                    }
                                }
                                None => {
                                    // Variable only in otherwise, it's a maybe
                                    merged.insert(*index, other_type.to_maybe());
                                }
                            }
                        }

                        merged
                    };

                    // Apply merged scope directly to the top scope.
                    // Variables created inside either branch at >= this count should not leak out as new bindings.
                    let source_start = pre_if_else_slot_count;
                    let call_frame = self.call_frames.last_mut().expect("Should have call frame");
                    let target_scope = call_frame.scope.last_mut().expect("Expected scope");
                    for (index, value_type) in merged {
                        if index >= source_start {
                            continue;
                        }

                        match target_scope.variable_state.get_mut(&index) {
                            Some(existing) => match existing {
                                MaybeValueType::Set(_) => {
                                    if existing.value_type() != value_type.value_type() {
                                        return Err(ConstructError::new(
                                            condition.get_location(),
                                            ConstructErrorType::VariableTypeMaybeAltered(
                                                existing.value_type(),
                                                value_type.value_type(),
                                            ),
                                        ));
                                    }
                                }
                                MaybeValueType::Maybe(_) => {
                                    // Existing is Maybe, upgrade to Set if the incoming type matches.
                                    // This happens when both branches of an if else assign to a variable that was declared without initialization (let x;).
                                    if value_type.value_type() == existing.value_type() {
                                        *existing = value_type.clone();
                                    } else {
                                        return Err(ConstructError::new(
                                            condition.get_location(),
                                            ConstructErrorType::VariableTypeMaybeAltered(
                                                existing.value_type(),
                                                value_type.value_type(),
                                            ),
                                        ));
                                    }
                                }
                            },
                            None => {
                                target_scope.variable_state.insert(index, value_type);
                            }
                        }
                    }

                    Some(otherwise)
                } else if body.get_value_type().is_some() {
                    // Invalid, otherwise it'd be a "maybe" assignment (I don't like it)
                    return Err(ConstructError::new(
                        body.get_location(),
                        ConstructErrorType::IfValueWithoutElse,
                    ));
                } else {
                    // No else :P
                    if let Err(err) = self.apply_scope_variables(&body_scope, true) {
                        return Err(ConstructError::new(condition.get_location(), err));
                    }
                    None
                };

                AExpr::If(condition, body, otherwise)
            }
        };

        // Ensure typing matches
        let expr_type = a_expr.get_value_type();
        if expr_type.map(|v: MaybeValueType| v.value_type()) != type_hint.cloned()
            && type_hint.is_some()
        {
            return Err(ConstructError::new(
                a_expr.get_location(),
                ConstructErrorType::TypeHintMismatch(type_hint.cloned(), a_expr.get_value_type()),
            ));
        }

        Ok(a_expr)
    }
}
