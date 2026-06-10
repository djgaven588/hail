use std::any::type_name;

use crate::{
    constructor::{CallSource, ValueType, VariableSlot},
    instructor::{FunctionEntry, Instruction, Program, ProgramValue},
    module::FunctionKind,
};

#[derive(Debug, PartialEq, Eq)]
pub enum ExecutorError {
    // Expected name, provided name
    WrongReturnValue(String, String),
    NoReturnValue,
    MissingFunction(String),
    InvalidFunctionCall(String, Vec<ValueType>, Vec<ValueType>),
    InvalidFunctionReturnType(String, Option<ValueType>, Option<ValueType>),
    FunctionDidNotObeyTyping(String, Option<ValueType>, String),
    ReturnDidNotObeyTyping,
    UnwrapFailed,
    ExpectFailed(String),
    Todo(String),
    NegativeIndex(i64),
    IndexOutOfRange(i64),
}

struct CallFrame {
    /// Where to resume execution after this function returns
    return_address: usize,
    /// Index into `variables` where this frame's locals start
    local_base: usize,
    /// Stack depth at the point the call was made (after args were popped)
    /// Used to restore the stack on return without leaving stale values
    stack_base: usize,
    return_token: bool,
}

#[derive(Default)]
pub struct Executor {
    counter: usize,
    variables: Vec<Box<dyn ProgramValue>>,
    stack: Vec<Box<dyn ProgramValue>>,
    call_stack: Vec<CallFrame>,
}

impl Executor {
    /// Run the script as a whole
    pub fn run(mut self, program: &Program) -> Result<(), ExecutorError> {
        self.execute_impl(program)?;

        Ok(())
    }

    /// Run the script as a whole while returning the output value (required value)
    pub fn run_with_any_return(
        mut self,
        program: &Program,
    ) -> Result<Box<dyn ProgramValue>, ExecutorError> {
        let result = self.execute_impl(program)?;

        if let Some(program_value) = result {
            Ok(program_value)
        } else {
            Err(ExecutorError::NoReturnValue)
        }
    }

    /// Run the script as a whole while returning a typed output value (required value)
    pub fn run_with_return<T: ProgramValue>(self, program: &Program) -> Result<T, ExecutorError> {
        let value = self.run_with_any_return(program)?;

        let any_val = value.as_any();
        // Touch weird to avoid a clone while still getting type info lazily
        match any_val.downcast_ref::<T>() {
            Some(_) => Ok(*value.into_any().downcast().unwrap()),
            None => Err(ExecutorError::WrongReturnValue(
                type_name::<T>().to_owned(),
                value.display_name(),
            )),
        }
    }

    /// Run the provided function of the script
    /// !!! WARNING !!!
    /// The reason param_typing and return_type are provided here is to attempt to type check against *your* expectations
    /// If what you provide or cast to is not an expected type, you can expect undefined behavior
    /// Perhaps ProgramValue should have a way to get ValueType? Sounds slow.
    pub fn run_function_with_any_return(
        self,
        program: &Program,
        name: &str,
        param_typing: Vec<ValueType>,
        params: Vec<Box<dyn ProgramValue>>,
        return_type: Option<ValueType>,
    ) -> Result<Option<Box<dyn ProgramValue>>, ExecutorError> {
        // Do your damn job, look above
        assert_eq!(param_typing.len(), params.len());

        // Search for function
        let Some((function_index, function)) = program
            .functions
            .iter()
            .enumerate()
            .find(|v| v.1.name.as_str() == name)
        else {
            return Err(ExecutorError::MissingFunction(name.to_string()));
        };

        // Ensure signature matches
        if function.params.len() != param_typing.len() {
            return Err(ExecutorError::InvalidFunctionCall(
                name.to_string(),
                function.params.clone(),
                param_typing,
            ));
        }

        for (index, param) in function.params.iter().enumerate() {
            if &param_typing[index] != param {
                return Err(ExecutorError::InvalidFunctionCall(
                    name.to_string(),
                    function.params.clone(),
                    param_typing,
                ));
            }
        }

        if function.return_type != return_type {
            return Err(ExecutorError::InvalidFunctionReturnType(
                name.to_string(),
                function.return_type.clone(),
                return_type.clone(),
            ));
        }

        // Execute
        let result = self.run_function_with_any_return_impl(
            program,
            function_index,
            params,
            return_type.is_some(),
        )?;

        // Validate return
        if result.is_some() != return_type.is_some() {
            return Err(ExecutorError::FunctionDidNotObeyTyping(
                name.to_string(),
                function.return_type.clone(),
                result.map_or_else(|| "No Return".to_string(), |v| v.display_name()),
            ));
        }

        // Survived
        Ok(result)
    }

    /// See ``run_function_with_any_return`` above
    /// This is a very low level function that will blow up if not treated with care, use the wrapper
    fn run_function_with_any_return_impl(
        mut self,
        program: &Program,
        function_index: usize,
        params: Vec<Box<dyn ProgramValue>>,
        returns: bool,
    ) -> Result<Option<Box<dyn ProgramValue>>, ExecutorError> {
        let function = &program.functions[function_index];

        // Run without a care for the result (we are "arming" the executor for it)
        // This is needed to initialize globals
        self.execute_impl(program)?;

        let local_base = self.variables.len();
        let stack_base = self.stack.len();

        self.call_stack.push(CallFrame {
            return_address: self.counter + 1,
            local_base,
            stack_base,
            return_token: true, // Fire an early exit
        });

        // Jump to function
        self.counter = function.instruction;

        // Push params to stack for the call
        for arg in params.into_iter() {
            self.variables.push(arg);
        }

        // Execute and validate
        let result = self.execute_impl(program)?;
        if returns != result.is_some() {
            return Err(ExecutorError::ReturnDidNotObeyTyping);
        }

        Ok(result)
    }

    fn execute_impl(
        &mut self,
        program: &Program,
    ) -> Result<Option<Box<dyn ProgramValue>>, ExecutorError> {
        while self.counter < program.instructions.len() {
            let instruction = &program.instructions[self.counter];
            //std::thread::sleep(std::time::Duration::from_millis(250));
            //println!("Executing instruction: [{}] {instruction:?}", self.counter);
            //println!("Stack:\n{:?}", self.stack);
            //println!("Variables:\n{:?}", self.variables);
            match instruction {
                Instruction::Print => {
                    let value = self.stack.pop().expect("Should have print message");
                    // Hope that the prior stage set this up correctly. Yolo
                    let string = value.into_any().downcast::<String>().unwrap();
                    println!("Print: {string}");
                }
                Instruction::Constant(index) => {
                    self.stack.push(index.as_ref().clone_box());
                }
                Instruction::SetVariable(variable_slot) => {
                    let value = self
                        .stack
                        .pop()
                        .expect("Set variable stack value should exist.");

                    let slot_index = self.resolve_variable_slot(variable_slot);
                    // Slots are created only when needed
                    if self.variables.len() <= slot_index {
                        self.variables.push(value);
                    } else {
                        self.variables[slot_index] = value;
                    }
                }
                Instruction::ReserveVariable(variable_slot) => {
                    // Reserve the variable slot, it's value will be nothing
                    let value = Box::new(());

                    let slot_index = self.resolve_variable_slot(variable_slot);
                    // Slots are created only when needed
                    if self.variables.len() <= slot_index {
                        self.variables.push(value);
                    } else {
                        self.variables[slot_index] = value;
                    }
                }
                Instruction::JumpIfFalse(address) => {
                    let value = self
                        .stack
                        .pop()
                        .expect("Jump condition value should exist.");

                    let condition = value.into_any().downcast::<bool>().expect("Should be bool");

                    // Jump if *false*
                    if !*condition {
                        self.counter = *address;
                        continue;
                    }
                }
                Instruction::JumpIfTrue(address) => {
                    let value = self
                        .stack
                        .pop()
                        .expect("Jump condition value should exist.");

                    let condition = value.into_any().downcast::<bool>().expect("Should be bool");

                    // Jump if *true*
                    if *condition {
                        self.counter = *address;
                        continue;
                    }
                }
                Instruction::Jump(address) => {
                    self.counter = *address;
                    continue;
                }
                Instruction::GetVariable(variable_slot) => {
                    self.stack.push(
                        self.variables[self.resolve_variable_slot(variable_slot)]
                            .as_ref()
                            .clone_box(),
                    );
                }
                Instruction::LoadFunction(index) => {
                    let entry = &program.functions[*index];
                    self.stack.push(Box::new(entry.clone()));
                }
                Instruction::CallScript(index, arg_count) => {
                    let entry_info = &program.functions[*index];
                    let entry = entry_info.instruction;
                    let param_count = *arg_count; // Validated by constructor

                    // Drain args left to right from the top of the stack
                    let stack_len = self.stack.len();
                    assert!(
                        stack_len >= param_count,
                        "CallScript '{}': expected {param_count} args, found {stack_len}",
                        entry_info.name
                    );
                    let args: Vec<Box<dyn ProgramValue>> =
                        self.stack.drain(stack_len - param_count..).collect();

                    let local_base = self.variables.len();
                    let stack_base = self.stack.len(); // Stack after args were drained

                    self.call_stack.push(CallFrame {
                        return_address: self.counter + 1,
                        local_base,
                        stack_base,
                        return_token: false,
                    });

                    // Allocate all local slots at once.
                    // First param_count slots are filled with the arguments.
                    for arg in args {
                        self.variables.push(arg);
                    }

                    // Remaining slots (locals declared inside the body) start uninitialized.
                    // TODO: Needed?
                    for _ in param_count..entry_info.local_count {
                        self.variables.push(Box::new(()));
                    }

                    self.counter = entry;
                    continue;
                }
                Instruction::CallDynamic(arg_count) => {
                    let param_count = *arg_count;

                    // Drain args from top of stack
                    let stack_len = self.stack.len();
                    assert!(
                        stack_len >= param_count + 1,
                        "Expected {param_count} args + function value on stack"
                    );
                    let args: Vec<Box<dyn ProgramValue>> =
                        self.stack.drain(stack_len - param_count..).collect();

                    // Pop the function value that sits below the args
                    let func_val = self.stack.pop().expect("Function value missing");
                    let func = *func_val
                        .into_any()
                        .downcast::<FunctionEntry>()
                        .expect("Value is not a FunctionEntry");

                    let local_base = self.variables.len();
                    let stack_base = self.stack.len();

                    self.call_stack.push(CallFrame {
                        return_address: self.counter + 1,
                        local_base,
                        stack_base,
                        return_token: false,
                    });

                    for arg in args {
                        self.variables.push(arg);
                    }
                    for _ in param_count..func.local_count {
                        self.variables.push(Box::new(()));
                    }

                    self.counter = func.instruction;
                    continue;
                }
                Instruction::Return(has_value) => {
                    let return_value = if *has_value {
                        Some(self.stack.pop().expect("Expected value on stack"))
                    } else {
                        None
                    };

                    // Top-level return (no call frame means we're at script level, just exit)
                    if self.call_stack.is_empty() {
                        return Ok(None);
                    }

                    let frame = self.call_stack.pop().expect("Call stack is empty");

                    if frame.return_token {
                        return Ok(return_value);
                    }

                    // Resume at the saved return address
                    self.counter = frame.return_address;

                    // Restore the value stack to where it was before the call
                    self.stack.truncate(frame.stack_base);

                    // Deallocate all local variables for this frame
                    self.variables.truncate(frame.local_base);

                    if let Some(return_value) = return_value {
                        self.stack.push(return_value);
                    }

                    continue;
                }
                Instruction::Call(call_info_index) => {
                    let call_info = &program.call_info[*call_info_index];
                    let stack_len = self.stack.len();

                    let has_callee = call_info.call_fn.has_callee();

                    // Take off the stack and then put into the expected ordering
                    let mut args: Vec<Box<dyn ProgramValue>> = self
                        .stack
                        .drain(
                            stack_len
                                - if has_callee {
                                    call_info.params.len().saturating_sub(1)
                                } else {
                                    call_info.params.len()
                                }..,
                        )
                        .collect();
                    args.reverse();

                    // Pop callee before calling
                    let mut callee = if has_callee {
                        Some(self.stack.pop().expect("Callee missing"))
                    } else {
                        None
                    };

                    let result = match call_info.call_fn {
                        FunctionKind::Free(free) => free(self, args)?,
                        FunctionKind::Method(method) => {
                            let value = method(self, callee.as_ref().unwrap(), args)?;

                            if value.is_some() != call_info.return_type.is_some() {
                                panic!(
                                    "Call info and reality don't match, a value was either returned or not unexpectedly in '{}'",
                                    call_info.name
                                );
                            }

                            // Unlike the below MethodMut, the variable isn't restored due to it not modifying the callee

                            value
                        }
                        FunctionKind::MethodMut(method) => {
                            let value = method(self, callee.as_mut().unwrap(), args)?;

                            if value.is_some() != call_info.return_type.is_some() {
                                panic!(
                                    "Call info and reality don't match, a value was either returned or not unexpectedly in '{}'",
                                    call_info.name
                                );
                            }

                            if let CallSource::Variable(slot) = &call_info.source {
                                let slot_index = self.resolve_variable_slot(slot);
                                self.variables[slot_index] = callee.unwrap();
                            }

                            value
                        }
                        FunctionKind::Property(property) => {
                            let value = property(self, callee.as_mut().unwrap())?;

                            if value.is_some() != call_info.return_type.is_some() {
                                // Bad developer, obey your return info
                                panic!(
                                    "Call info and reality don't match, a value was either returned or not unexpectedly in '{}'",
                                    call_info.name
                                );
                            }

                            if let CallSource::Variable(slot) = &call_info.source {
                                let slot_index = self.resolve_variable_slot(slot);
                                self.variables[slot_index] = callee.unwrap();
                            }

                            value
                        }
                        FunctionKind::Setter(setter) => {
                            let value = setter(
                                self,
                                callee.as_mut().unwrap(),
                                // TODO: Does this need a check?
                                args[0].as_ref().clone_box(),
                            )?;

                            if value.is_some() != call_info.return_type.is_some() {
                                panic!(
                                    "Call info and reality don't match, a value was either returned or not unexpectedly in '{}'",
                                    call_info.name
                                );
                            }

                            if let CallSource::Variable(slot) = &call_info.source {
                                let slot_index = self.resolve_variable_slot(slot);
                                self.variables[slot_index] = callee.unwrap();
                            }

                            value
                        }
                    };

                    if let Some(result) = result {
                        self.stack.push(result);
                    }
                }
                Instruction::ImportedFunctionCall(program_index, function_index, arg_count) => {
                    let stack_len = self.stack.len();

                    // Take off the stack and then put into the expected ordering
                    let args: Vec<Box<dyn ProgramValue>> =
                        self.stack.drain(stack_len - arg_count..).collect();

                    // Find function
                    let (_, program) = &program.imports[*program_index];
                    let function = &program.functions[*function_index];

                    // Execute for value
                    let executor = Executor::default();
                    let result = executor.run_function_with_any_return_impl(
                        program,
                        *function_index,
                        args,
                        function.return_type.is_some(),
                    )?;

                    if result.is_some() != function.return_type.is_some() {
                        unreachable!(
                            "You wouldn't hit a return type with undefined behavior would ya?"
                        );
                        // Kaboom
                    }

                    if let Some(result) = result {
                        self.stack.push(result);
                    }
                }
                Instruction::Drop(drop_count) => {
                    // In the event we push something to the stack (like a function result)
                    // and we don't want to use the value, we need to explicitly get rid of it.
                    self.stack
                        .truncate(self.stack.len().saturating_sub(*drop_count));
                }
            }

            self.counter += 1;
        }

        // We've ran the entire program and at this point expect it to only have 0-1 stack elements
        Ok(self.stack.pop())
    }

    fn resolve_variable_slot(&self, slot: &VariableSlot) -> usize {
        if slot.is_global {
            slot.index
        } else {
            self.call_stack
                .last()
                .expect("Non-global variable accessed outside a call frame")
                .local_base
                + slot.index
        }
    }
}
