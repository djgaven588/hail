use std::{
    fs,
    time::{Duration, Instant},
};

use crate::{library, scanner::Scanner};

pub fn bench(script_name: &str) {
    println!("Running benchmark for script '{script_name}'");
    println!("2...");
    std::thread::sleep(Duration::from_secs(1));
    println!("1...");
    std::thread::sleep(Duration::from_secs(1));

    walker_bench(script_name);

    println!("2...");
    std::thread::sleep(Duration::from_secs(1));
    println!("1...");
    std::thread::sleep(Duration::from_secs(1));

    machine_bench(script_name);

    println!("2...");
    std::thread::sleep(Duration::from_secs(1));
    println!("1...");
    std::thread::sleep(Duration::from_secs(1));

    rhai_bench(script_name);
}

fn walker_bench(script_name: &str) {
    let mut scanner =
        Scanner::new(fs::read_to_string("./".to_string() + script_name + ".hail").unwrap());
    scanner.scan();
    let mut parser = crate::parser::Parser::new(scanner.get().unwrap());
    parser.parse();

    if !parser.errors().is_empty() {
        panic!("{:?}", parser.errors());
    }

    println!("Running walker...");
    for _ in 0..10 {
        let start = Instant::now();
        let result = crate::walker::ExecutionContext::new(library()).run(parser.get().unwrap());
        let end = start.elapsed();
        //println!("Result: {result:?}");
        println!("Walker took {:.5} seconds", end.as_secs_f64());

        if let Err(err) = result {
            panic!("{err:?}");
        }
    }
}

fn machine_bench(script_name: &str) {
    let mut scanner =
        Scanner::new(fs::read_to_string("./".to_string() + script_name + ".hail").unwrap());
    scanner.scan();
    let mut parser = crate::parser::Parser::new(scanner.get().unwrap());
    parser.parse();

    if !parser.errors().is_empty() {
        panic!("{:?}", parser.errors());
    }

    let vm = crate::machine::Vm::new(library(), parser.get().unwrap());
    println!("Running machine...");
    for _ in 0..10 {
        let start = Instant::now();
        let result = vm.run();
        let end = start.elapsed();
        //println!("Result: {result:?}");
        println!("Machine took {:.5} seconds", end.as_secs_f64());

        if let Err(err) = result {
            panic!("{err:?}");
        }
    }
}

fn rhai_bench(script_name: &str) {
    println!("Benching Rhai");
    let engine = rhai::Engine::new();
    let ast = engine
        .compile(fs::read_to_string("./".to_string() + script_name + ".rhai").unwrap())
        .unwrap();
    let ast = engine.optimize_ast(&rhai::Scope::new(), ast, rhai::OptimizationLevel::Full);

    for _ in 0..10 {
        let start = Instant::now();
        engine.run_ast(&ast).unwrap();
        let end = start.elapsed();
        println!("Rhai took {:.5} seconds", end.as_secs_f64());
    }
}
