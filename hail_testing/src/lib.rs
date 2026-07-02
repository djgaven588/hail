#[cfg(test)]
mod basic;

#[cfg(test)]
mod functions;

#[cfg(test)]
mod native_calls;

#[cfg(test)]
mod custom_types;

#[cfg(test)]
mod imports;

#[cfg(test)]
mod arrays;

#[cfg(test)]
mod options;

#[cfg(test)]
mod setters;

#[cfg(test)]
mod for_loops;

#[cfg(test)]
mod control_flow;

#[cfg(test)]
mod generics;

#[cfg(test)]
mod tracking;

use hail::Constructor;
use hail::Instructor;
use hail::MemoryImportResolver;
use hail::Parser;
use hail::Scanner;
use hail::Stmt;
use std::sync::Arc;

use hail::Program;

pub fn try_compile(source: String) -> Result<Program, String> {
    let mut scanner = Scanner::new(source.clone());
    scanner.scan();
    if !scanner.errors().is_empty() {
        return Err(format!("Scanner errors for: {}", source));
    }

    let stmts = scanner.get().ok_or_else(|| "No tokens".to_string())?;
    let mut parser = Parser::new(stmts);
    parser.parse();
    if !parser.errors().is_empty() {
        return Err(format!("Parse errors for: {}", source));
    }

    let stmts = parser.get().ok_or_else(|| "No stmts".to_string())?;
    let module = Arc::new(hail_std::std().expect("Should register"));
    let resolver = Arc::new(MemoryImportResolver::default());

    let constructor = Constructor::new(module.clone(), resolver.clone(), None);
    let _astmts = constructor
        .generate(stmts)
        .map_err(|_| format!("Constructor errors for: {}", source))?;

    let instructor = Instructor::new(module, resolver, None);
    Ok(instructor.generate(_astmts, source, vec![]))
}

pub fn expected_compile(source: String) -> Program {
    let stmts = expected_parse(source.clone());

    let module = Arc::new(hail_std::std().expect("Should register"));
    let resolver = Arc::new(MemoryImportResolver::default());

    // Type checking and such
    let constructor = Constructor::new(module.clone(), resolver.clone(), None);
    let astmts = constructor
        .generate(&stmts)
        .expect("Should be able to construct");

    // Turn to machine code
    let instructor = Instructor::new(module, resolver, None);
    let program = instructor.generate(astmts, source, vec![]);
    program
}

pub fn expected_parse(source: String) -> Vec<Stmt> {
    let mut scanner = Scanner::new(source);
    scanner.scan();

    // We should have no errors
    assert_eq!(scanner.errors(), &[]);

    // Parse the scanned tokens
    let mut parser = Parser::new(scanner.get().unwrap());
    parser.parse();

    // We should have no errors
    assert_eq!(parser.errors(), &[]);

    // Type checking and such
    parser.get().expect("Should have stmts").to_vec()
}
