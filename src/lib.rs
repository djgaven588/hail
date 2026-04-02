mod benching;
mod machine;
mod parser;
mod scanner;
mod walker;

use std::{collections::HashMap, fs};

use crate::{
    parser::{AssignmentOp, BinaryOp, Parser, UnaryOp, VariableMutability},
    scanner::Scanner,
};

pub fn run(script_name: String) {
    benching::bench(&script_name);
    return;

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
        let result = walker::ExecutionContext::new().run(stmts);
        println!("Result: {result:?}");

        let vm = machine::Vm::new(stmts);
        println!("Running machine...");
        let result = vm.run();
        println!("Result: {result:?}");
    } else {
        println!("Failed to get statements.");
    }

    /*
     */
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
        location: Location,
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
                    location,
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

    pub fn mut_variable(
        &mut self,
        name: &str,
        location: Location,
        modify: impl FnOnce(&mut VariableState, Location) -> Result<(), ExecutionError>,
    ) -> Result<(), ExecutionError> {
        let len = self.scopes.len();
        for i in 0..len {
            // Reverse loop
            let i = len - i - 1;

            // Search for variable we can mutate
            if let Some(found) = self.scopes[i].variables.get_mut(name) {
                if found.mutability != VariableMutability::Mutable {
                    return Err(ExecutionError::new(
                        location,
                        ExecutionErrorType::VariableImmutable(name.to_string()),
                    ));
                }

                return modify(found, location);
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
        let len = self.scopes.len();
        for i in 0..len {
            let i = len - i - 1;
            if let Some(found) = self.scopes[i].variables.get_mut(name) {
                return Ok(found.value.clone());
            }
        }

        Err(ExecutionError::new(
            location,
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

impl Dynamic {
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
    RedefinedConstant,
    ExpectedBoolean,
    AssignmentOpInvalid(Dynamic, AssignmentOp, Dynamic),
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
