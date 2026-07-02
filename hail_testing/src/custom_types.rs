use crate::expected_parse;
use hail::FunctionInfo;
use hail::FunctionKind;
use hail::ModuleError;
use hail::Program;
use hail::ValueType;
use hail::{Constructor, Executor, Instructor, MemoryImportResolver, Module, Parser, Scanner};
use std::sync::Arc;

// Example CustomType
#[derive(Clone, Debug)]
struct Counter {
    value: i64,
}

impl hail::memory_usage::MemoryUsage for Counter {
    fn memory_usage(&self) -> usize {
        unimplemented!()
    }
}

impl Counter {
    fn new(n: i64) -> Self {
        Counter { value: n }
    }
    fn get_value(&self) -> i64 {
        self.value
    }
    fn is_twenty(&self) -> bool {
        self.value == 20
    }
    fn add(&mut self, n: i64) -> i64 {
        self.value += n;
        self.value
    }
}

/// Build a module that knows about `Counter`.
///
/// Registered functions:
/// - `make_counter(n: i64) -> Counter`, free constructor
///
/// Registered properties on `Counter`:
/// - `.get_value  -> i64`
/// - `.is_twenty  -> bool`
///
/// Registered methods on `Counter`:
/// - `.add(n: i64) -> i64`
/// - `.add_and_multiply(n: i64, multiple: i64) -> i64`
///
fn counter_module() -> Module {
    let mut module = Module::default();

    module
        .register_type_named::<Counter>("Counter")
        .expect("Should register");

    module
        .register_global("LEET", 1337 as i64)
        .expect("Should register.");

    // Free function: make_counter(n) -> Counter
    module
        .register_function(FunctionInfo {
            name: "make_counter".to_string(),
            param_types: vec![ValueType::Int],
            param_names: vec!["value".to_owned()],
            doc_comments: vec![],
            return_type: Some(ValueType::of::<Counter>()),
            kind: FunctionKind::Free(|_exec, mut args| {
                let n = *args
                    .pop()
                    .expect("Should have arg")
                    .into_any()
                    .downcast::<i64>()
                    .expect("Should downcast");
                Ok(Some(Box::new(Counter::new(n))))
            }),
        })
        .expect("Should register");

    // Property: counter.get_value -> i64
    module
        .register_method(
            ValueType::of::<Counter>(),
            FunctionInfo {
                name: "get_value".to_string(),
                param_types: vec![ValueType::of::<Counter>()],
                return_type: Some(ValueType::Int),
                param_names: vec!["counter".to_owned()],
                doc_comments: vec![],
                kind: FunctionKind::Property(|_exec, callee| {
                    let c = callee
                        .as_any()
                        .downcast_ref::<Counter>()
                        .expect("Value should be typeof(Counter)");
                    Ok(Some(Box::new(c.get_value())))
                }),
            },
        )
        .expect("Should register");

    // Property: counter.is_twenty -> bool
    module
        .register_method(
            ValueType::of::<Counter>(),
            FunctionInfo {
                name: "is_twenty".to_string(),
                param_types: vec![ValueType::of::<Counter>()],
                return_type: Some(ValueType::Bool),
                param_names: vec!["counter".to_owned()],
                doc_comments: vec![],
                kind: FunctionKind::Property(|_exec, caller| {
                    let c = caller
                        .as_any()
                        .downcast_ref::<Counter>()
                        .expect("Value should be typeof(Counter)");
                    Ok(Some(Box::new(c.is_twenty())))
                }),
            },
        )
        .expect("Should register");

    // Method: counter.add(n) -> i64
    module
        .register_method(
            ValueType::of::<Counter>(),
            FunctionInfo {
                name: "add".to_string(),
                param_types: vec![ValueType::of::<Counter>(), ValueType::Int],
                return_type: Some(ValueType::Int),
                param_names: vec!["counter".to_owned(), "value".to_string()],
                doc_comments: vec![],
                kind: FunctionKind::MethodMut(|_exec, caller, mut args| {
                    // The executor reverses args so that pop() yields the
                    // receiver first, then each argument left-to-right.
                    let c = caller
                        .as_any_mut()
                        .downcast_mut::<Counter>()
                        .expect("Should downcast");
                    let n = *args
                        .pop()
                        .expect("Should have arg")
                        .into_any()
                        .downcast::<i64>()
                        .expect("Should downcast");
                    Ok(Some(Box::new(c.add(n))))
                }),
            },
        )
        .expect("Should register");

    // Method: counter.add_and_multiply(n, multiple) -> i64
    module
        .register_method(
            ValueType::of::<Counter>(),
            FunctionInfo {
                name: "add_and_multiply".to_string(),
                param_types: vec![ValueType::of::<Counter>(), ValueType::Int, ValueType::Int],
                return_type: Some(ValueType::Int),
                param_names: vec![
                    "counter".to_owned(),
                    "added".to_string(),
                    "multiplied".to_string(),
                ],
                doc_comments: vec![],
                kind: FunctionKind::MethodMut(|_exec, caller, mut args| {
                    // The executor reverses args so that pop() yields the
                    // receiver first, then each argument left-to-right.
                    let c = caller
                        .as_any_mut()
                        .downcast_mut::<Counter>()
                        .expect("Should downcast");
                    let n = *args
                        .pop()
                        .expect("Should have arg")
                        .into_any()
                        .downcast::<i64>()
                        .expect("Should downcast");
                    let multiple = *args
                        .pop()
                        .expect("Should have arg")
                        .into_any()
                        .downcast::<i64>()
                        .expect("Should downcast");
                    c.add(n);
                    c.value *= multiple;
                    Ok(Some(Box::new(c.value)))
                }),
            },
        )
        .expect("Should register");

    module
}

/// Compile a script against an arbitrary module, panicking on any error.
fn compile_with_module(source: String, module: Arc<Module>) -> Program {
    let stmts = expected_parse(source.clone());
    let resolver = Arc::new(MemoryImportResolver::default());
    let constructor = Constructor::new(module.clone(), resolver.clone(), None);
    let astmts = constructor
        .generate(&stmts)
        .expect("Should be able to construct");
    let instructor = Instructor::new(module, resolver, None);
    instructor.generate(astmts, source, vec![])
}

/// Attempt to compile a script against an arbitrary module, returning `Err` if any pipeline stage fails.
fn try_compile_with_module(source: String, module: Arc<Module>) -> Result<Program, String> {
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
    let resolver = Arc::new(MemoryImportResolver::default());
    let constructor = Constructor::new(module.clone(), resolver.clone(), None);
    let astmts = constructor
        .generate(stmts)
        .map_err(|_| format!("Constructor errors for: {}", source))?;
    let instructor = Instructor::new(module, resolver, None);
    Ok(instructor.generate(astmts, source, vec![]))
}

// ValueType::of / register_type unit tests

/// `ValueType::of::<T>()` should be reflexively equal and differ across
/// distinct types.
#[test]
fn value_type_of_equality() {
    assert_eq!(ValueType::of::<i64>(), ValueType::of::<i64>());
    assert_eq!(ValueType::of::<Counter>(), ValueType::of::<Counter>());
    assert_ne!(ValueType::of::<i64>(), ValueType::of::<f64>());
    assert_ne!(ValueType::of::<Counter>(), ValueType::of::<i64>());
}

/// `Module::register_type_named` must make the type resolvable by its short name and its full path.
#[test]
fn register_type_resolves_name() {
    let mut module = Module::default();
    let vt = module
        .register_type_named::<Counter>("Counter")
        .expect("Should register");

    // The returned ValueType must round-trip through of::<Counter>()
    assert_eq!(vt, ValueType::of::<Counter>());

    // Short name lookup ("Counter")
    assert_eq!(module.resolve_type("Counter"), Some(vt.clone()));

    // Full path lookup (e.g. "hail::testing::tests::Counter")
    assert_eq!(
        module.resolve_type(std::any::type_name::<Counter>()),
        Some(vt),
    );
}

#[test]
fn register_type_twice() {
    let mut module = Module::default();
    module
        .register_type_named::<Counter>("Counter")
        .expect("Should register");

    assert_eq!(
        module.register_type_named::<Counter>("Counter"),
        Err(ModuleError::DuplicateType(
            ValueType::of::<Counter>(),
            ValueType::of::<Counter>()
        ))
    );
}

#[test]
fn register_different_types() {
    #[derive(Clone)]
    struct Other;

    impl hail::memory_usage::MemoryUsage for Other {
        fn memory_usage(&self) -> usize {
            unimplemented!()
        }
    }

    let mut module = Module::default();
    let vt_counter = module
        .register_type_named::<Counter>("Counter")
        .expect("Should register");
    let vt_other = module
        .register_type_named::<Other>("Other")
        .expect("Should register");
    assert_ne!(vt_counter, vt_other);
}

#[test]
fn custom_type_property_bool_true() {
    let module = counter_module();
    let program = compile_with_module("make_counter(20).is_twenty".to_owned(), Arc::new(module));
    let executor = Executor::default();
    let result = executor
        .run_with_return::<bool>(&program)
        .expect("Should execute");
    assert!(result);
}

/// `make_counter(10).is_twenty` should evaluate to `false`.
#[test]
fn custom_type_property_bool_false() {
    let module = counter_module();
    let program = compile_with_module("make_counter(10).is_twenty".to_owned(), Arc::new(module));
    let executor = Executor::default();
    let result = executor
        .run_with_return::<bool>(&program)
        .expect("Should execute");
    assert!(!result);
}

/// `make_counter(42).get_value` should return `42`.
#[test]
fn custom_type_property_int() {
    let module = counter_module();
    let program = compile_with_module("make_counter(42).get_value".to_owned(), Arc::new(module));
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 42);
}

/// Custom type stored in a `let` binding and then accessed via property.
#[test]
fn custom_type_stored_in_variable() {
    let module = counter_module();
    let program = compile_with_module(
        "let c = make_counter(7); c.get_value".to_owned(),
        Arc::new(module),
    );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 7);
}

/// Method call with an extra argument: `make_counter(10).add(5)` == 15.
#[test]
fn custom_type_method_with_arg() {
    let module = counter_module();
    let program = compile_with_module("make_counter(10).add(5)".to_owned(), Arc::new(module));
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 15);
}

/// The counter retrieved from the variable should still work after being
/// used as the receiver in a method call.
#[test]
fn custom_type_variable_method_call() {
    let module = counter_module();
    let program = compile_with_module(
        "let mut c = make_counter(3); c.add(100)".to_owned(),
        Arc::new(module),
    );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 103);
}

/// Calling an unknown property on a custom type should be invalid.
#[test]
fn custom_type_unknown_property_rejected() {
    let module = counter_module();
    let result =
        try_compile_with_module("make_counter(1).nonexistent".to_owned(), Arc::new(module));
    assert!(result.is_err(), "Should reject unknown property");
}

/// Calling a Counter property on a non Counter value is invalid as i64 doesn't have .is_twenty in this module.
#[test]
fn custom_type_wrong_receiver_rejected() {
    let module = counter_module();
    let result = try_compile_with_module("42.is_twenty".to_owned(), Arc::new(module));
    assert!(
        result.is_err(),
        "Should reject method on wrong receiver type"
    );
}

/// A method call on a computed expression (not a variable) should not attempt writeback.
///
/// The result is left on the stack.
#[test]
fn callee_mutation_stack_source() {
    let module = counter_module();
    // 5 + 10, not writing back variable
    let program = compile_with_module("make_counter(5).add(10)".to_owned(), Arc::new(module));
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 15);
}

/// A property that reads the callee should work when the callee is a variable, and any mutation should be written back.
#[test]
fn callee_property_variable_slot() {
    let module = counter_module();
    // 20 == 20
    let program = compile_with_module(
        "let c = make_counter(20); c.is_twenty".to_owned(),
        Arc::new(module),
    );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<bool>(&program)
        .expect("Should execute");
    assert!(result);
}

/// Sequential calls on the same variable should each write back the mutated callee, so subsequent calls see the updated state.
#[test]
fn callee_mutation_sequential_calls() {
    let module = counter_module();
    // 0 + 1 + 2 + 3 = 6
    let program = compile_with_module(
        "let mut c = make_counter(0); c.add(1); c.add(2); c.add(3)".to_owned(),
        Arc::new(module),
    );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 6);
}

/// A method call on a variable followed by reading the variable should
/// see the mutated value.
#[test]
fn callee_mutation_variable_used_after() {
    let module = counter_module();
    // 10 + 5 = 15
    let program = compile_with_module(
        "let mut c = make_counter(10); c.add(5); c.get_value".to_owned(),
        Arc::new(module),
    );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 15);
}

/// A method call on a variable followed by another method call with the
/// same receiver should see the mutated state.
#[test]
fn callee_mutation_chained_on_same_variable() {
    let module = Arc::new(counter_module());
    // 0 + 10 != 20
    let program = compile_with_module(
        "let mut c = make_counter(0); c.add(10); c.is_twenty".to_owned(),
        module.clone(),
    );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<bool>(&program)
        .expect("Should execute");
    assert!(!result);

    // 0 + 10 + 10 == 20
    let program2 = compile_with_module(
        "let mut c = make_counter(0); c.add(10); c.add(10); c.is_twenty".to_owned(),
        module,
    );
    let executor2 = Executor::default();
    let result2 = executor2
        .run_with_return::<bool>(&program2)
        .expect("Should execute");
    assert!(result2);
}

/// A free function call should not interfere with callee mutation logic.
#[test]
fn callee_mutation_free_function_still_works() {
    let module = counter_module();

    let program = compile_with_module("make_counter(7).get_value".to_owned(), Arc::new(module));
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 7);
}

/// A free function call should not interfere with callee mutation logic.
#[test]
fn multi_argument_call() {
    let module = counter_module();

    let program = compile_with_module(
        "make_counter(7).add_and_multiply(5, 2)".to_owned(),
        Arc::new(module),
    );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 24);
}

/// Basic global retrieval
#[test]
fn global_retrieve() {
    let module = counter_module();

    let program = compile_with_module("LEET".to_owned(), Arc::new(module));
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 1337);
}

/// Global used in expression
#[test]
fn global_retrieve_expression() {
    let mut module = hail_std::std().expect("Should register");
    module.merge(counter_module()).expect("Should register");

    let program = compile_with_module("LEET * 2".to_owned(), Arc::new(module));
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 2674);
}
