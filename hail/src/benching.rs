use std::{
    fs,
    sync::Arc,
    time::{Duration, Instant},
};

use crate::{
    constructor::{Constructor, MemoryImportResolver},
    executor::Executor,
    instructor::Instructor,
    module::Module,
    scanner::Scanner,
};

pub fn bench(script_name: &str) {
    let iterations = 5;
    println!("Running benchmark for script '{script_name}' {iterations} times each.");
    println!("2...");
    std::thread::sleep(Duration::from_secs(1));
    println!("1...");
    std::thread::sleep(Duration::from_secs(1));

    let machine_time = machine_bench(script_name, iterations);

    println!("2...");
    std::thread::sleep(Duration::from_secs(1));
    println!("1...");
    std::thread::sleep(Duration::from_secs(1));

    let rhai_time = rhai_bench(script_name, iterations);
    println!(
        "Average Timings Over {iterations} Loops\n	Machine: {machine_time:.5} seconds\n	Rhai: {rhai_time:.5} seconds",
    );
}

fn machine_bench(script_name: &str, iterations: usize) -> f64 {
    todo!("Bring back, likely in hail_testing");
    /*
    let source_path = "./".to_string() + script_name + ".hail";
    let source_code = fs::read_to_string(source_path.clone()).unwrap();
    let mut scanner = Scanner::new(source_code.clone());
    scanner.scan();
    let mut parser = crate::parser::Parser::new(scanner.get().unwrap());
    parser.parse();

    if !parser.errors().is_empty() {
        panic!("{:?}", parser.errors());
    }

    let module = Arc::new(Module::std());
    let resolver = Arc::new(MemoryImportResolver::default());
    let astmts = match Constructor::new(module.clone(), resolver.clone(), Some(source_path))
        .generate(parser.get().unwrap())
    {
        Ok(val) => val,
        Err(err) => {
            panic!("Unexpected test failure: {err:?}");
        }
    };

    let program = Instructor::new(module.clone(), resolver, None).generate(astmts, source_code);
    println!("Running machine...");
    let mut average = 0.;

    for _ in 0..iterations {
        let start = Instant::now();

        let result = Executor::default().run(&program);
        println!("Result: {result:?}");
        let end = start.elapsed();
        //println!("Result: {result:?}");
        let time = end.as_secs_f64();
        average += time;
        println!("Machine took {:.5} seconds", time);

        if let Err(err) = result {
            panic!("{err:?}");
        }
    }

    average / iterations as f64*/
}

fn rhai_bench(script_name: &str, iterations: usize) -> f64 {
    let engine = rhai::Engine::new();
    let ast = engine
        .compile(fs::read_to_string("./".to_string() + script_name + ".rhai").unwrap())
        .unwrap();
    let ast = engine.optimize_ast(&rhai::Scope::new(), ast, rhai::OptimizationLevel::Full);

    println!("Benching Rhai");
    let mut average = 0.;

    for _ in 0..iterations {
        let start = Instant::now();
        engine.run_ast(&ast).unwrap();
        let end = start.elapsed();
        let time = end.as_secs_f64();
        average += time;
        println!("Rhai took {:.5} seconds", time);
    }

    average / iterations as f64
}
