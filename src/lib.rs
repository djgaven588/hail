#![feature(downcast_unchecked)]

mod benching;
mod constructor;
mod executor;
mod instructor;
mod module;
mod parser;
mod scanner;
mod testing;

use std::{any::TypeId, fmt::Debug, fs};

use crate::{
    constructor::Constructor,
    executor::Executor,
    instructor::{Instructor, ProgramValue},
    module::Module,
    parser::{Parser, Stmt, VariableMutability},
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
        let module = Module::std();
        let astmts = Constructor::new(module.clone()).generate(stmts).unwrap();
        println!("\nConstruct: \n{astmts:?}\n");

        let program = Instructor::new(module).generate(&astmts);
        println!("\nInstructions: \n{program:?}\n");

        let result = Executor::default().run_with_return::<i64>(&program);
        println!("Result: {result:?}");
    } else {
        println!("Failed to get statements.");
    }
}

#[derive(Debug)]
pub struct NativeFuncInfo {
    pub name: String,
    pub signature: Vec<TypeId>,
    pub param_info: Vec<&'static str>,
    pub call: fn(&mut Executor, Vec<Box<dyn ProgramValue>>) -> Result<Box<dyn ProgramValue>, ()>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Location {
    line: u32,
    column: u16,
    length: u16,
}

impl Default for Location {
    fn default() -> Self {
        Self {
            line: 1,
            column: 1,
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
