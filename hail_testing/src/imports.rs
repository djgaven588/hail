use std::{collections::HashMap, sync::Arc};

use hail::{
    Constructor, Executor, Instructor, MemoryImportResolver, Parser, Program, ProgramValue, Scanner,
};

/// Helper to compile a script with imports resolved from the provided map
fn compile_with_imports(
    source: String,
    imports: &HashMap<String, String>,
) -> Result<Program, String> {
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
    let resolver = Arc::new(MemoryImportResolver {
        script_files: imports.clone(),
    });
    let constructor = Constructor::new(module.clone(), resolver.clone(), None);
    let _astmts = constructor
        .generate(stmts)
        .map_err(|e| format!("Constructor errors: {:?}", e))?;

    let instructor = Instructor::new(module, resolver, None);
    Ok(instructor.generate(_astmts, source, vec![]))
}

/// Helper to compile and run a script with imports, returning typed result
fn run_with_imports<T: ProgramValue>(
    source: String,
    imports: &HashMap<String, String>,
) -> Result<T, String> {
    let program = compile_with_imports(source, imports)?;
    let executor = Executor::default();
    executor
        .run_with_return::<T>(&program)
        .map_err(|e| format!("Execution error: {:?}", e))
}

/// Basic import test simple function with one parameter
#[test]
fn import_single_param_function() {
    let mut imports = HashMap::new();
    imports.insert(
        "math".to_string(),
        r#"
fn add_one(n: i64) -> i64 {
    n + 1
}
"#
        .to_string(),
    );

    let result = run_with_imports::<i64>(
        r#"
import "math" as math;
math::add_one(5)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");
    assert_eq!(result, 6);
}

/// Recursive Fibonacci defined in an imported module at multiple input sizes
#[test]
fn import_recursive_fib() {
    let mut imports = HashMap::new();
    imports.insert(
        "fib".to_string(),
        r#"
fn fib(n: i64) -> i64 {
    if n <= 1 {
        n
    } else {
        fib(n - 1) + fib(n - 2)
    }
}
"#
        .to_string(),
    );

    // Test base cases
    let result = run_with_imports::<i64>(
        r#"
import "fib" as fib;
fib::fib(0)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");
    assert_eq!(result, 0);

    let result = run_with_imports::<i64>(
        r#"
import "fib" as fib;
fib::fib(1)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");
    assert_eq!(result, 1);

    // Test recursive case
    let result = run_with_imports::<i64>(
        r#"
import "fib" as fib;
fib::fib(5)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");
    assert_eq!(result, 5);

    // Test larger recursive case
    let result = run_with_imports::<i64>(
        r#"
import "fib" as fib;
fib::fib(10)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");
    assert_eq!(result, 55);

    // Test even larger case to ensure no stack or frame issues
    let result = run_with_imports::<i64>(
        r#"
import "fib" as fib;
fib::fib(20)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");
    assert_eq!(result, 6765);
}

/// Imported function with two and three parameters, verifying argument passing works correctly
#[test]
fn import_multiple_params() {
    let mut imports = HashMap::new();
    imports.insert(
        "math".to_string(),
        r#"
fn multiply(a: i64, b: i64) -> i64 {
    a * b
}

fn add_three(x: i64, y: i64, z: i64) -> i64 {
    x + y + z
}
"#
        .to_string(),
    );

    let result = run_with_imports::<i64>(
        r#"
import "math" as math;
math::multiply(3, 4)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");
    assert_eq!(result, 12);

    let result = run_with_imports::<i64>(
        r#"
import "math" as math;
math::add_three(1, 2, 3)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");
    assert_eq!(result, 6);
}

/// Imported functions that accept and return `String`, including a loop based repeat helper
#[test]
fn import_string_operations() {
    let mut imports = HashMap::new();
    imports.insert(
        "greet".to_string(),
        r#"
fn greet(name: String) -> String {
    "Hello, " + name
}

fn repeat(str: String, times: i64) -> String {
    let mut result = "";
    let mut i = 0;
    while i < times {
        result = result + str;
        i = i + 1;
    }
    result
}
"#
        .to_string(),
    );

    let result = run_with_imports::<String>(
        r#"
import "greet" as greet;
greet::greet("World")
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");
    assert_eq!(result, "Hello, World".to_string());

    let result = run_with_imports::<String>(
        r#"
import "greet" as greet;
greet::repeat("ab", 3)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");
    assert_eq!(result, "ababab".to_string());
}

/// Imported functions with no parameters, including one that returns a module constant
#[test]
fn import_zero_params() {
    let mut imports = HashMap::new();
    imports.insert(
        "constants".to_string(),
        r#"
const PI = 3.14;

fn get_answer() -> i64 {
    42
}

fn get_pi() -> f64 {
    PI
}
"#
        .to_string(),
    );

    let result = run_with_imports::<i64>(
        r#"
import "constants" as constants;
constants::get_answer()
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");
    assert_eq!(result, 42);

    let result = run_with_imports::<f64>(
        r#"
import "constants" as constants;
constants::get_pi()
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");
    assert!((result - 3.14).abs() < 0.01);
}

/// Three level cross module call chain (A imports B, B imports C) verifying the full resolution path
#[test]
fn import_nested_cross_module_calls() {
    let mut imports = HashMap::new();

    // Module C: base function
    imports.insert(
        "c".to_string(),
        r#"
fn double(n: i64) -> i64 {
    n * 2
}
"#
        .to_string(),
    );

    // Module B: calls c::double
    imports.insert(
        "b".to_string(),
        r#"
import "c" as c;

fn triple(n: i64) -> i64 {
    c::double(n) + n
}
"#
        .to_string(),
    );

    // Module A: calls b::triple which calls c::double
    imports.insert(
        "a".to_string(),
        r#"
import "b" as b;

fn main() -> i64 {
    b::triple(5)
}
"#
        .to_string(),
    );

    let result = run_with_imports::<i64>(
        r#"
import "a" as a;
a::main()
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    // double(5)=10, triple(5)=10+5=15
    assert_eq!(result, 15);
}

/// Same imported function invoked twice with different arguments, results stored in locals and combined
#[test]
fn import_multiple_calls_same_function() {
    let mut imports = HashMap::new();
    imports.insert(
        "math".to_string(),
        r#"
fn square(n: i64) -> i64 {
    n * n
}
"#
        .to_string(),
    );

    let result = run_with_imports::<i64>(
        r#"
import "math" as math;
let x = math::square(3);
let y = math::square(4);
x + y
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    // 9 + 16 = 25
    assert_eq!(result, 25);
}

/// Caller side while loop that accumulates results from repeated calls to an imported recursive function
#[test]
fn import_with_loop() {
    let mut imports = HashMap::new();
    imports.insert(
        "fib".to_string(),
        r#"
fn fib(n: i64) -> i64 {
    if n <= 1 {
        n
    } else {
        fib(n - 1) + fib(n - 2)
    }
}
"#
        .to_string(),
    );

    // Test that loop with imported recursive function works
    let result = run_with_imports::<i64>(
        r#"
import "fib" as fib;
let mut sum = 0;
let mut i = 0;
while i < 5 {
    sum = sum + fib::fib(i);
    i = i + 1;
}
sum
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    // fib(0)+fib(1)+fib(2)+fib(3)+fib(4) = 0+1+1+2+3 = 7
    assert_eq!(result, 7);
}
/// Error case: calling an imported function with too few arguments fails at construction time
#[test]
fn import_wrong_param_count() {
    let mut imports = HashMap::new();
    imports.insert(
        "math".to_string(),
        r#"
fn add(a: i64, b: i64) -> i64 {
    a + b
}
"#
        .to_string(),
    );

    let result = compile_with_imports(
        r#"
import "math" as math;
math::add(5)
"#
        .to_string(),
        &imports,
    );

    assert!(result.is_err(), "Should fail with wrong param count");
}

/// Error case: passing a `String` where `i64` is expected fails at construction time
#[test]
fn import_type_mismatch() {
    let mut imports = HashMap::new();
    imports.insert(
        "math".to_string(),
        r#"
fn add(a: i64, b: i64) -> i64 {
    a + b
}
"#
        .to_string(),
    );

    let result = compile_with_imports(
        r#"
import "math" as math;
math::add("hello", 5)
"#
        .to_string(),
        &imports,
    );

    assert!(result.is_err(), "Should fail with type mismatch");
}

/// Error case: referencing a function name that does not exist within an imported module
#[test]
fn import_nonexistent_function() {
    let mut imports = HashMap::new();
    imports.insert(
        "math".to_string(),
        r#"
fn add(a: i64, b: i64) -> i64 {
    a + b
}
"#
        .to_string(),
    );

    let result = compile_with_imports(
        r#"
import "math" as math;
math::subtract(5, 3)
"#
        .to_string(),
        &imports,
    );

    assert!(result.is_err(), "Should fail with nonexistent function");
}

/// Error case: importing a module alias that has no corresponding entry in the resolver map
#[test]
fn import_nonexistent_namespace() {
    let mut imports = HashMap::new();
    imports.insert(
        "math".to_string(),
        r#"
fn add(a: i64, b: i64) -> i64 {
    a + b
}
"#
        .to_string(),
    );

    let result = compile_with_imports(
        r#"
import "nonexistent" as ns;
ns::add(5, 3)
"#
        .to_string(),
        &imports,
    );

    assert!(result.is_err(), "Should fail with nonexistent namespace");
}

/// Module A defines recursive factorial; module B imports it and adds its own recursion to sum factorials
#[test]
fn import_complex_nested_recursion() {
    let mut imports = HashMap::new();

    // Module A: factorial
    imports.insert(
        "a".to_string(),
        r#"
fn factorial(n: i64) -> i64 {
    if n <= 1 {
        1
    } else {
        n * factorial(n - 1)
    }
}
"#
        .to_string(),
    );

    // Module B: Uses A to compute sum of factorials
    imports.insert(
        "b".to_string(),
        r#"
import "a" as a;

fn sum_factorial(n: i64) -> i64 {
    if n <= 0 {
        0
    } else {
        a::factorial(n) + sum_factorial(n - 1)
    }
}
"#
        .to_string(),
    );

    let result = run_with_imports::<i64>(
        r#"
import "b" as b;
b::sum_factorial(5)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    // 1! + 2! + 3! + 4! + 5! = 1 + 2 + 6 + 24 + 120 = 153
    assert_eq!(result, 153);
}

/// Caller-side block introduces a shadowed `x` while the imported function still receives the outer binding
#[test]
fn import_with_variable_shadowing() {
    let mut imports = HashMap::new();
    imports.insert(
        "math".to_string(),
        r#"
fn double(n: i64) -> i64 {
    n * 2
}
"#
        .to_string(),
    );

    let result = run_with_imports::<i64>(
        r#"
import "math" as math;
let x = 5;
let y = math::double(x);
{
    let x = 10;
    let z = math::double(x);
    y + z
}
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    // double(5)=10, double(10)=20, 10+20=30
    assert_eq!(result, 30);
}

/// Imported predicate and conjunction helper exercised with both true and false argument combinations
#[test]
fn import_boolean_logic() {
    let mut imports = HashMap::new();
    imports.insert(
        "logic".to_string(),
        r#"
fn is_positive(n: i64) -> bool {
    n > 0
}

fn and(a: bool, b: bool) -> bool {
    if a {
        if b {
            true
        } else {
            false
        }
    } else {
        false
    }
}
"#
        .to_string(),
    );

    let result = run_with_imports::<bool>(
        r#"
import "logic" as logic;
let a = logic::is_positive(5);
let b = logic::is_positive(-3);
logic::and(a, b)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    assert_eq!(result, false);

    let result = run_with_imports::<bool>(
        r#"
import "logic" as logic;
let a = logic::is_positive(5);
let b = logic::is_positive(3);
logic::and(a, b)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    assert_eq!(result, true);
}

/// Imported max function exercising both branches of an if else, tested with two orderings
#[test]
fn import_with_if_else() {
    let mut imports = HashMap::new();
    imports.insert(
        "math".to_string(),
        r#"
fn max(a: i64, b: i64) -> i64 {
    if a > b {
        a
    } else {
        b
    }
}
"#
        .to_string(),
    );

    let result = run_with_imports::<i64>(
        r#"
import "math" as math;
math::max(10, 20)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    assert_eq!(result, 20);

    let result = run_with_imports::<i64>(
        r#"
import "math" as math;
math::max(30, 15)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    assert_eq!(result, 30);
}

/// Imported function using ``let mut`` locals and a while loop to compute a running sum
#[test]
fn import_with_mutable_variables() {
    let mut imports = HashMap::new();
    imports.insert(
        "math".to_string(),
        r#"
fn sum_to_n(n: i64) -> i64 {
    let mut sum = 0;
    let mut i = 1;
    while i <= n {
        sum = sum + i;
        i = i + 1;
    }
    sum
}
"#
        .to_string(),
    );

    let result = run_with_imports::<i64>(
        r#"
import "math" as math;
math::sum_to_n(5)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    // 1+2+3+4+5 = 15
    assert_eq!(result, 15);

    let result = run_with_imports::<i64>(
        r#"
import "math" as math;
math::sum_to_n(10)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    // 1+2+...+10 = 55
    assert_eq!(result, 55);
}

/// Two separate modules imported into one caller, functions from each used in sequence
#[test]
fn import_multiple_modules() {
    let mut imports = HashMap::new();

    imports.insert(
        "math".to_string(),
        r#"
fn add(a: i64, b: i64) -> i64 {
    a + b
}

fn multiply(a: i64, b: i64) -> i64 {
    a * b
}
"#
        .to_string(),
    );

    imports.insert(
        "logic".to_string(),
        r#"
fn is_zero(n: i64) -> bool {
    n == 0
}
"#
        .to_string(),
    );

    let result = run_with_imports::<i64>(
        r#"
import "math" as math;
import "logic" as logic;
let a = math::add(5, 3);
let b = math::multiply(a, 2);
b
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    // (5+3)*2 = 16
    assert_eq!(result, 16);
}

/// Imported function that references its own module-level `const` values inside a loop body
#[test]
fn import_with_constants() {
    let mut imports = HashMap::new();
    imports.insert(
        "constants".to_string(),
        r#"
const BASE = 2;
const EXPONENT = 8;

fn power_of_two() -> i64 {
    let mut result = 1;
    let mut i = 0;
    while i < EXPONENT {
        result = result * BASE;
        i = i + 1;
    }
    result
}
"#
        .to_string(),
    );

    let result = run_with_imports::<i64>(
        r#"
import "constants" as constants;
constants::power_of_two()
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    // 2^8 = 256
    assert_eq!(result, 256);
}

/// Three level cross module call chain using single expression functions to verify resolution overhead is minimal
#[test]
fn import_deeply_nested_calls() {
    let mut imports = HashMap::new();

    imports.insert(
        "a".to_string(),
        r#"
fn step1(n: i64) -> i64 { n + 1 }
"#
        .to_string(),
    );

    imports.insert(
        "b".to_string(),
        r#"
import "a" as a;
fn step2(n: i64) -> i64 { a::step1(n) * 2 }
"#
        .to_string(),
    );

    imports.insert(
        "c".to_string(),
        r#"
import "b" as b;
fn step3(n: i64) -> i64 { b::step2(n) - 3 }
"#
        .to_string(),
    );

    let result = run_with_imports::<i64>(
        r#"
import "c" as c;
c::step3(5)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    // step1(5)=6, step2(5)=12, step3(5)=9
    assert_eq!(result, 9);
}

/// Imported `abs` and `clamp` helpers exercising `<`, `>`, and nested if-else branching logic
#[test]
fn import_comparison_operators() {
    let mut imports = HashMap::new();
    imports.insert(
        "math".to_string(),
        r#"
fn abs(n: i64) -> i64 {
    if n < 0 {
        0 - n
    } else {
        n
    }
}

fn clamp(value: i64, min: i64, max: i64) -> i64 {
    if value < min {
        min
    } else {
        if value > max {
            max
        } else {
            value
        }
    }
}
"#
        .to_string(),
    );

    let result = run_with_imports::<i64>(
        r#"
import "math" as math;
math::abs(-5)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    assert_eq!(result, 5);

    let result = run_with_imports::<i64>(
        r#"
import "math" as math;
math::clamp(10, 0, 100)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    assert_eq!(result, 10);

    let result = run_with_imports::<i64>(
        r#"
import "math" as math;
math::clamp(-5, 0, 100)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    assert_eq!(result, 0);

    let result = run_with_imports::<i64>(
        r#"
import "math" as math;
math::clamp(150, 0, 100)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    assert_eq!(result, 100);

    let result = run_with_imports::<i64>(
        r#"
import "math" as math;
math::abs(5)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    assert_eq!(result, 5);
}

/// Imported function using ``+=`` for both the accumulator and loop counter inside a while loop
#[test]
fn import_compound_assignment() {
    let mut imports = HashMap::new();
    imports.insert(
        "math".to_string(),
        r#"
fn accumulate(n: i64) -> i64 {
    let mut sum = 0;
    let mut i = 1;
    while i <= n {
        sum += i;
        i += 1;
    }
    sum
}
"#
        .to_string(),
    );

    let result = run_with_imports::<i64>(
        r#"
import "math" as math;
math::accumulate(5)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    // 1+2+3+4+5 = 15
    assert_eq!(result, 15);
}

/// Import with early return pattern (if-else returns)
#[test]
fn import_early_return_pattern() {
    let mut imports = HashMap::new();
    imports.insert(
        "math".to_string(),
        r#"
fn safe_divide(numerator: i64, denominator: i64) -> i64 {
    if denominator == 0 {
        0
    } else {
        numerator / denominator
    }
}
"#
        .to_string(),
    );

    let result = run_with_imports::<i64>(
        r#"
import "math" as math;
math::safe_divide(10, 3)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    assert_eq!(result, 3);

    let result = run_with_imports::<i64>(
        r#"
import "math" as math;
math::safe_divide(10, 0)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    assert_eq!(result, 0);
}

/// Import with multiple statements in body (not just expression)
#[test]
fn import_multiple_statements() {
    let mut imports = HashMap::new();
    imports.insert(
        "math".to_string(),
        r#"
fn compute(a: i64, b: i64) -> i64 {
    let temp1 = a + b;
    let temp2 = a - b;
    temp1 * temp2
}
"#
        .to_string(),
    );

    let result = run_with_imports::<i64>(
        r#"
import "math" as math;
math::compute(10, 3)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    // (10+3)*(10-3) = 13*7 = 91
    assert_eq!(result, 91);
}

/// Import with nested blocks in imported function
#[test]
fn import_nested_blocks() {
    let mut imports = HashMap::new();
    imports.insert(
        "math".to_string(),
        r#"
fn compute_with_block(n: i64) -> i64 {
    {
        let temp = n * 2;
        {
            let inner = temp + 1;
            inner
        }
    }
}
"#
        .to_string(),
    );

    let result = run_with_imports::<i64>(
        r#"
import "math" as math;
math::compute_with_block(5)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    // (5*2)+1 = 11
    assert_eq!(result, 11);
}

/// Import with shadowing in imported function scope
#[test]
fn import_shadowing_in_imported_function() {
    let mut imports = HashMap::new();
    imports.insert(
        "math".to_string(),
        r#"
fn compute(x: i64) -> i64 {
    let y = x * 2;
    {
        let x = 10;
        y + x
    }
}
"#
        .to_string(),
    );

    let result = run_with_imports::<i64>(
        r#"
import "math" as math;
math::compute(5)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    // y=10, x=10 (shadowed), 10+10=20
    assert_eq!(result, 20);
}

/// Import with boolean negation in imported function
#[test]
fn import_boolean_negation() {
    let mut imports = HashMap::new();
    imports.insert(
        "logic".to_string(),
        r#"
fn is_negative(n: i64) -> bool {
    n < 0
}

fn not_positive(n: i64) -> bool {
    if is_negative(n) {
        true
    } else {
        false
    }
}
"#
        .to_string(),
    );

    let result = run_with_imports::<bool>(
        r#"
import "logic" as logic;
logic::not_positive(-5)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    assert_eq!(result, true);

    let result = run_with_imports::<bool>(
        r#"
import "logic" as logic;
logic::not_positive(5)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    assert_eq!(result, false);
}

/// Import with complex expression in return
#[test]
fn import_complex_expression_return() {
    let mut imports = HashMap::new();
    imports.insert(
        "math".to_string(),
        r#"
fn compute(a: i64, b: i64) -> i64 {
    (a + b) * (a - b) + a * b / 2
}
"#
        .to_string(),
    );

    let result = run_with_imports::<i64>(
        r#"
import "math" as math;
math::compute(10, 5)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    // (10+5)*(10-5) + 10*5/2 = 15*5 + 50/2 = 75 + 25 = 100
    assert_eq!(result, 100);
}

/// Import with while loop and break-like pattern (using if to exit)
#[test]
fn import_while_with_condition() {
    let mut imports = HashMap::new();
    imports.insert(
        "math".to_string(),
        r#"
fn find_first_even(max: i64) -> i64 {
    let mut n = 1;
    while true {
        if n % 2 == 0 {
            return n;
        }
        n = n + 1;
    }

    n
}
"#
        .to_string(),
    );

    let result = run_with_imports::<i64>(
        r#"
import "math" as math;
math::find_first_even(10)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    assert_eq!(result, 2);
}

/// Import with multiple recursive calls in same expression (like fib)
#[test]
fn import_multiple_recursive_calls() {
    let mut imports = HashMap::new();
    imports.insert(
        "math".to_string(),
        r#"
fn double_fib(n: i64) -> i64 {
    if n <= 1 {
        n
    } else {
        fib(n - 1) + fib(n - 2) + fib(n - 1) + fib(n - 2)
    }
}

fn fib(n: i64) -> i64 {
    if n <= 1 {
        n
    } else {
        fib(n - 1) + fib(n - 2)
    }
}
"#
        .to_string(),
    );

    let result = run_with_imports::<i64>(
        r#"
import "math" as math;
math::double_fib(5)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    //fib(4)+fib(3)+fib(4)+fib(3) = 2*(fib(4)+fib(3)) = 2*fib(5) = 2*5 = 10
    assert_eq!(result, 10);
}

/// Import exercising mutual recursion between a recursive module local function and an imported helper
#[test]
fn import_mixed_recursion_and_import() {
    let mut imports = HashMap::new();

    // Module A: simple increment
    imports.insert(
        "a".to_string(),
        r#"
fn inc(n: i64) -> i64 {
    n + 1
}
"#
        .to_string(),
    );

    // Module B: recursive with import call
    imports.insert(
        "b".to_string(),
        r#"
import "a" as a;

fn count_down(n: i64) -> i64 {
    if n <= 0 {
        0
    } else {
        a::inc(count_down(n - 1))
    }
}
"#
        .to_string(),
    );

    let result = run_with_imports::<i64>(
        r#"
import "b" as b;
b::count_down(5)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    // count_down(0)=0, inc(0)=1, inc(1)=2, ..., inc(4)=5
    assert_eq!(result, 5);
}

/// Import with large recursion depth to test stack management
#[test]
fn import_large_recursion_depth() {
    let mut imports = HashMap::new();
    imports.insert(
        "math".to_string(),
        r#"
fn sum_to_n(n: i64) -> i64 {
    if n <= 0 {
        0
    } else {
        n + sum_to_n(n - 1)
    }
}
"#
        .to_string(),
    );

    let result = run_with_imports::<i64>(
        r#"
import "math" as math;
math::sum_to_n(100)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    // 1+2+...+100 = 5050
    assert_eq!(result, 5050);
}

/// Import verifying that a function body can reference its own module-level `const` by name
#[test]
fn import_constant_access() {
    let mut imports = HashMap::new();
    imports.insert(
        "constants".to_string(),
        r#"
const MULTIPLIER = 10;

fn scale(n: i64) -> i64 {
    n * MULTIPLIER
}
"#
        .to_string(),
    );

    let result = run_with_imports::<i64>(
        r#"
import "constants" as constants;
constants::scale(5)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    assert_eq!(result, 50);
}

/// Import verifying all four basic arithmetic operators (+, -, *, /) can coexist in one imported function body
#[test]
fn import_all_arithmetic_operators() {
    let mut imports = HashMap::new();
    imports.insert(
        "math".to_string(),
        r#"
fn compute(a: i64, b: i64) -> i64 {
    let add_result = a + b;
    let sub_result = a - b;
    let mul_result = a * b;
    let div_result = a / b;
    add_result + sub_result + mul_result + div_result
}
"#
        .to_string(),
    );

    let result = run_with_imports::<i64>(
        r#"
import "math" as math;
math::compute(10, 3)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    // (10+3) + (10-3) + (10*3) + (10/3) = 13 + 7 + 30 + 3 = 53
    assert_eq!(result, 53);
}

/// Import combining nested boolean conditions in one imported function with a second function that branches on the result
#[test]
fn import_comparison_and_logic() {
    let mut imports = HashMap::new();
    imports.insert(
        "logic".to_string(),
        r#"
fn complex_condition(a: i64, b: i64) -> bool {
    if a > 0 {
        if b < 10 {
            true
        } else {
            false
        }
    } else {
        false
    }
}

fn get_result(flag: bool, value: i64) -> i64 {
    if flag {
        value * 2
    } else {
        value + 10
    }
}
"#
        .to_string(),
    );

    let result = run_with_imports::<i64>(
        r#"
import "logic" as logic;
let flag = logic::complex_condition(5, 3);
logic::get_result(flag, 7)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    // complex_condition(5,3)=true, get_result(true,7)=14
    assert_eq!(result, 14);

    let result = run_with_imports::<i64>(
        r#"
import "logic" as logic;
let flag = logic::complex_condition(-5, 3);
logic::get_result(flag, 7)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    // complex_condition(-5,3)=false, get_result(false,7)=17
    assert_eq!(result, 17);
}

/// Import verifying a function can reach its final expression through different branches of nested if-else chains (positive / negative / zero)
#[test]
fn import_multiple_return_points() {
    let mut imports = HashMap::new();
    imports.insert(
        "math".to_string(),
        r#"
fn classify(n: i64) -> String {
    if n > 0 {
        "positive"
    } else {
        if n < 0 {
            "negative"
        } else {
            "zero"
        }
    }
}
"#
        .to_string(),
    );

    let result = run_with_imports::<String>(
        r#"
import "math" as math;
math::classify(5)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    assert_eq!(result, "positive".to_string());

    let result = run_with_imports::<String>(
        r#"
import "math" as math;
math::classify(-5)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    assert_eq!(result, "negative".to_string());

    let result = run_with_imports::<String>(
        r#"
import "math" as math;
math::classify(0)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    assert_eq!(result, "zero".to_string());
}

/// Import verifying that a parameter can be shadowed inside deeply nested blocks while outer bindings remain accessible
#[test]
fn import_variable_shadowing_across_scopes() {
    let mut imports = HashMap::new();
    imports.insert(
        "math".to_string(),
        r#"
fn compute(x: i64) -> i64 {
    let y = x * 2;
    {
        let z = y + 1;
        {
            let x = 100;
            z + x
        }
    }
}
"#
        .to_string(),
    );

    let result = run_with_imports::<i64>(
        r#"
import "math" as math;
math::compute(5)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    // y=10, z=11, x=100 (shadowed), 11+100=111
    assert_eq!(result, 111);
}

/// Import of an iterative factorial (while loop based), exercising multiple input sizes including edge cases 0 and 1
#[test]
fn import_iterative_computation() {
    let mut imports = HashMap::new();
    imports.insert(
        "math".to_string(),
        r#"
fn factorial(n: i64) -> i64 {
    let mut result = 1;
    let mut i = 2;
    while i <= n {
        result = result * i;
        i = i + 1;
    }
    result
}
"#
        .to_string(),
    );

    let result = run_with_imports::<i64>(
        r#"
import "math" as math;
math::factorial(5)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    // 1*2*3*4*5 = 120
    assert_eq!(result, 120);

    let result = run_with_imports::<i64>(
        r#"
import "math" as math;
math::factorial(0)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    assert_eq!(result, 1);

    let result = run_with_imports::<i64>(
        r#"
import "math" as math;
math::factorial(1)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    assert_eq!(result, 1);

    let result = run_with_imports::<i64>(
        r#"
import "math" as math;
math::factorial(10)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    // 3628800
    assert_eq!(result, 3628800);
}

/// Import of a binary exponentiation function using mutable locals and a module level ``const``
#[test]
fn import_variable_mutability_patterns() {
    let mut imports = HashMap::new();
    imports.insert(
        "math".to_string(),
        r#"
const BASE = 2;

fn compute_power(exponent: i64) -> i64 {
    let mut result = 1;
    let mut base = BASE;
    let mut exp = exponent;

    while exp > 0 {
        if exp % 2 == 1 {
            result = result * base;
        }
        base = base * base;
        exp = exp / 2;
    }

    result
}
"#
        .to_string(),
    );

    let result = run_with_imports::<i64>(
        r#"
import "math" as math;
math::compute_power(10)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    // 2^10 = 1024
    assert_eq!(result, 1024);
}

/// Import of a grade classification function using five levels of nested if else to map score ranges to letter grades
#[test]
fn import_nested_if_else_chains() {
    let mut imports = HashMap::new();
    imports.insert(
        "math".to_string(),
        r#"
fn grade(score: i64) -> String {
    if score >= 90 {
        "A"
    } else {
        if score >= 80 {
            "B"
        } else {
            if score >= 70 {
                "C"
            } else {
                if score >= 60 {
                    "D"
                } else {
                    "F"
                }
            }
        }
    }
}
"#
        .to_string(),
    );

    let result = run_with_imports::<String>(
        r#"
import "math" as math;
math::grade(95)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    assert_eq!(result, "A".to_string());

    let result = run_with_imports::<String>(
        r#"
import "math" as math;
math::grade(85)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    assert_eq!(result, "B".to_string());

    let result = run_with_imports::<String>(
        r#"
import "math" as math;
math::grade(75)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    assert_eq!(result, "C".to_string());

    let result = run_with_imports::<String>(
        r#"
import "math" as math;
math::grade(65)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    assert_eq!(result, "D".to_string());

    let result = run_with_imports::<String>(
        r#"
import "math" as math;
math::grade(50)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    assert_eq!(result, "F".to_string());
}

/// Import verifying ``+=`` and ``i += 1`` work inside a while loop body (sum of squares)
#[test]
fn import_compound_assignment_in_loop() {
    let mut imports = HashMap::new();
    imports.insert(
        "math".to_string(),
        r#"
fn sum_of_squares(n: i64) -> i64 {
    let mut sum = 0;
    let mut i = 1;
    while i <= n {
        sum += i * i;
        i += 1;
    }
    sum
}
"#
        .to_string(),
    );

    let result = run_with_imports::<i64>(
        r#"
import "math" as math;
math::sum_of_squares(5)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    // 1^2 + 2^2 + 3^2 + 4^2 + 5^2 = 1+4+9+16+25 = 55
    assert_eq!(result, 55);
}

/// Import combining a range check predicate with a second function that branches on the boolean result
#[test]
fn import_boolean_operators_in_condition() {
    let mut imports = HashMap::new();
    imports.insert(
        "logic".to_string(),
        r#"
fn is_in_range(value: i64, min: i64, max: i64) -> bool {
    if value >= min {
        if value <= max {
            true
        } else {
            false
        }
    } else {
        false
    }
}

fn check_and_return(flag: bool, a: i64, b: i64) -> i64 {
    if flag {
        a + b
    } else {
        a - b
    }
}
"#
        .to_string(),
    );

    let result = run_with_imports::<i64>(
        r#"
import "logic" as logic;
let in_range = logic::is_in_range(5, 1, 10);
logic::check_and_return(in_range, 20, 3)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    // is_in_range(5,1,10)=true, check_and_return(true,20,3)=23
    assert_eq!(result, 23);

    let result = run_with_imports::<i64>(
        r#"
import "logic" as logic;
let in_range = logic::is_in_range(15, 1, 10);
logic::check_and_return(in_range, 20, 3)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    // is_in_range(15,1,10)=false, check_and_return(false,20,3)=17
    assert_eq!(result, 17);
}

/// Import of a function that builds a repeated-string via a while loop, exercising zero and multi-character cases
#[test]
fn import_string_operations_in_loop() {
    let mut imports = HashMap::new();
    imports.insert(
        "string".to_string(),
        r#"
fn build_string(count: i64) -> String {
    let mut result = "";
    let mut i = 0;
    while i < count {
        result = result + "a";
        i = i + 1;
    }
    result
}
"#
        .to_string(),
    );

    let result = run_with_imports::<String>(
        r#"
import "string" as string;
string::build_string(5)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    assert_eq!(result, "aaaaa".to_string());

    let result = run_with_imports::<String>(
        r#"
import "string" as string;
string::build_string(0)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    assert_eq!(result, "".to_string());

    let result = run_with_imports::<String>(
        r#"
import "string" as string;
string::build_string(10)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    assert_eq!(result, "aaaaaaaaaa".to_string());
}

/// Import exercising every comparison operator (>, <, ==, !=, >=, <=) across three helper functions
#[test]
fn import_all_comparison_operators() {
    let mut imports = HashMap::new();
    imports.insert(
        "math".to_string(),
        r#"
fn compare(a: i64, b: i64) -> i64 {
    if a > b {
        1
    } else {
        if a < b {
            -1
        } else {
            if a >= b {
                2
            } else {
                if a <= b {
                    3
                } else {
                    0
                }
            }
        }
    }
}

fn equals(a: i64, b: i64) -> bool {
    if a == b {
        true
    } else {
        false
    }
}

fn not_equals(a: i64, b: i64) -> bool {
    if a != b {
        true
    } else {
        false
    }
}
"#
        .to_string(),
    );

    let result = run_with_imports::<i64>(
        r#"
import "math" as math;
math::compare(5, 3)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    assert_eq!(result, 1);

    let result = run_with_imports::<i64>(
        r#"
import "math" as math;
math::compare(3, 5)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    assert_eq!(result, -1);

    let result = run_with_imports::<i64>(
        r#"
import "math" as math;
math::compare(5, 5)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    assert_eq!(result, 2);

    let result = run_with_imports::<bool>(
        r#"
import "math" as math;
math::equals(5, 5)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    assert_eq!(result, true);

    let result = run_with_imports::<bool>(
        r#"
import "math" as math;
math::equals(5, 3)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    assert_eq!(result, false);

    let result = run_with_imports::<bool>(
        r#"
import "math" as math;
math::not_equals(5, 3)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    assert_eq!(result, true);

    let result = run_with_imports::<bool>(
        r#"
import "math" as math;
math::not_equals(5, 5)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    assert_eq!(result, false);
}

/// Import verifying three levels of nested `{ }` blocks can each introduce a new local variable while capturing outer ones
#[test]
fn import_nested_blocks_different_scopes() {
    let mut imports = HashMap::new();
    imports.insert(
        "math".to_string(),
        r#"
fn compute(x: i64) -> i64 {
    let outer = x * 2;
    {
        let inner = outer + 1;
        {
            let deep = inner * 3;
            deep
        }
    }
}
"#
        .to_string(),
    );

    let result = run_with_imports::<i64>(
        r#"
import "math" as math;
math::compute(5)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    // outer=10, inner=11, deep=33
    assert_eq!(result, 33);
}

/// Import of a function that finds the largest ^2 exponent via a while loop whose condition involves multiplication
#[test]
fn import_while_with_complex_condition() {
    let mut imports = HashMap::new();
    imports.insert(
        "math".to_string(),
        r#"
fn find_max_power_of_two(limit: i64) -> i64 {
    let mut power = 1;
    let mut exp = 0;
    while power * 2 <= limit {
        power = power * 2;
        exp = exp + 1;
    }
    exp
}
"#
        .to_string(),
    );

    let result = run_with_imports::<i64>(
        r#"
import "math" as math;
math::find_max_power_of_two(100)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    // 2^6=64 <= 100, 2^7=128 > 100, so exp=6
    assert_eq!(result, 6);

    let result = run_with_imports::<i64>(
        r#"
import "math" as math;
math::find_max_power_of_two(1024)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    // 2^10=1024 <= 1024, so exp=10
    assert_eq!(result, 10);

    let result = run_with_imports::<i64>(
        r#"
import "math" as math;
math::find_max_power_of_two(1)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    // 2^0=1, 2^1=2 > 1, so exp=0
    assert_eq!(result, 0);

    let result = run_with_imports::<i64>(
        r#"
import "math" as math;
math::find_max_power_of_two(0)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    assert_eq!(result, 0);
}

/// Import exercising every basic type (i64, f64, bool, String) as parameter and return across four separate functions
#[test]
fn import_all_basic_types() {
    let mut imports = HashMap::new();
    imports.insert(
        "types".to_string(),
        r#"
fn process_int(n: i64) -> i64 { n * 2 }

fn process_float(f: f64) -> f64 { f * 2.0 }

fn process_bool(b: bool) -> bool { if b { true } else { false } }

fn process_string(s: String) -> String { "processed: " + s }
"#
        .to_string(),
    );

    let result = run_with_imports::<i64>(
        r#"
import "types" as types;
types::process_int(5)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    assert_eq!(result, 10);

    let result = run_with_imports::<f64>(
        r#"
import "types" as types;
types::process_float(3.5)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    assert!((result - 7.0).abs() < 0.01);

    let result = run_with_imports::<bool>(
        r#"
import "types" as types;
types::process_bool(true)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    assert_eq!(result, true);

    let result = run_with_imports::<String>(
        r#"
import "types" as types;
types::process_string("hello")
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    assert_eq!(result, "processed: hello".to_string());
}

/// Make sure the stack doesn't explode

#[test]
fn import_deep_call_stack() {
    let mut imports = HashMap::new();
    imports.insert(
        "math".to_string(),
        r#"
fn ackermann(m: i64, n: i64) -> i64 {
    if m == 0 {
        n + 1
    } else {
        let value =  if n == 0 {
            ackermann(m - 1, 1)
        } else {
            ackermann(m - 1, ackermann(m, n - 1))
        };

		value
    }
}
"#
        .to_string(),
    );

    // Ackermann(1, 2) = 4
    let result = run_with_imports::<i64>(
        r#"
import "math" as math;
math::ackermann(1, 2)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    assert_eq!(result, 4);

    // Ackermann(2, 2) = 7
    let result = run_with_imports::<i64>(
        r#"
import "math" as math;
math::ackermann(2, 2)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    assert_eq!(result, 7);

    // Ackermann(0, 5) = 6
    let result = run_with_imports::<i64>(
        r#"
import "math" as math;
math::ackermann(0, 5)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    assert_eq!(result, 6);

    // Ackermann(1, 0) = 2
    let result = run_with_imports::<i64>(
        r#"
import "math" as math;
math::ackermann(1, 0)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    assert_eq!(result, 2);
}

/// Import verifying a non-recursive function can invoke the same imported recursive function multiple times within one expression
#[test]
fn import_nested_recursive_calls_in_expression() {
    let mut imports = HashMap::new();
    imports.insert(
        "math".to_string(),
        r#"
fn fib(n: i64) -> i64 {
    if n <= 1 {
        n
    } else {
        fib(n - 1) + fib(n - 2)
    }
}

fn double_fib_sum(a: i64, b: i64) -> i64 {
    fib(a) * 2 + fib(b) * 2
}
"#
        .to_string(),
    );

    let result = run_with_imports::<i64>(
        r#"
import "math" as math;
math::double_fib_sum(5, 3)
"#
        .to_string(),
        &imports,
    )
    .expect("Should execute");

    // fib(5)=5, fib(3)=2, so 5*2 + 2*2 = 10+4 = 14
    assert_eq!(result, 14);
}
