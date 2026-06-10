mod benching;
pub use benching::*;
mod constructor;
pub use constructor::*;
mod executor;
pub use executor::*;
mod instructor;
pub use instructor::*;
mod module;
pub use module::*;
mod parser;
pub use parser::*;
mod scanner;
pub use scanner::*;
mod visualize;
pub use visualize::*;

use std::{fmt::Debug, fs, sync::Arc};

pub fn run(script_name: String, is_bench: bool) {
    todo!("Bring back as part of ``hail_testing`` or some sort of thing");

    if is_bench {
        benching::bench(&script_name);
        return;
    }
    /*
    let path = "./".to_string() + script_name.as_str() + ".hail";
    println!("Running: {path}");
    let source = fs::read_to_string(&path).expect("Script should be at location.");

    let mut scanner = Scanner::new(source.clone());
    scanner.scan();

    println!("Tokens:");
    for token in scanner.get().unwrap() {
        println!("Token: {token:?}");
    }

    if !scanner.errors().is_empty() {
        println!(
            "{}",
            visualize::format_diagnostics(
                &source,
                Some(path.as_str()),
                scanner.errors(),
                scanner.get().unwrap()
            )
        );
        return;
    }

    let mut parser = Parser::new(scanner.get().unwrap());
    parser.parse();

    if !parser.errors().is_empty() {
        print!(
            "{}",
            visualize::format_diagnostics(
                &source,
                Some(path.as_str()),
                parser.errors(),
                scanner.get().unwrap()
            )
        );
        return;
    }

    if let Some(stmts) = parser.get() {
        let module = Arc::new(Module::std());
        let mut resolver = MemoryImportResolver::default();
        resolver.script_files.insert(
            "fib".to_owned(),
            fs::read_to_string("./scripts/fib.hail").expect("REMOVE THIS DAMNED THING"),
        );
        resolver.script_files.insert(
            "basic".to_owned(),
            fs::read_to_string("./scripts/basic.hail").expect("REMOVE THIS DAMNED THING"),
        );
        let resolver = Arc::new(resolver);

        let astmts =
            match Constructor::new(module.clone(), resolver.clone(), Some(path.to_string()))
                .generate(stmts)
            {
                Ok(val) => val,
                Err(err) => {
                    print!(
                        "{}",
                        visualize::format_diagnostic(
                            &source,
                            Some(path.as_str()),
                            &err,
                            scanner.get().unwrap()
                        )
                    );
                    return;
                }
            };
        println!("\nConstruct: \n{astmts:?}\n");

        let program = Instructor::new(module, resolver, None).generate(astmts, source);
        println!("\nInstructions: \n{program:?}\n");

        let result = Executor::default().run_with_return::<i64>(&program);
        println!("Result: {result:?}");
    } else {
        println!("Failed to get statements.");
    }*/
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Location {
    pub line: u32,
    pub column: u16,
    pub length: u16,
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
