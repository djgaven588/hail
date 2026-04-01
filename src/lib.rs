mod machine;
mod parser;
mod scanner;
mod walker;

use std::{
    collections::HashMap,
    fs,
    time::{Duration, Instant},
};

use crate::{
    parser::{Parser, VariableMutability},
    scanner::{Scanner, Token},
};

pub fn run(source: String) {
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
        println!("2...");
        std::thread::sleep(Duration::from_secs(1));
        println!("1...");
        std::thread::sleep(Duration::from_secs(1));

        let vm = machine::Vm::new(stmts);
        println!("Running walker...");
        for _ in 0..10 {
            let start = Instant::now();
            let result = walker::ExecutionContext::new().run(stmts);
            let end = start.elapsed();
            //println!("Result: {result:?}");
            println!("Walker took {:.5} seconds", end.as_secs_f64());
        }

        println!("2...");
        std::thread::sleep(Duration::from_secs(1));
        println!("1...");
        std::thread::sleep(Duration::from_secs(1));
        let vm = machine::Vm::new(stmts);
        println!("Running machine...");
        for _ in 0..10 {
            let start = Instant::now();
            let result = vm.run();
            let end = start.elapsed();
            //println!("Result: {result:?}");
            println!("Machine took {:.5} seconds", end.as_secs_f64());
        }
    } else {
        println!("Failed to get statements.");
    }

    println!("Benching Rhai");
    println!("2...");
    std::thread::sleep(Duration::from_secs(1));
    println!("1...");
    std::thread::sleep(Duration::from_secs(1));
    let engine = rhai::Engine::new();
    let ast = engine
        .compile(fs::read_to_string("./loop_bench.rhai").unwrap())
        .unwrap();
    let ast = engine.optimize_ast(&rhai::Scope::new(), ast, rhai::OptimizationLevel::Full);

    for _ in 0..10 {
        let start = Instant::now();
        engine.run_ast(&ast).unwrap();
        let end = start.elapsed();
        println!("Rhai took {:.5} seconds", end.as_secs_f64());
    }
}

#[derive(Default)]
pub struct Scope {
    variables: HashMap<String, VariableState>,
}

#[derive(Default)]
struct Scoper {
    scopes: Vec<Scope>,
}

impl Scoper {
    pub fn new(scope: Option<Scope>) -> Scoper {
        Scoper {
            scopes: vec![scope.unwrap_or_else(|| Scope::default())],
        }
    }

    pub fn push(&mut self, scope: Option<Scope>) {
        self.scopes.push(scope.unwrap_or(Scope::default()));
    }

    pub fn pop(&mut self) {
        self.scopes.pop();
    }

    pub fn define_variable(
        &mut self,
        name: &str,
        mutability: VariableMutability,
        value: Dynamic,
        location: &Location,
    ) -> Result<(), ExecutionError> {
        // If it's a constant, make sure we're not bypassing the fact it's a constant by redefining it
        // Constants are still *scoped*, this is more of a "enforce good behavior" that can be removed if needed
        let len = self.scopes.len();
        for i in 0..len {
            let i = len - i - 1;
            if let Some(found) = self.scopes[i].variables.get_mut(name)
                && found.mutability == VariableMutability::Constant
            {
                return Err(ExecutionError::new(
                    location.clone(),
                    ExecutionErrorType::RedefinedConstant,
                ));
            }
        }

        self.scopes
            .last_mut()
            .expect("A scope should always exist.")
            .variables
            .insert(name.to_string(), VariableState::new(value, mutability));
        Ok(())
    }

    pub fn assign_variable(
        &mut self,
        name: &str,
        value: Dynamic,
        location: &Location,
    ) -> Result<(), ExecutionError> {
        let len = self.scopes.len();
        for i in 0..len {
            let i = len - i - 1;
            if let Some(found) = self.scopes[i].variables.get_mut(name) {
                if found.mutability != VariableMutability::Mutable {
                    return Err(ExecutionError::new(
                        location.clone(),
                        ExecutionErrorType::VariableImmutable(name.to_string()),
                    ));
                }
                found.value = value;
                return Ok(());
            }
        }

        Err(ExecutionError::new(
            location.clone(),
            ExecutionErrorType::VariableUndefined(name.to_string()),
        ))
    }

    pub fn get_variable(
        &mut self,
        name: &str,
        location: &Location,
    ) -> Result<Dynamic, ExecutionError> {
        let len = self.scopes.len();
        for i in 0..len {
            let i = len - i - 1;
            if let Some(found) = self.scopes[i].variables.get_mut(name) {
                return Ok(found.value.clone());
            }
        }

        Err(ExecutionError::new(
            location.clone(),
            ExecutionErrorType::VariableUndefined(name.to_string()),
        ))
    }
}

#[derive(Debug, Clone)]
pub enum Dynamic {
    Bool(bool),
    Integer(i64),
    Float(f64),
    String(String),
    Nil,
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
    InvalidOperator(Dynamic, Token, Dynamic),
    InvalidUnaryOperator(Token, Dynamic),
    DivideByZero,
    NoValue,
    UnexpectedReturningStatement,
    RedefinedConstant,
    ExpectedBoolean,
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

#[derive(Debug, Clone)]
struct Location {
    line: usize,
    column: usize,
    length: usize,
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
        Location {
            line,
            column,
            length,
        }
    }
}
