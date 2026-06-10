use std::any::TypeId;

use hashbrown::HashMap;

use crate::{
    SyncProgramValue,
    constructor::ValueType,
    executor::{Executor, ExecutorError},
    instructor::ProgramValue,
    parser::{BinaryOp, UnaryOp},
};

#[derive(Debug, PartialEq)]
pub enum ModuleError {
    DuplicateType(ValueType, ValueType),
    DuplicateMethod(FunctionInfo, FunctionInfo),
    DuplicateFunction(FunctionInfo, FunctionInfo),
    DuplicateGlobal(String, ValueType, ValueType),
}

pub type FreeFn = fn(
    &mut Executor,
    Vec<Box<dyn ProgramValue>>,
) -> Result<Option<Box<dyn ProgramValue>>, ExecutorError>;

pub type MethodFn = fn(
    &mut Executor,
    &Box<dyn ProgramValue>,
    Vec<Box<dyn ProgramValue>>,
) -> Result<Option<Box<dyn ProgramValue>>, ExecutorError>;

pub type MethodFnMut = fn(
    &mut Executor,
    &mut Box<dyn ProgramValue>,
    Vec<Box<dyn ProgramValue>>,
) -> Result<Option<Box<dyn ProgramValue>>, ExecutorError>;

pub type PropertyFn = fn(
    &mut Executor,
    &Box<dyn ProgramValue>,
) -> Result<Option<Box<dyn ProgramValue>>, ExecutorError>;

pub type SetterFn = fn(
    &mut Executor,
    &mut Box<dyn ProgramValue>,
    Box<dyn ProgramValue>,
) -> Result<Option<Box<dyn ProgramValue>>, ExecutorError>;

/// Determines how a function is called from scripts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FunctionKind {
    /// Top level free function (add(a, b))
    Free(FreeFn),
    /// Dot call requiring parentheses (x.foo(a, b))
    Method(MethodFn),
    /// Additional method where callee is modified
    MethodMut(MethodFnMut),
    /// Dot access with no parentheses (x.len)
    Property(PropertyFn),
    /// Dot access setter (x.value = 5)
    Setter(SetterFn),
}

impl FunctionKind {
    pub fn has_callee(&self) -> bool {
        match self {
            FunctionKind::Free(_) => false,
            FunctionKind::Method(_) => true,
            FunctionKind::MethodMut(_) => true,
            FunctionKind::Property(_) => true,
            FunctionKind::Setter(_) => true,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionInfo {
    pub name: String,
    pub param_types: Vec<ValueType>,
    pub return_type: Option<ValueType>,
    pub kind: FunctionKind,
}

#[derive(PartialEq, Eq, Clone, Hash, Debug)]
pub struct UnarySignature {
    op: UnaryOp,
    value: ValueType,
}

impl UnarySignature {
    pub fn new(op: UnaryOp, value: ValueType) -> UnarySignature {
        Self { op, value }
    }
}

#[derive(PartialEq, Eq, Clone, Hash, Debug)]
pub struct BinarySignature {
    a: ValueType,
    op: BinaryOp,
    b: ValueType,
}

impl BinarySignature {
    pub fn new(a: ValueType, op: BinaryOp, b: ValueType) -> BinarySignature {
        Self { a, op, b }
    }
}

#[derive(Default, Debug)]
pub struct Module {
    // NOTE: When modifying these fields you need to make sure to add its corresponding module merge!!
    typing: HashMap<String, ValueType>,
    globals: HashMap<String, (ValueType, Box<dyn SyncProgramValue>)>,
    functions: HashMap<String, Vec<FunctionInfo>>,
    methods: HashMap<(ValueType, String), Vec<FunctionInfo>>,
}

impl Module {
    pub fn resolve_type(&self, name: &str) -> Option<ValueType> {
        match name {
            "i64" => Some(ValueType::Int),
            "f64" => Some(ValueType::Float),
            "bool" => Some(ValueType::Bool),
            "String" => Some(ValueType::String),
            name => self.typing.get(name).cloned(),
        }
    }

    /// Register a value that can be accessed like a variable
    pub fn register_global<T: SyncProgramValue>(
        &mut self,
        name: &str,
        value: T,
    ) -> Result<(), ModuleError> {
        let vt = ValueType::of::<T>();
        if let Some(previous) = self
            .globals
            .insert(name.to_owned(), (vt.clone(), Box::new(value)))
        {
            return Err(ModuleError::DuplicateGlobal(
                name.to_string(),
                previous.0,
                vt,
            ));
        }

        Ok(())
    }

    /// Register a native function that can be called from scripts
    pub fn register_function(&mut self, info: FunctionInfo) -> Result<(), ModuleError> {
        if let Some(functions) = self.functions.get_mut(&info.name) {
            // Check for duplicate signature
            for existing in functions.iter() {
                if existing.param_types.iter().eq(info.param_types.iter()) {
                    // Must be a different handler
                    if existing.kind != info.kind {
                        return Err(ModuleError::DuplicateFunction(existing.clone(), info));
                    }
                }
            }

            functions.push(info);
        } else {
            self.functions.insert(info.name.clone(), vec![info]);
        }

        Ok(())
    }

    pub fn resolve_global(&self, name: &str) -> Option<&(ValueType, Box<dyn SyncProgramValue>)> {
        self.globals.get(name)
    }

    /// Lookup a registered native function by name
    pub fn lookup_function(
        &self,
        name: &str,
        params: &[ValueType],
    ) -> Result<&FunctionInfo, Option<&FunctionInfo>> {
        let Some(functions) = self.functions.get(name) else {
            return Err(None);
        };

        for entry in functions {
            if entry.param_types.iter().eq(params) {
                return Ok(entry);
            }
        }

        Err(functions.first())
    }

    /// Register a native method on a specific receiver type
    pub fn register_method(
        &mut self,
        receiver_type: ValueType,
        info: FunctionInfo,
    ) -> Result<(), ModuleError> {
        let key = (receiver_type.clone(), info.name.clone());

        if let Some(methods) = self.methods.get_mut(&key) {
            // Check for duplicate signature
            for existing in methods.iter() {
                if existing.param_types.iter().eq(info.param_types.iter()) {
                    // Must be a different handler
                    if existing.kind != info.kind {
                        return Err(ModuleError::DuplicateMethod(existing.clone(), info));
                    }
                }
            }

            methods.push(info);
        } else {
            self.methods.insert(key, vec![info]);
        }

        Ok(())
    }

    /// Lookup a registered method by receiver type and name
    pub fn find_method(
        &self,
        receiver_type: &ValueType,
        name: &str,
        params: &[ValueType],
    ) -> Option<&FunctionInfo> {
        for entry in self
            .methods
            .get(&(receiver_type.clone(), name.to_string()))?
        {
            if entry.param_types.iter().eq(params) {
                return Some(entry);
            }
        }

        None
    }

    /// Register a custom Rust type.
    /// This by itself allows functions, methods, ect. to use it, but a script can't specify it as a type.
    pub fn register_type<T: ProgramValue + Clone>(&mut self) -> Result<ValueType, ModuleError> {
        let vt = ValueType::of::<T>();
        let full_name = std::any::type_name::<T>();
        if let Some(previous) = self.typing.insert(full_name.to_string(), vt.clone()) {
            // We only care if they're incompatible.
            if vt != previous {
                return Err(ModuleError::DuplicateType(previous, vt));
            }
        }

        define_type_wrappers::<T>(self, vt.clone())?;

        Ok(vt)
    }

    /// Register a custom Rust type with the module under its specified name
    /// This allows scripts to specify it as a type, as well as functions, methods, ect.
    /// TODO: This should validate if it overrides
    pub fn register_type_named<T: ProgramValue + Clone>(
        &mut self,
        type_name: &str,
    ) -> Result<ValueType, ModuleError> {
        let vt = ValueType::CustomType(TypeId::of::<T>(), type_name.to_string());
        if let Some(previous) = self.typing.insert(type_name.to_string(), vt.clone()) {
            return Err(ModuleError::DuplicateType(previous, vt));
        }

        define_type_wrappers::<T>(self, vt.clone())?;

        // We do a normal registration anyways for function signatures
        self.register_type::<T>()?;

        Ok(vt)
    }

    pub fn merge(&mut self, other: Module) -> Result<(), ModuleError> {
        for registered_type in other.typing {
            if let Some(previous) = self
                .typing
                .insert(registered_type.0, registered_type.1.clone())
            {
                return Err(ModuleError::DuplicateType(previous, registered_type.1));
            }
        }

        for global in other.globals {
            let global_type = global.1.0.clone();
            if let Some(previous) = self.globals.insert(global.0.clone(), global.1) {
                return Err(ModuleError::DuplicateGlobal(
                    global.0,
                    previous.0,
                    global_type,
                ));
            }
        }

        for function in other.functions {
            if let Some(functions) = self.functions.get_mut(&function.0) {
                // Check for duplicate signature
                for new in function.1.clone() {
                    for existing in functions.iter() {
                        if existing.param_types.iter().eq(new.param_types.iter()) {
                            // Must be a different handler
                            if existing.kind != new.kind {
                                return Err(ModuleError::DuplicateFunction(
                                    existing.clone(),
                                    new.clone(),
                                ));
                            }
                        }
                    }

                    functions.push(new.clone());
                }
            } else {
                // Take the whole thing
                self.functions.insert(function.0, function.1);
            }
        }

        for method in other.methods {
            if let Some(methods) = self.methods.get_mut(&method.0) {
                // Check for duplicate signature
                for new in method.1.clone() {
                    for existing in methods.iter() {
                        if existing.param_types.iter().eq(new.param_types.iter()) {
                            // Must be a different handler
                            if existing.kind != new.kind {
                                return Err(ModuleError::DuplicateMethod(
                                    existing.clone(),
                                    new.clone(),
                                ));
                            }
                        }
                    }

                    methods.push(new.clone());
                }
            } else {
                // Take the whole thing
                self.methods.insert(method.0, method.1);
            }
        }

        Ok(())
    }
}

pub fn define_type_wrappers<T: ProgramValue + Clone>(
    module: &mut Module,
    typing: ValueType,
) -> Result<(), ModuleError> {
    // Should this be a better generic system?
    // Yes.
    // I don't currently want to or know how to do that.
    // TODO: Fix type wrappers, they should support layered typing better.

    // Base Vec<T>, Option<T>
    let base_vector_type = define_vector_wrap::<T>(module, typing.clone())?;
    let base_option_type = define_option_wrap::<T>(module, typing)?;

    // Vec<Vec<T>>, Vec<Option<T>>, Option<Vec<T>>, Option<Option<T>>
    define_vector_wrap::<Vec<T>>(module, base_vector_type.clone())?;
    let array_of_options = define_vector_wrap::<Option<T>>(module, base_option_type.clone())?;
    define_option_wrap::<Vec<T>>(module, base_vector_type)?;
    define_option_wrap::<Option<T>>(module, base_option_type)?;

    // Option<Vec<Option<T>>>
    define_option_wrap::<Vec<Option<T>>>(module, array_of_options)?;

    Ok(())
}

fn define_option_wrap<T: ProgramValue + Clone>(
    module: &mut Module,
    typing: ValueType,
) -> Result<ValueType, ModuleError> {
    let typing_name = typing.to_string();
    let option_type_name = "Option<".to_string() + typing_name.as_str() + ">";
    let option_value_type =
        ValueType::CustomType(TypeId::of::<Option<T>>(), option_type_name.to_string());
    module
        .typing
        .insert(option_type_name.to_string(), option_value_type.clone());

    // None initialization
    module.register_function(FunctionInfo {
        name: option_type_name.clone(),
        param_types: vec![],
        return_type: Some(option_value_type.clone()),
        kind: FunctionKind::Free(|_executor, _args| {
            let option: Option<T> = None;
            Ok(Some(Box::new(option)))
        }),
    })?;

    // Some initialization
    module.register_function(FunctionInfo {
        name: "Some".to_string(),
        param_types: vec![typing.clone()],
        return_type: Some(option_value_type.clone()),
        kind: FunctionKind::Free(|_executor, mut args| {
            let callee = args
                .pop()
                .expect("Should have Some arg value.")
                .into_any()
                .downcast::<T>()
                .expect("Should downcast callee to T");

            Ok(Some(Box::new(Some(*callee))))
        }),
    })?;

    // Yoink
    module.register_method(
        option_value_type.clone(),
        FunctionInfo {
            name: "unwrap".to_string(),
            param_types: vec![option_value_type.clone()],
            return_type: Some(typing.clone()),
            kind: FunctionKind::Method(|_executor, callee, _args| {
                let callee = callee
                    .as_ref()
                    .as_any()
                    .downcast_ref::<Option<T>>()
                    .expect("Should downcast callee to Option<T>");

                if let Some(callee) = callee {
                    Ok(Some(Box::new(callee.clone())))
                } else {
                    Err(ExecutorError::UnwrapFailed)
                }
            }),
        },
    )?;

    // Yeet
    module.register_method(
        option_value_type.clone(),
        FunctionInfo {
            name: "expect".to_string(),
            param_types: vec![option_value_type.clone(), ValueType::String],
            return_type: Some(typing.clone()),
            kind: FunctionKind::Method(|_executor, callee, mut args| {
                let callee = callee
                    .as_ref()
                    .as_any()
                    .downcast_ref::<Option<T>>()
                    .expect("Should downcast callee to Option<T>");

                if let Some(callee) = callee {
                    Ok(Some(Box::new(callee.clone())))
                } else {
                    // String in args is the explicit message attached to this failure
                    let message = args
                        .pop()
                        .expect("Args should have expect message.")
                        .into_any()
                        .downcast::<String>()
                        .expect("Expected string arg for expect");
                    Err(ExecutorError::ExpectFailed(*message))
                }
            }),
        },
    )?;

    // Sanity checking
    module.register_method(
        option_value_type.clone(),
        FunctionInfo {
            name: "is_some".to_string(),
            param_types: vec![option_value_type.clone()],
            return_type: Some(ValueType::Bool),
            kind: FunctionKind::Method(|_executor, callee, _args| {
                let callee = callee
                    .as_ref()
                    .as_any()
                    .downcast_ref::<Option<T>>()
                    .expect("Should downcast callee to Option<T>");

                Ok(Some(Box::new(callee.is_some())))
            }),
        },
    )?;

    module.register_method(
        option_value_type.clone(),
        FunctionInfo {
            name: "is_none".to_string(),
            param_types: vec![option_value_type.clone()],
            return_type: Some(ValueType::Bool),
            kind: FunctionKind::Method(|_executor, callee, _args| {
                let callee = callee
                    .as_ref()
                    .as_any()
                    .downcast_ref::<Option<T>>()
                    .expect("Should downcast callee to Option<T>");

                Ok(Some(Box::new(callee.is_none())))
            }),
        },
    )?;

    Ok(option_value_type)
}

fn define_vector_wrap<T: ProgramValue + Clone>(
    module: &mut Module,
    typing: ValueType,
) -> Result<ValueType, ModuleError> {
    let typing_name = typing.to_string();
    let vec_type_name = format!("Vec<{typing_name}>");
    let vec_value_type = ValueType::CustomType(TypeId::of::<Vec<T>>(), vec_type_name.to_string());
    module
        .typing
        .insert(vec_type_name.to_string(), vec_value_type.clone());

    let typing_value_optional =
        ValueType::CustomType(TypeId::of::<Option<T>>(), format!("Option<{typing_name}>"));

    module.register_function(FunctionInfo {
        name: vec_type_name.clone(),
        param_types: vec![],
        return_type: Some(vec_value_type.clone()),
        kind: FunctionKind::Free(|_executor, _args| {
            let vec: Vec<T> = Vec::new();
            Ok(Some(Box::new(vec)))
        }),
    })?;

    // Givith
    module.register_method(
        vec_value_type.clone(),
        FunctionInfo {
            name: "push".to_string(),
            param_types: vec![vec_value_type.clone(), typing.clone()],
            return_type: None,
            kind: FunctionKind::MethodMut(|_executor, callee, mut args| {
                let callee = callee
                    .as_any_mut()
                    .downcast_mut::<Vec<T>>()
                    .expect("Should downcast callee to Vec<T>");
                let entry = args
                    .pop()
                    .expect("Should have 1 arg")
                    .into_any()
                    .downcast::<T>()
                    .expect("Push arg for downcast isn't T");
                callee.push(*entry);

                Ok(None)
            }),
        },
    )?;

    module.register_method(
        vec_value_type.clone(),
        FunctionInfo {
            name: "insert".to_string(),
            param_types: vec![vec_value_type.clone(), ValueType::Int, typing.clone()],
            return_type: None,
            kind: FunctionKind::MethodMut(|_executor, callee, mut args| {
                let callee = callee
                    .as_any_mut()
                    .downcast_mut::<Vec<T>>()
                    .expect("Should downcast callee to Vec<T>");
                let index = *args
                    .pop()
                    .expect("Should have 2 arg")
                    .into_any()
                    .downcast::<i64>()
                    .expect("Push arg for downcast isn't i64");
                let entry = args
                    .pop()
                    .expect("Should have 1 arg")
                    .into_any()
                    .downcast::<T>()
                    .expect("Push arg for downcast isn't T");

                if index < 0 {
                    return Err(ExecutorError::NegativeIndex(index));
                } else if index > callee.len() as i64 {
                    return Err(ExecutorError::IndexOutOfRange(index));
                }

                callee.insert(index as usize, *entry);

                Ok(None)
            }),
        },
    )?;

    // Takith
    module.register_method(
        vec_value_type.clone(),
        FunctionInfo {
            name: "pop".to_string(),
            param_types: vec![vec_value_type.clone()],
            return_type: Some(typing_value_optional.clone()),
            kind: FunctionKind::MethodMut(|_executor, callee, _args| {
                let callee = callee
                    .as_any_mut()
                    .downcast_mut::<Vec<T>>()
                    .expect("Should downcast callee to Vec<T>");

                let value = callee.pop();

                Ok(Some(Box::new(value)))
            }),
        },
    )?;

    module.register_method(
        vec_value_type.clone(),
        FunctionInfo {
            name: "remove".to_string(),
            param_types: vec![vec_value_type.clone(), ValueType::Int],
            return_type: None,
            kind: FunctionKind::MethodMut(|_executor, callee, mut args| {
                let callee = callee
                    .as_mut()
                    .as_any_mut()
                    .downcast_mut::<Vec<T>>()
                    .expect("Should downcast callee to Vec<T>");
                let index = *args
                    .pop()
                    .expect("Should have 1 arg")
                    .into_any()
                    .downcast::<i64>()
                    .expect("Index arg for downcast isn't Int");

                if index < 0 {
                    return Err(ExecutorError::NegativeIndex(index));
                } else if index >= callee.len() as i64 {
                    return Err(ExecutorError::IndexOutOfRange(index));
                }

                callee.remove(index as usize);

                Ok(None)
            }),
        },
    )?;

    // Findith
    module.register_method(
        vec_value_type.clone(),
        FunctionInfo {
            name: "get".to_string(),
            param_types: vec![vec_value_type.clone(), ValueType::Int],
            return_type: Some(typing_value_optional),
            kind: FunctionKind::Method(|_executor, callee, mut args| {
                let callee = callee
                    .as_ref()
                    .as_any()
                    .downcast_ref::<Vec<T>>()
                    .expect("Should downcast callee to Vec<T>");
                let entry = *args
                    .pop()
                    .expect("Should have 1 arg")
                    .into_any()
                    .downcast::<i64>()
                    .expect("Index arg for downcast isn't Int");

                if entry < 0 {
                    return Err(ExecutorError::NegativeIndex(entry));
                }

                let value = callee.get(entry as usize).cloned();

                Ok(Some(Box::new(value)))
            }),
        },
    )?;

    module.register_method(
        vec_value_type.clone(),
        FunctionInfo {
            name: "index".to_string(),
            param_types: vec![vec_value_type.clone(), ValueType::Int],
            return_type: Some(typing.clone()),
            kind: FunctionKind::Method(|_executor, callee, mut args| {
                let callee = callee
                    .as_ref()
                    .as_any()
                    .downcast_ref::<Vec<T>>()
                    .expect("Should downcast callee to Vec<T>");
                let entry = *args
                    .pop()
                    .expect("Should have 1 arg")
                    .into_any()
                    .downcast::<i64>()
                    .expect("Index arg for downcast isn't Int");

                if entry < 0 {
                    return Err(ExecutorError::NegativeIndex(entry));
                }

                let value = callee.get(entry as usize).cloned();

                let Some(value) = value else {
                    return Err(ExecutorError::IndexOutOfRange(entry));
                };

                Ok(Some(Box::new(value)))
            }),
        },
    )?;

    // Indexed set method (vec[i] = x)
    // Registered as a method (not setter) because it takes two arguments beyond the receiver.
    module.register_method(
        vec_value_type.clone(),
        FunctionInfo {
            name: "set".to_string(),
            param_types: vec![vec_value_type.clone(), ValueType::Int, typing.clone()],
            return_type: None,
            kind: FunctionKind::MethodMut(|_executor, callee, mut args| {
                let callee = callee
                    .as_any_mut()
                    .downcast_mut::<Vec<T>>()
                    .expect("Should downcast callee to Vec<T>");

                let entry = *args
                    .pop()
                    .expect("Set should have index arg")
                    .into_any()
                    .downcast::<i64>()
                    .expect("Index arg for set downcast isn't Int");

                let value = args
                    .pop()
                    .expect("Set should have value arg")
                    .into_any()
                    .downcast::<T>()
                    .expect("Value arg for set downcast isn't T");

                if entry < 0 {
                    return Err(ExecutorError::NegativeIndex(entry));
                }

                let idx = entry as usize;
                if idx >= callee.len() {
                    return Err(ExecutorError::IndexOutOfRange(entry));
                }

                callee[idx] = *value;
                Ok(None)
            }),
        },
    )?;

    module.register_method(
        vec_value_type.clone(),
        FunctionInfo {
            name: "is_empty".to_string(),
            param_types: vec![vec_value_type.clone()],
            return_type: Some(ValueType::Bool),
            kind: FunctionKind::Method(|_executor, callee, _args| {
                let callee = callee
                    .as_ref()
                    .as_any()
                    .downcast_ref::<Vec<T>>()
                    .expect("Should downcast callee to Vec<T>");

                Ok(Some(Box::new(callee.is_empty())))
            }),
        },
    )?;

    module.register_method(
        vec_value_type.clone(),
        FunctionInfo {
            name: "len".to_string(),
            param_types: vec![vec_value_type.clone()],
            return_type: Some(ValueType::Int),
            kind: FunctionKind::Method(|_executor, callee, _args| {
                let callee = callee
                    .as_ref()
                    .as_any()
                    .downcast_ref::<Vec<T>>()
                    .expect("Should downcast callee to Vec<T>");

                Ok(Some(Box::new(callee.len() as i64)))
            }),
        },
    )?;

    let iterator_type = ValueType::of::<VecIterator<T>>();
    module.register_method(
        vec_value_type.clone(),
        FunctionInfo {
            name: "into_iter".to_string(),
            param_types: vec![vec_value_type.clone()],
            return_type: Some(iterator_type.clone()),
            kind: FunctionKind::Method(|_executor, callee, _args| {
                let callee = callee
                    .as_ref()
                    .as_any()
                    .downcast_ref::<Vec<T>>()
                    .expect("Should downcast callee to Vec<T>");

                Ok(Some(Box::new(VecIterator {
                    index: 0,
                    values: callee.clone(),
                })))
            }),
        },
    )?;

    module.register_method(
        iterator_type.clone(),
        FunctionInfo {
            name: "next".to_string(),
            param_types: vec![iterator_type.clone()],
            return_type: Some(ValueType::of::<Option<T>>()),
            kind: FunctionKind::MethodMut(|_executor, callee, _args| {
                let callee = callee
                    .as_mut()
                    .as_any_mut()
                    .downcast_mut::<VecIterator<T>>()
                    .expect("Should downcast callee to Vec<T>");

                if callee.index >= callee.values.len() {
                    return Ok(Some(Box::new(None::<T>)));
                }

                // TODO: This should probably take the value instead of clone it
                let next = callee.values[callee.index].clone();
                callee.index += 1;

                Ok(Some(Box::new(Some(next))))
            }),
        },
    )?;

    Ok(vec_value_type)
}

#[derive(Clone)]
pub struct VecIterator<T: ProgramValue> {
    pub index: usize,
    pub values: Vec<T>,
}
