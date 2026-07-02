use crate::expected_parse;
use hail::FunctionInfo;
use hail::FunctionKind;
use hail::{Constructor, Executor, Instructor, MemoryImportResolver, Module, Parser, Scanner};
use std::sync::Arc;

// == Counter Type for Setter Testing ==
#[derive(Clone)]
struct Counter {
    value: i64,
}

impl hail::memory_usage::MemoryUsage for Counter {
    fn memory_usage(&self) -> usize {
        unimplemented!()
    }
}

impl Counter {
    fn new() -> Self {
        Counter { value: 0 }
    }
    fn get_value(&self) -> i64 {
        self.value
    }
    fn set_value(&mut self, new_value: i64) {
        self.value = new_value;
    }
}

// == Point2D Type for Setter Testing ==
#[derive(Clone)]
struct Point2D {
    x: i64,
    y: i64,
}

impl hail::memory_usage::MemoryUsage for Point2D {
    fn memory_usage(&self) -> usize {
        unimplemented!()
    }
}

impl Point2D {
    fn new() -> Self {
        Point2D { x: 0, y: 0 }
    }
    fn get_x(&self) -> i64 {
        self.x
    }
    fn set_x(&mut self, new_value: i64) {
        self.x = new_value;
    }
    fn get_y(&self) -> i64 {
        self.y
    }
    fn set_y(&mut self, new_value: i64) {
        self.y = new_value;
    }
}

/// Build a module that knows about Counter and Point2D with setters.
fn setter_module() -> Arc<Module> {
    let mut module = Module::default();

    // Register types
    module
        .register_type_named::<Counter>("Counter")
        .expect("Should register");
    module
        .register_type_named::<Point2D>("Point2D")
        .expect("Should register");

    // Counter constructors and properties/setters
    module
        .register_function(FunctionInfo {
            name: "make_counter".to_string(),
            param_types: vec![],
            param_names: vec![],
            doc_comments: vec![],
            return_type: Some(hail::ValueType::of::<Counter>()),
            kind: FunctionKind::Free(|_exec, _args| Ok(Some(Box::new(Counter::new())))),
        })
        .expect("Should register");

    // Counter getter (property)
    module
        .register_method(
            hail::ValueType::of::<Counter>(),
            FunctionInfo {
                name: "value".to_string(),
                param_types: vec![hail::ValueType::of::<Counter>()],
                param_names: vec!["counter".to_string()],
                doc_comments: vec![],
                return_type: Some(hail::ValueType::Int),
                kind: FunctionKind::Property(|_exec, callee| {
                    let c = callee
                        .as_any()
                        .downcast_ref::<Counter>()
                        .expect("Should downcast to Counter");
                    Ok(Some(Box::new(c.get_value())))
                }),
            },
        )
        .expect("Should register");

    // Counter setter (setter named "value" so `counter.value = 10` works)
    module
        .register_method(
            hail::ValueType::of::<Counter>(),
            FunctionInfo {
                name: "value".to_string(),
                param_types: vec![hail::ValueType::of::<Counter>(), hail::ValueType::Int],
                param_names: vec!["counter".to_string(), "value".to_string()],
                doc_comments: vec![],
                return_type: None,
                kind: FunctionKind::Setter(|_exec, callee, arg| {
                    let c = callee
                        .as_any_mut()
                        .downcast_mut::<Counter>()
                        .expect("Should downcast to Counter");
                    let new_value = arg
                        .into_any()
                        .downcast::<i64>()
                        .expect("Should downcast value to i64");
                    c.set_value(*new_value);
                    Ok(None)
                }),
            },
        )
        .expect("Should register");

    // Point2D constructors and properties/setters
    module
        .register_function(FunctionInfo {
            name: "make_point".to_string(),
            param_types: vec![],
            param_names: vec![],
            doc_comments: vec![],
            return_type: Some(hail::ValueType::of::<Point2D>()),
            kind: FunctionKind::Free(|_exec, _args| Ok(Some(Box::new(Point2D::new())))),
        })
        .expect("Should register");

    // Point2D x getter (property)
    module
        .register_method(
            hail::ValueType::of::<Point2D>(),
            FunctionInfo {
                name: "x".to_string(),
                param_types: vec![hail::ValueType::of::<Point2D>()],
                param_names: vec!["point".to_string()],
                doc_comments: vec![],
                return_type: Some(hail::ValueType::Int),
                kind: FunctionKind::Property(|_exec, callee| {
                    let p = callee
                        .as_any()
                        .downcast_ref::<Point2D>()
                        .expect("Should downcast to Point2D");
                    Ok(Some(Box::new(p.get_x())))
                }),
            },
        )
        .expect("Should register");

    // Point2D x setter (setter named "x" so `point.x = 10` works)
    module
        .register_method(
            hail::ValueType::of::<Point2D>(),
            FunctionInfo {
                name: "x".to_string(),
                param_types: vec![hail::ValueType::of::<Point2D>(), hail::ValueType::Int],
                param_names: vec!["point".to_string(), "x".to_string()],
                doc_comments: vec![],
                return_type: None,
                kind: FunctionKind::Setter(|_exec, callee, arg| {
                    let p = callee
                        .as_any_mut()
                        .downcast_mut::<Point2D>()
                        .expect("Should downcast to Point2D");
                    let new_value = arg
                        .into_any()
                        .downcast::<i64>()
                        .expect("Should downcast value to i64");
                    p.set_x(*new_value);
                    Ok(None)
                }),
            },
        )
        .expect("Should register");

    // Point2D y getter (property)
    module
        .register_method(
            hail::ValueType::of::<Point2D>(),
            FunctionInfo {
                name: "y".to_string(),
                param_types: vec![hail::ValueType::of::<Point2D>()],
                param_names: vec!["counter".to_string()],
                doc_comments: vec![],
                return_type: Some(hail::ValueType::Int),
                kind: FunctionKind::Property(|_exec, callee| {
                    let p = callee
                        .as_any()
                        .downcast_ref::<Point2D>()
                        .expect("Should downcast to Point2D");
                    Ok(Some(Box::new(p.get_y())))
                }),
            },
        )
        .expect("Should register");

    // Point2D y setter (setter named "y" so `point.y = 10` works)
    module
        .register_method(
            hail::ValueType::of::<Point2D>(),
            FunctionInfo {
                name: "y".to_string(),
                param_types: vec![hail::ValueType::of::<Point2D>(), hail::ValueType::Int],
                param_names: vec!["counter".to_string(), "y".to_string()],
                doc_comments: vec![],
                return_type: None,
                kind: FunctionKind::Setter(|_exec, callee, arg| {
                    let p = callee
                        .as_any_mut()
                        .downcast_mut::<Point2D>()
                        .expect("Should downcast to Point2D");
                    let new_value = arg
                        .into_any()
                        .downcast::<i64>()
                        .expect("Should downcast value to i64");
                    p.set_y(*new_value);
                    Ok(None)
                }),
            },
        )
        .expect("Should register");

    Arc::new(module)
}

/// Compile a script against the setter module, panicking on any error.
fn compile_with_setters(source: String) -> hail::Program {
    let stmts = expected_parse(source.clone());
    let module = setter_module();
    let resolver = Arc::new(MemoryImportResolver::default());
    let constructor = Constructor::new(module.clone(), resolver.clone(), None);
    let astmts = constructor
        .generate(&stmts)
        .expect("Should be able to construct");
    let instructor = Instructor::new(module, resolver, None);
    instructor.generate(astmts, source, vec![])
}

/// Attempt to compile a script against the setter module, returning Err if any pipeline stage fails.
fn try_compile_with_setters(source: String) -> Result<hail::Program, String> {
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
    let module = setter_module();
    let resolver = Arc::new(MemoryImportResolver::default());
    let constructor = Constructor::new(module.clone(), resolver.clone(), None);
    let astmts = constructor
        .generate(stmts)
        .map_err(|_| format!("Constructor errors for: {}", source))?;
    let instructor = Instructor::new(module, resolver, None);
    Ok(instructor.generate(astmts, source, vec![]))
}

/// Test basic setter: obj.field = value (e.g., `point.x = 10`)
#[test]
fn test_basic_setter() {
    let program = compile_with_setters("let mut p = make_point(); p.x = 10; p.x".to_string());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 10);
}

/// Test setter with getter: counter.value = 10, value() -> 10
#[test]
fn test_setter_getter_counter() {
    let program =
        compile_with_setters("let mut c = make_counter(); c.value = 10; c.value".to_string());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 10);
}

/// Test setter not found: accessing non-existent field with assignment
#[test]
fn test_setter_not_found_error() {
    let result = try_compile_with_setters("let mut p = make_point(); p.z = 10".to_string());
    assert!(result.is_err(), "Setter not found should fail");
}

/// Test multiple setter calls in sequence: counter.value = 1, value; counter.value = 2, value; counter.value = 3, value -> 3
#[test]
fn test_multiple_setter_calls() {
    let program = compile_with_setters(
        "let mut c = make_counter(); c.value = 1; c.value; c.value = 2; c.value; c.value = 3; c.value".to_string(),
    );
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 3);
}

/// Test getter and setter together: counter.value = 42, value -> 42
#[test]
fn test_getter_setter_together() {
    let program =
        compile_with_setters("let mut c = make_counter(); c.value = 42; c.value".to_string());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 42);
}

/// Test Point2D x setter independently: point.x = 100, x -> 100
#[test]
fn test_point2d_x_setter() {
    let program = compile_with_setters("let mut p = make_point(); p.x = 100; p.x".to_string());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 100);
}

/// Test Point2D y setter independently: point.y = 200, y -> 200
#[test]
fn test_point2d_y_setter() {
    let program = compile_with_setters("let mut p = make_point(); p.y = 200; p.y".to_string());
    let executor = Executor::default();
    let result = executor
        .run_with_return::<i64>(&program)
        .expect("Should execute");
    assert_eq!(result, 200);
}

/// Require setters to operate on mutable variables
#[test]
fn test_setter_on_immutable_error() {
    let result = try_compile_with_setters("let p = make_point(); p.x = 10".to_string());
    assert!(result.is_err(), "Should produce VariableImmutable error");
}
