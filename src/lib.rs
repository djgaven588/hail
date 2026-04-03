mod benching;
mod machine;
mod parser;
mod scanner;
mod walker;

use std::{
    any::{Any, TypeId, type_name},
    fmt::Debug,
    fs,
    hash::Hash,
    sync::Arc,
};

use hashbrown::HashMap;

use crate::{
    parser::{AssignmentOp, BinaryOp, Parser, Stmt, UnaryOp, VariableMutability},
    scanner::Scanner,
};

pub fn run(script_name: String, is_bench: bool) {
    if is_bench {
        benching::bench(&script_name);
        return;
    }

    println!("Running: {script_name}");
    let source = fs::read_to_string(script_name + ".hail").expect("Script should be at location.");

    let mut scanner = Scanner::new(source);
    scanner.scan();

    println!("Tokens:");
    for token in scanner.get().unwrap() {
        println!("Token: {token:?}");
    }

    println!("Syntax Errors:");
    for error in scanner.errors() {
        println!("Error: {error:?}");
    }

    let mut parser = Parser::new(scanner.get().unwrap());
    parser.parse();
    println!("Parse Errors:");
    for error in parser.errors() {
        println!("Error: {error:?}");
    }

    if let Some(stmts) = parser.get() {
        println!("Running walker...");
        let result = walker::ExecutionContext::new(library()).run(stmts);
        println!("Result: {result:?}");

        let vm = machine::Vm::new(library(), stmts);
        println!("Running machine...");
        let result = vm.run();
        println!("Result: {result:?}");
    } else {
        println!("Failed to get statements.");
    }

    /*
     */
}

pub trait Scriptable {
    fn to_dynamic(self) -> Dynamic;
    fn from_dynamic(dynamic: Dynamic) -> Self;
}

impl Scriptable for f64 {
    fn to_dynamic(self) -> Dynamic {
        Dynamic::Float(self)
    }

    fn from_dynamic(dynamic: Dynamic) -> Self {
        let Dynamic::Float(val) = dynamic else {
            unreachable!();
        };

        val
    }
}

impl Scriptable for i64 {
    fn to_dynamic(self) -> Dynamic {
        Dynamic::Integer(self)
    }
    fn from_dynamic(dynamic: Dynamic) -> Self {
        let Dynamic::Integer(val) = dynamic else {
            unreachable!();
        };

        val
    }
}

impl Scriptable for String {
    fn to_dynamic(self) -> Dynamic {
        Dynamic::String(self)
    }
    fn from_dynamic(dynamic: Dynamic) -> Self {
        let Dynamic::String(val) = dynamic else {
            unreachable!();
        };

        val
    }
}

impl Scriptable for bool {
    fn to_dynamic(self) -> Dynamic {
        Dynamic::Bool(self)
    }
    fn from_dynamic(dynamic: Dynamic) -> Self {
        let Dynamic::Bool(val) = dynamic else {
            unreachable!();
        };

        val
    }
}

impl Scriptable for Box<Arc<NativeFuncInfo>> {
    fn to_dynamic(self) -> Dynamic {
        Dynamic::NativeFunc(self)
    }
    fn from_dynamic(dynamic: Dynamic) -> Self {
        let Dynamic::NativeFunc(val) = dynamic else {
            unreachable!();
        };

        val
    }
}

pub fn library() -> Arc<Module> {
    let mut module = Module::default();

    module
        .define(
            "SOMETHING",
            Box::new(Arc::new(NativeFuncInfo {
                name: "SOMETHING".to_owned(),
                signature: vec![TypeId::of::<String>()],
                param_info: vec![type_name::<String>()],
                call: |executor, params| {
                    let arg = params.into_iter().next().unwrap();
                    let arg = arg.unwrap_string();
                    Ok(Some(Dynamic::String(format!(
                        "Something was called! Param: {arg}"
                    ))))
                },
            })),
        )
        .unwrap();

    Arc::new(module)
}

#[derive(Default)]
pub struct Module {
    globals: HashMap<String, Dynamic>,
}

impl Module {
    pub fn global(&self, name: &str) -> Option<&Dynamic> {
        self.globals.get(name)
    }

    /// Define constants, functions, anything implementing ``Scripting`` that are made available to scripts
    pub fn define<T: Scriptable>(&mut self, name: &str, value: T) -> Result<(), String> {
        let Some(existing) = self.globals.insert(name.to_string(), value.to_dynamic()) else {
            return Ok(());
        };

        Err(format!("'{name}' was occupied with '{existing:?}'"))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct VariableName {
    hash: u64,
    name: String,
}

impl Hash for VariableName {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.hash.hash(state);
    }
}

impl VariableName {
    pub fn new(name: String) -> VariableName {
        todo!()
        /*
        VariableName {
            hash: name.hash(state);
        } */
    }
}

#[derive(Default)]
pub struct Scope {
    variables: HashMap<String, VariableState>,
}

#[derive(Default)]
struct Frame {
    return_address: usize,
    scopes: Vec<Scope>,
}

impl Frame {
    fn new(scope: Scope, return_address: usize) -> Frame {
        Self {
            scopes: vec![scope],
            return_address,
        }
    }
}

struct Scoper {
    module: Arc<Module>,
    frames: Vec<Frame>,
}

impl Scoper {
    /// Create an execution scope with the provided library and initial scope
    /// Generally a module will be provided, a scope is truly optional
    pub fn new(module: Arc<Module>, scope: Option<Scope>) -> Scoper {
        Scoper {
            module,
            frames: vec![Frame::new(scope.unwrap_or_else(|| Scope::default()), 0)],
        }
    }

    /// Enter a new function frame
    pub fn enter(&mut self, return_address: usize) {
        self.frames
            .push(Frame::new(Scope::default(), return_address));
    }

    // Exit a function frame
    pub fn exit(&mut self) -> Option<usize> {
        let return_address = self
            .frames
            .pop()
            .expect("Frame should exist.")
            .return_address;

        // If we've removed the root frame, we're done.
        if self.frames.is_empty() {
            return None;
        }

        Some(return_address)
    }

    pub fn push(&mut self, scope: Option<Scope>) {
        self.frames
            .last_mut()
            .expect("Should always have a call frame")
            .scopes
            .push(scope.unwrap_or(Scope::default()));
    }

    pub fn pop(&mut self) {
        self.frames
            .last_mut()
            .expect("Should always have a call frame")
            .scopes
            .pop();
    }

    pub fn define_variable(
        &mut self,
        name: &str,
        mutability: VariableMutability,
        value: Dynamic,
        location: Location,
    ) -> Result<(), ExecutionError> {
        // If it's a constant, make sure we're not bypassing the fact it's a constant by redefining it
        // Constants are still *scoped*, this is more of a "enforce good behavior" that can be removed if needed
        let frame = self
            .frames
            .last_mut()
            .expect("Should always have a call frame");

        frame
            .scopes
            .last_mut()
            .expect("A scope should always exist.")
            .variables
            .insert(name.to_string(), VariableState::new(value, mutability));
        Ok(())
    }

    pub fn mut_variable(
        &mut self,
        name: &str,
        location: Location,
        modify: impl FnOnce(&mut VariableState, Location) -> Result<(), ExecutionError>,
    ) -> Result<(), ExecutionError> {
        let frame = self
            .frames
            .last_mut()
            .expect("Should always have a call frame");

        let len = frame.scopes.len();
        for i in 0..len {
            // Reverse loop
            let i = len - i - 1;

            // Search for variable we can mutate
            if let Some(found) = frame.scopes[i].variables.get_mut(name) {
                if found.mutability != VariableMutability::Mutable {
                    return Err(ExecutionError::new(
                        location,
                        ExecutionErrorType::VariableImmutable(name.to_string()),
                    ));
                }

                return modify(found, location);
            }
        }

        // Check global
        if self.frames.len() > 1 {
            let frame = self
                .frames
                .first_mut()
                .expect("Should always have a call frame");
            let len = frame.scopes.len();
            for i in 0..len {
                // Reverse loop
                let i = len - i - 1;

                // Search for variable we can mutate
                if let Some(found) = frame.scopes[i].variables.get_mut(name) {
                    if found.mutability != VariableMutability::Mutable {
                        return Err(ExecutionError::new(
                            location,
                            ExecutionErrorType::VariableImmutable(name.to_string()),
                        ));
                    }

                    return modify(found, location);
                }
            }
        }

        Err(ExecutionError::new(
            location,
            ExecutionErrorType::VariableUndefined(name.to_string()),
        ))
    }

    pub fn get_variable(
        &mut self,
        name: &str,
        location: Location,
    ) -> Result<Dynamic, ExecutionError> {
        {
            // Current Stack frame
            let frame = self
                .frames
                .last_mut()
                .expect("Should always have a call frame");
            let len = frame.scopes.len();
            for i in 0..len {
                let i = len - i - 1;
                if let Some(found) = frame.scopes[i].variables.get_mut(name) {
                    return Ok(found.value.clone());
                }
            }
        }

        if self.frames.len() > 1 {
            // Global Stack frame
            let frame = self
                .frames
                .first_mut()
                .expect("Should always have a call frame");
            let len = frame.scopes.len();
            for i in 0..len {
                let i = len - i - 1;
                if let Some(found) = frame.scopes[i].variables.get_mut(name) {
                    return Ok(found.value.clone());
                }
            }
        }

        let Some(global) = self.module.global(name) else {
            return Err(ExecutionError::new(
                location,
                ExecutionErrorType::VariableUndefined(name.to_string()),
            ));
        };

        Ok(global.clone())
    }
}

pub trait Executor {}

#[derive(Debug)]
pub struct NativeFuncInfo {
    pub name: String,
    pub signature: Vec<TypeId>,
    pub param_info: Vec<&'static str>,
    pub call: fn(&mut dyn Executor, Vec<Dynamic>) -> Result<Option<Dynamic>, ExecutionError>,
}

#[derive(Debug)]
pub enum TempScuff {
    FuncParse(Box<Stmt>),
    FuncVM(usize),
}

impl TempScuff {
    pub fn unwrap_parse(&self) -> &Box<Stmt> {
        match self {
            TempScuff::FuncParse(stmt) => stmt,
            TempScuff::FuncVM(_) => todo!(),
        }
    }

    pub fn unwrap_vm(&self) -> usize {
        match self {
            TempScuff::FuncParse(stmt) => todo!(),
            TempScuff::FuncVM(pointer) => *pointer,
        }
    }
}

#[derive(Debug)]
pub struct FuncInfo {
    pub name: String,
    pub param_info: Vec<(VariableMutability, String)>,
    // TODO: This is a work around for the stack machine
    pub call: TempScuff,
}

#[derive(Debug, Clone)]
pub enum Dynamic {
    NativeFunc(Box<Arc<NativeFuncInfo>>),
    Func(Box<Arc<FuncInfo>>),
    Bool(bool),
    Integer(i64),
    Float(f64),
    String(String),
    Nil,
}

impl Dynamic {
    pub const fn get_type(&self) -> TypeId {
        match self {
            Dynamic::NativeFunc(_) => TypeId::of::<Box<Arc<NativeFuncInfo>>>(),
            Dynamic::Func(_) => TypeId::of::<Box<Arc<FuncInfo>>>(),
            Dynamic::Bool(_) => TypeId::of::<bool>(),
            Dynamic::Integer(_) => TypeId::of::<i64>(),
            Dynamic::Float(_) => TypeId::of::<f64>(),
            Dynamic::String(_) => TypeId::of::<String>(),
            Dynamic::Nil => TypeId::of::<()>(),
        }
    }

    pub fn unwrap_into<T>(self) -> T
    where
        T: Scriptable,
    {
        T::from_dynamic(self)
    }

    pub fn unwrap_integer(self) -> i64 {
        let Dynamic::Integer(val) = self else {
            unreachable!("Unwrap should be done carefully!");
        };
        val
    }

    pub fn unwrap_float(self) -> f64 {
        let Dynamic::Float(val) = self else {
            unreachable!("Unwrap should be done carefully!");
        };
        val
    }

    pub fn unwrap_string(self) -> String {
        let Dynamic::String(val) = self else {
            unreachable!("Unwrap should be done carefully!");
        };
        val
    }
}

impl PartialEq for Dynamic {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Bool(l0), Self::Bool(r0)) => l0 == r0,
            (Self::Integer(l0), Self::Integer(r0)) => l0 == r0,
            (Self::Float(l0), Self::Float(r0)) => l0 == r0,
            (Self::String(l0), Self::String(r0)) => l0 == r0,
            _ => false,
        }
    }
}

impl Eq for Dynamic {}

#[derive(Debug)]
enum ExecutionErrorType {
    VariableImmutable(String),
    VariableUndefined(String),
    InvalidOperator(Dynamic, BinaryOp, Dynamic),
    InvalidUnaryOperator(UnaryOp, Dynamic),
    DivideByZero,
    NoValue,
    UnexpectedReturningStatement,
    ExpectedBoolean,
    AssignmentOpInvalid(Dynamic, AssignmentOp, Dynamic),
    NotAFunction(Dynamic),
    MismatchedNativeSignature(Vec<&'static str>, Vec<Dynamic>),
    MismatchedSignature(Vec<String>, Vec<Dynamic>),
}

#[derive(Debug)]
pub struct ExecutionError {
    location: Location,
    error_type: ExecutionErrorType,
}

impl ExecutionError {
    pub fn new(location: Location, error_type: ExecutionErrorType) -> ExecutionError {
        ExecutionError {
            location,
            error_type,
        }
    }
}

struct VariableState {
    value: Dynamic,
    mutability: VariableMutability,
}

impl VariableState {
    pub fn new(value: Dynamic, mutability: VariableMutability) -> VariableState {
        VariableState { value, mutability }
    }
}

#[derive(Debug, Clone, Copy)]
struct Location {
    line: u32,
    column: u16,
    length: u16,
}

impl Default for Location {
    fn default() -> Self {
        Self {
            line: 1,
            column: 0,
            length: 0,
        }
    }
}

impl Location {
    pub fn new(line: usize, column: usize, length: usize) -> Location {
        // Convert these to smaller types, maintains the nice interface
        Location {
            line: line as u32,
            column: column as u16,
            length: length as u16,
        }
    }
}
