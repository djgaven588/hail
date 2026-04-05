mod benching;
mod machine;
mod parser;
mod scanner;
mod walker;

use std::{
    any::{TypeId, type_name},
    fmt::Debug,
    fs,
    hash::{DefaultHasher, Hash, Hasher},
    sync::Arc,
};

use hashbrown::HashMap;

use crate::{
    machine::Vm,
    parser::{AssignmentOp, BinaryOp, Parser, Stmt, UnaryOp, VariableMutability},
    scanner::Scanner,
};

pub fn run(script_name: String, is_bench: bool) {
    if is_bench {
        benching::bench(&script_name);
        return;
    }

    let path = "./".to_string() + script_name.as_str() + ".hail";
    println!("Running: {path}");
    let source = fs::read_to_string(path).expect("Script should be at location.");

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
        let result = Vm::run(&vm);
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
    pub fn new(name: String) -> Arc<VariableName> {
        let mut hasher = DefaultHasher::new();
        name.hash(&mut hasher);
        VariableName {
            hash: hasher.finish(),
            name: name,
        }
        .into()
    }
}

struct Scoper {
    module: Arc<Module>,

    variables: Vec<(String, VariableState)>,
    frame_returns: Vec<usize>,

    frame_markers: Vec<usize>,
    scope_markers: Vec<usize>,
}

impl Scoper {
    /// Create an execution scope with the provided library and initial scope
    /// Generally a module will be provided, a scope is truly optional
    pub fn new(module: Arc<Module>) -> Scoper {
        Scoper {
            module,

            variables: Default::default(),
            frame_returns: Default::default(),

            frame_markers: Default::default(),
            scope_markers: Default::default(),
        }
    }

    /// Enter a new function frame
    pub fn enter(&mut self, return_address: usize) {
        // We don't need to scope as the frame marker is a scope boundary
        self.frame_markers.push(self.variables.len());
        self.frame_returns.push(return_address);
    }

    /// Exit a function frame
    pub fn exit(&mut self) -> Option<usize> {
        // Pop marker, may not exist.
        if let Some(marker) = self.frame_markers.pop() {
            // Get rid of everything that shouldn't be here anymore.
            self.variables.truncate(marker);

            while let Some(scope) = self.scope_markers.last()
                && *scope > marker
            {
                self.scope_markers.pop();
            }
        }

        // Return where we should go
        self.frame_returns.pop()
    }

    /// Push a scope level
    pub fn push(&mut self) {
        self.scope_markers.push(self.variables.len());
    }

    /// Pop a scope level
    pub fn pop(&mut self) {
        // Get rid of everything within this scope
        let marker = self.scope_markers.pop().expect("Should have scope marker");
        self.variables.truncate(marker);
    }

    pub fn define_variable(
        &mut self,
        name: &str,
        mutability: VariableMutability,
        value: Dynamic,
    ) -> Result<(), ExecutionError> {
        self.variables
            .push((name.to_string(), VariableState::new(value, mutability)));

        Ok(())
    }

    pub fn mut_variable(
        &mut self,
        name: &str,
        location: Location,
        modify: impl FnOnce(&mut VariableState, Location) -> Result<(), ExecutionError>,
    ) -> Result<(), ExecutionError> {
        let marker = *self.frame_markers.get(0).unwrap_or(&0);

        // Check current frame
        let len = self.variables.len();
        for i in (marker..len).rev() {
            // Search for variable we can mutate
            let variable = &mut self.variables[i];
            if variable.0 != name {
                continue;
            }

            if variable.1.mutability != VariableMutability::Mutable {
                return Err(ExecutionError::new(
                    location,
                    ExecutionErrorType::VariableImmutable(name.to_string()),
                ));
            }

            return modify(&mut variable.1, location);
        }

        // Check global frame
        if marker != 0 {
            let marker = *self.frame_markers.first().unwrap();

            for i in (0..marker).rev() {
                // Search for variable we can mutate
                let variable = &mut self.variables[i];
                if variable.0 != name {
                    continue;
                }

                if variable.1.mutability != VariableMutability::Mutable {
                    return Err(ExecutionError::new(
                        location,
                        ExecutionErrorType::VariableImmutable(name.to_string()),
                    ));
                }

                return modify(&mut variable.1, location);
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
        let marker = *self.frame_markers.get(0).unwrap_or(&0);

        // Check current frame
        let len = self.variables.len();
        for i in (marker..len).rev() {
            // Search for variable
            let variable = &mut self.variables[i];
            if variable.0 != name {
                continue;
            }

            return Ok(variable.1.value.clone());
        }

        // Check global frame
        if marker != 0 {
            let marker = *self.frame_markers.first().unwrap();

            for i in (0..marker).rev() {
                // Search for variable we can mutate
                let variable = &mut self.variables[i];
                if variable.0 != name {
                    continue;
                }

                return Ok(variable.1.value.clone());
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

#[derive(Debug)]
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
