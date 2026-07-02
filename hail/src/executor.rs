use std::any::type_name;

use crate::{
    Location,
    constructor::{CallSource, ValueType, VariableSlot},
    instructor::{FunctionEntry, Instruction, Program, ProgramValue},
    module::FunctionKind,
    visualize::{Diagnostic, Severity},
};

#[derive(Debug, PartialEq, Eq)]
pub struct ExecutorError {
    pub location: Location,
    pub kind: ExecutorErrorKind,
}

impl ExecutorError {
    pub fn new(kind: ExecutorErrorKind) -> Self {
        Self {
            location: Location {
                line: 0,
                column: 0,
                length: 0,
            },
            kind,
        }
    }

    pub fn new_with_program(
        kind: ExecutorErrorKind,
        program: &Program,
        program_counter: usize,
    ) -> Self {
        if let Some(location) = program.locations.get(program_counter).copied() {
            Self { location, kind }
        } else {
            Self::new(kind)
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum ExecutorErrorKind {
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
    Fail(String),
    NegativeIndex(i64),
    IndexOutOfRange(i64, usize),
    // In use, allocation size, memory limit
    OutOfMemory(usize, usize, usize),
    InstructionLimit(usize),
}

impl ExecutorError {
    pub fn message(&self) -> String {
        match &self.kind {
            ExecutorErrorKind::WrongReturnValue(expected, found) => {
                format!("Expected return type `{expected}` but got `{found}`.")
            }
            ExecutorErrorKind::NoReturnValue => {
                "Function was expected to return a value but returned none.".to_string()
            }
            ExecutorErrorKind::MissingFunction(name) => {
                format!("Function `{name}` not found in program.")
            }
            ExecutorErrorKind::InvalidFunctionCall(name, expected, found) => {
                let exp: Vec<String> = expected.iter().map(|t| t.to_string()).collect();
                let fnd: Vec<String> = found.iter().map(|t| t.to_string()).collect();
                format!(
                    "Invalid call to `{name}`. Expected params ({}) but got ({}).",
                    exp.join(", "),
                    fnd.join(", ")
                )
            }
            ExecutorErrorKind::InvalidFunctionReturnType(name, expected, found) => {
                let exp = expected
                    .as_ref()
                    .map(|t| t.to_string())
                    .unwrap_or_else(|| "none".to_string());
                let fnd = found
                    .as_ref()
                    .map(|t| t.to_string())
                    .unwrap_or_else(|| "none".to_string());
                format!("Function `{name}` return type mismatch. Expected `{exp}` but got `{fnd}`.")
            }
            ExecutorErrorKind::FunctionDidNotObeyTyping(name, expected, found) => {
                let exp = expected
                    .as_ref()
                    .map(|t| t.to_string())
                    .unwrap_or_else(|| "none".to_string());
                format!(
                    "Function `{name}` did not obey typing. Expected return `{exp}` but got `{found}`."
                )
            }
            ExecutorErrorKind::ReturnDidNotObeyTyping => {
                "Return value type does not match function signature.".to_string()
            }
            ExecutorErrorKind::UnwrapFailed => "Attempted to unwrap a `None` value.".to_string(),
            ExecutorErrorKind::ExpectFailed(msg) => {
                format!("Expect failed: {msg}")
            }
            ExecutorErrorKind::Todo(msg) => {
                format!("TODO: {msg}")
            }
            ExecutorErrorKind::Fail(msg) => {
                format!("FAIL: {msg}")
            }
            ExecutorErrorKind::NegativeIndex(idx) => {
                format!("Array index must be non-negative, got {idx}.")
            }
            ExecutorErrorKind::IndexOutOfRange(idx, len) => {
                format!("Array index {idx} is out of bounds for array of length {len}.")
            }
            ExecutorErrorKind::OutOfMemory(used, allocating, max_memory) => {
                // TODO / FIXME: Make these readouts pretty
                format!(
                    "Out of memory! In use: {used} bytes, Failed Allocation: {allocating} bytes, Memory Limit: {max_memory} bytes"
                )
            }
            ExecutorErrorKind::InstructionLimit(instructions) => {
                format!(
                    "Out of instructions! The limit of {instructions} instruction executions was reached."
                )
            }
        }
    }
}

impl Diagnostic for ExecutorError {
    fn severity(&self) -> Severity {
        Severity::Error
    }

    fn location(&self) -> Location {
        self.location
    }

    fn message(&self) -> String {
        self.message()
    }
}

#[derive(Clone, Debug)]
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

#[derive(Default, Debug)]
pub struct Executor {
    // Settings
    tracking: bool,
    memory_limit: usize,
    instruction_limit: usize,

    // Stats
    peak_memory_usage: usize,

    // State
    counter: usize,
    active_memory_usage: usize,
    executed_instructions: usize,
    variables: Vec<Box<dyn ProgramValue>>,
    stack: Vec<Box<dyn ProgramValue>>,
    call_stack: Vec<CallFrame>,
}

impl Clone for Executor {
    fn clone(&self) -> Self {
        Self {
            tracking: self.tracking.clone(),
            memory_limit: self.memory_limit.clone(),
            instruction_limit: self.instruction_limit.clone(),
            peak_memory_usage: self.peak_memory_usage.clone(),
            counter: self.counter.clone(),
            active_memory_usage: self.active_memory_usage.clone(),
            executed_instructions: self.executed_instructions.clone(),
            variables: self
                .variables
                .iter()
                .map(|v| v.as_ref().clone_box())
                .collect(),
            stack: self.stack.iter().map(|v| v.as_ref().clone_box()).collect(),
            call_stack: self.call_stack.clone(),
        }
    }
}

pub enum ExecutionResult {
    Result(Box<Result<Box<dyn ProgramValue>, ExecutorError>>),
    Yield,
    InstructionLimit,
    MemoryOverflow,
}

pub enum StepResult {
    Result(Box<Option<Box<dyn ProgramValue>>>),
    Continue,
    Yield,
    InstructionLimit,
}

pub struct TrackingInfo {
    pub executed_instructions: usize,
    pub memory_peak: usize,
}

impl Executor {
    pub fn tracked(instruction_limit: usize, memory_limit: usize) -> Executor {
        Executor {
            tracking: true,
            instruction_limit,
            memory_limit,
            ..Default::default()
        }
    }

    pub fn instruction_in_bounds(&self, program: &Program) -> bool {
        self.counter < program.instructions.len()
    }

    // Update and reset tracking with new limits
    pub fn update_limits(&mut self, instruction_limit: usize, memory_limit: usize) {
        self.executed_instructions = 0;

        self.instruction_limit = instruction_limit;
        self.memory_limit = memory_limit;
        self.peak_memory_usage = self.memory_limit;
    }

    /// Take the tracking info, resetting it
    /// Useful for "script budgets" where you might want to resume next tick
    pub fn take_tracking(&mut self) -> TrackingInfo {
        let executed_instructions = self.executed_instructions;
        self.executed_instructions = 0;

        let memory_peak = self.peak_memory_usage;
        self.peak_memory_usage = self.active_memory_usage;

        TrackingInfo {
            executed_instructions,
            memory_peak,
        }
    }

    pub fn current_memory_usage(&self) -> usize {
        self.active_memory_usage
    }

    fn increment_memory(
        &mut self,
        value: &Box<dyn ProgramValue>,
        program: &Program,
    ) -> Result<(), ExecutorError> {
        // Memory usage of Box<ProgramValue> doesn't count the Box as it's "unfair" to count Executor overhead.
        let size = value.as_ref().memory_usage();

        if size + self.active_memory_usage > self.memory_limit {
            return Err(ExecutorError::new_with_program(
                ExecutorErrorKind::OutOfMemory(self.active_memory_usage, size, self.memory_limit),
                program,
                self.counter,
            ));
        }
        self.active_memory_usage += size;

        // Keep track of highest usage
        if self.active_memory_usage > self.peak_memory_usage {
            self.peak_memory_usage = self.active_memory_usage;
        }

        Ok(())
    }

    fn decrement_memory(&mut self, value: &Box<dyn ProgramValue>) {
        // Memory usage of Box<ProgramValue> doesn't count the Box as it's "unfair" to count Executor overhead.
        self.active_memory_usage -= value.as_ref().memory_usage();
    }

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
            Err(ExecutorError::new(ExecutorErrorKind::NoReturnValue))
        }
    }

    /// Run the script as a whole while returning a typed output value (required value)
    pub fn run_with_return<T: ProgramValue>(self, program: &Program) -> Result<T, ExecutorError> {
        let value = self.run_with_any_return(program)?;

        let any_val = value.as_any();
        // Touch weird to avoid a clone while still getting type info lazily
        match any_val.downcast_ref::<T>() {
            Some(_) => Ok(*value.into_any().downcast().unwrap()),
            None => Err(ExecutorError::new(ExecutorErrorKind::WrongReturnValue(
                type_name::<T>().to_owned(),
                value.display_name(),
            ))),
        }
    }

    /// Run the provided function of the script
    /// !!! WARNING !!!
    /// The reason param_typing and return_type are provided here is to attempt to type check against *your* expectations
    /// If what you provide or cast to is not an expected type, you can expect undefined behavior
    /// Perhaps ProgramValue should have a way to get ValueType? Sounds slow.
    pub fn run_function_with_any_return(
        mut self,
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
            return Err(ExecutorError::new(ExecutorErrorKind::MissingFunction(
                name.to_string(),
            )));
        };

        // Ensure signature matches
        if function.params.len() != param_typing.len() {
            return Err(ExecutorError::new(ExecutorErrorKind::InvalidFunctionCall(
                name.to_string(),
                function.params.clone(),
                param_typing,
            )));
        }

        for (index, param) in function.params.iter().enumerate() {
            if &param_typing[index] != param {
                return Err(ExecutorError::new(ExecutorErrorKind::InvalidFunctionCall(
                    name.to_string(),
                    function.params.clone(),
                    param_typing,
                )));
            }
        }

        if function.return_type != return_type {
            return Err(ExecutorError::new(
                ExecutorErrorKind::InvalidFunctionReturnType(
                    name.to_string(),
                    function.return_type.clone(),
                    return_type.clone(),
                ),
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
            return Err(ExecutorError::new(
                ExecutorErrorKind::FunctionDidNotObeyTyping(
                    name.to_string(),
                    function.return_type.clone(),
                    result.map_or_else(|| "No Return".to_string(), |v| v.display_name()),
                ),
            ));
        }

        // Survived
        Ok(result)
    }

    /// See ``run_function_with_any_return`` above
    /// This is a very low level function that will blow up if not treated with care, use the wrapper
    fn run_function_with_any_return_impl(
        &mut self,
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

        // For memory purposes we're ignoring CallFrames for simplicity
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
            if self.tracking {
                self.increment_memory(&arg, program)?;
            }

            self.variables.push(arg);
        }

        // Execute and validate
        let result = self.execute_impl(program)?;
        if returns != result.is_some() {
            return Err(ExecutorError::new(
                ExecutorErrorKind::ReturnDidNotObeyTyping,
            ));
        }

        Ok(result)
    }

    fn execute_impl(
        &mut self,
        program: &Program,
    ) -> Result<Option<Box<dyn ProgramValue>>, ExecutorError> {
        while self.counter < program.instructions.len() {
            //std::thread::sleep(std::time::Duration::from_millis(250));
            //println!("Executing instruction: [{}] {instruction:?}", self.counter);
            //println!("Stack:\n{:?}", self.stack);
            //println!("Variables:\n{:?}", self.variables);
            match self.execute_instruction(program)? {
                StepResult::Result(result) => return Ok(*result),
                StepResult::Continue | StepResult::Yield => continue,
                StepResult::InstructionLimit => {
                    return Err(ExecutorError::new_with_program(
                        ExecutorErrorKind::InstructionLimit(self.instruction_limit),
                        program,
                        self.counter,
                    ));
                }
            }
        }

        // We've ran the entire program and at this point expect it to only have 0-1 stack elements
        // TODO / FIXME / CHECKME: This is likely wrong and could have weird execution result consequences
        let value = self.stack.pop();

        if self.tracking
            && let Some(value) = &value
        {
            self.decrement_memory(value);
        }

        Ok(value)
    }

    pub fn execute_instruction(&mut self, program: &Program) -> Result<StepResult, ExecutorError> {
        if self.tracking {
            if self.executed_instructions >= self.instruction_limit {
                return Ok(StepResult::InstructionLimit);
            }
            self.executed_instructions += 1;
        }

        let instruction_index = self.counter;
        let instruction = &program.instructions[self.counter];

        match instruction {
            Instruction::Constant(constant) => {
                let constant = constant.as_ref().clone_box();
                if self.tracking {
                    self.increment_memory(&constant, program)?;
                }

                self.stack.push(constant);
            }
            Instruction::SetVariable(variable_slot) => {
                // Since we're moving the value from the stack into variables, we don't decrement increment memory, just decrement the old
                let mut value = self
                    .stack
                    .pop()
                    .expect("Set variable stack value should exist.");

                let slot_index = self.resolve_variable_slot(variable_slot);
                // Slots are created only when needed
                if self.variables.len() <= slot_index {
                    self.variables.push(value);
                } else {
                    if self.tracking {
                        std::mem::swap(&mut self.variables[slot_index], &mut value);

                        // Make sure to decrement memory for the previous value
                        self.decrement_memory(&value);
                    } else {
                        self.variables[slot_index] = value;
                    }
                }
            }
            Instruction::ReserveVariable(variable_slot) => {
                // Reserve the variable slot, it's value will be nothing, no memory needs to be tracked.
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

                if self.tracking {
                    self.decrement_memory(&value);
                }

                let condition = value.into_any().downcast::<bool>().expect("Should be bool");

                // Jump if *false*
                if !*condition {
                    self.counter = *address;
                    return Ok(StepResult::Continue);
                }
            }
            Instruction::JumpIfTrue(address) => {
                let value = self
                    .stack
                    .pop()
                    .expect("Jump condition value should exist.");

                if self.tracking {
                    self.decrement_memory(&value);
                }

                let condition = value.into_any().downcast::<bool>().expect("Should be bool");

                // Jump if *true*
                if *condition {
                    self.counter = *address;
                    return Ok(StepResult::Continue);
                }
            }
            Instruction::Jump(address) => {
                self.counter = *address;
                return Ok(StepResult::Continue);
            }
            Instruction::GetVariable(variable_slot) => {
                let value = self.variables[self.resolve_variable_slot(variable_slot)]
                    .as_ref()
                    .clone_box();

                if self.tracking {
                    self.increment_memory(&value, program)?;
                }

                self.stack.push(value);
            }
            Instruction::LoadFunction(index) => {
                let entry = program.functions[*index].clone_box();

                if self.tracking {
                    self.increment_memory(&entry, program)?;
                }

                self.stack.push(entry);
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

                // We're moving these args straight to variables, no need to handle memory management
                let args: Vec<Box<dyn ProgramValue>> =
                    self.stack.drain(stack_len - param_count..).collect();

                let local_base = self.variables.len();
                let stack_base = self.stack.len(); // Stack after args were drained

                // For memory purposes we're ignoring CallFrames for simplicity
                self.call_stack.push(CallFrame {
                    return_address: self.counter + 1,
                    local_base,
                    stack_base,
                    return_token: false,
                });

                // Allocate all local slots at once.
                // First param_count slots are filled with the arguments.
                for arg in args {
                    // Since we're moving from stack to variables we don't need to change memory counters
                    self.variables.push(arg);
                }

                // Remaining slots (locals declared inside the body) start uninitialized.
                // TODO: Needed?
                for _ in param_count..entry_info.local_count {
                    self.variables.push(Box::new(()));
                }

                self.counter = entry;
                return Ok(StepResult::Continue);
            }
            Instruction::CallDynamic(arg_count) => {
                let param_count = *arg_count;

                // Drain args from top of stack
                let stack_len = self.stack.len();
                assert!(
                    stack_len >= param_count + 1,
                    "Expected {param_count} args + function value on stack"
                );

                // Memory counting won't be done on this as args are moved straight to variables
                let args: Vec<Box<dyn ProgramValue>> =
                    self.stack.drain(stack_len - param_count..).collect();

                // Pop the function value that sits below the args
                let func_val = self.stack.pop().expect("Function value missing");
                if self.tracking {
                    self.decrement_memory(&func_val);
                }

                let func = *func_val
                    .into_any()
                    .downcast::<FunctionEntry>()
                    .expect("Value is not a FunctionEntry");

                let local_base = self.variables.len();
                let stack_base = self.stack.len();

                // For memory purposes we're ignoring CallFrames for simplicity
                self.call_stack.push(CallFrame {
                    return_address: self.counter + 1,
                    local_base,
                    stack_base,
                    return_token: false,
                });

                for arg in args {
                    // Since we're moving from stack to variables we don't need to change memory counters
                    self.variables.push(arg);
                }

                // Remaining slots (locals declared inside the body) start uninitialized.
                // TODO: Needed?
                for _ in param_count..func.local_count {
                    self.variables.push(Box::new(()));
                }

                self.counter = func.instruction;
                return Ok(StepResult::Continue);
            }
            Instruction::Return(has_value) => {
                let return_value = if *has_value {
                    let value = self.stack.pop().expect("Expected value on stack");

                    // Decrement memory as we may not actually put it back on the stack
                    if self.tracking {
                        self.decrement_memory(&value);
                    }

                    Some(value)
                } else {
                    None
                };

                // Top-level return (no call frame means we're at script level, just exit)
                if self.call_stack.is_empty() {
                    return Ok(StepResult::Result(Box::new(None)));
                }

                let frame = self.call_stack.pop().expect("Call stack is empty");

                if frame.return_token {
                    return Ok(StepResult::Result(Box::new(return_value)));
                }

                // Resume at the saved return address
                self.counter = frame.return_address;

                if self.tracking {
                    // Clearing the stack and variables is more annoying when tracked so here's some magic to avoid cloning where shortly unused capacity will be

                    unsafe {
                        // SAFETY: Decrement memory just updates the memory usage, and the pointer is verified to be within range
                        if self.stack.len() > frame.stack_base {
                            let stack_ptr = self.stack.as_ptr().add(frame.stack_base);
                            let len = self.stack.len() - frame.stack_base;
                            for i in 0..len {
                                self.decrement_memory(&*stack_ptr.add(i));
                            }
                        }
                    }

                    unsafe {
                        // SAFETY: Same as above
                        if self.variables.len() > frame.local_base {
                            let vars_ptr = self.variables.as_ptr().add(frame.local_base);
                            let len = self.variables.len() - frame.local_base;
                            for i in 0..len {
                                self.decrement_memory(&*vars_ptr.add(i));
                            }
                        }
                    }
                }

                // Restore the value stack to where it was before the call
                self.stack.truncate(frame.stack_base);

                // Deallocate all local variables for this frame
                self.variables.truncate(frame.local_base);

                if let Some(return_value) = return_value {
                    // Put it back in memory
                    // TODO / FIXME: We likely could avoid this push pop
                    if self.tracking {
                        self.increment_memory(&return_value, program)?;
                    }

                    self.stack.push(return_value);
                }

                return Ok(StepResult::Continue);
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

                if self.tracking {
                    for arg in &args {
                        self.decrement_memory(arg);
                    }
                }

                // Pop callee before calling
                let mut callee = if has_callee {
                    let callee = self.stack.pop().expect("Callee missing");

                    // If we put the callee back, we'll increment again later
                    if self.tracking {
                        self.decrement_memory(&callee);
                    }

                    Some(callee)
                } else {
                    None
                };

                let result = match call_info.call_fn {
                    FunctionKind::Free(free) => free(self, args).map_err(|err| {
                        // A
                        ExecutorError::new_with_program(err, program, instruction_index)
                    })?,
                    FunctionKind::Method(method) => {
                        let value =
                            method(self, callee.as_ref().unwrap(), args).map_err(|err| {
                                // A
                                ExecutorError::new_with_program(err, program, instruction_index)
                            })?;

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
                        let value =
                            method(self, callee.as_mut().unwrap(), args).map_err(|err| {
                                ExecutorError::new_with_program(err, program, instruction_index)
                            })?;

                        if value.is_some() != call_info.return_type.is_some() {
                            panic!(
                                "Call info and reality don't match, a value was either returned or not unexpectedly in '{}'",
                                call_info.name
                            );
                        }

                        if let CallSource::Variable(slot) = &call_info.source {
                            let mut callee = callee.unwrap();
                            let slot_index = self.resolve_variable_slot(slot);

                            if self.tracking {
                                self.increment_memory(&callee, program)?;
                            }

                            if self.tracking {
                                std::mem::swap(&mut self.variables[slot_index], &mut callee);

                                // Make sure to decrement memory for the previous value
                                self.decrement_memory(&callee);
                            } else {
                                self.variables[slot_index] = callee;
                            }
                        }

                        value
                    }
                    FunctionKind::Property(property) => {
                        let value = property(self, callee.as_mut().unwrap()).map_err(|err| {
                            ExecutorError::new_with_program(err, program, instruction_index)
                        })?;

                        if value.is_some() != call_info.return_type.is_some() {
                            // Bad developer, obey your return info
                            panic!(
                                "Call info and reality don't match, a value was either returned or not unexpectedly in '{}'",
                                call_info.name
                            );
                        }

                        if let CallSource::Variable(slot) = &call_info.source {
                            let mut callee = callee.unwrap();
                            let slot_index = self.resolve_variable_slot(slot);

                            if self.tracking {
                                self.increment_memory(&callee, program)?;
                            }

                            if self.tracking {
                                std::mem::swap(&mut self.variables[slot_index], &mut callee);

                                // Make sure to decrement memory for the previous value
                                self.decrement_memory(&callee);
                            } else {
                                self.variables[slot_index] = callee;
                            }
                        }

                        value
                    }
                    FunctionKind::Setter(setter) => {
                        let value = setter(
                            self,
                            callee.as_mut().unwrap(),
                            // TODO: Does this need a check?
                            args[0].as_ref().clone_box(),
                        )
                        .map_err(|err| {
                            ExecutorError::new_with_program(err, program, instruction_index)
                        })?;

                        if value.is_some() != call_info.return_type.is_some() {
                            panic!(
                                "Call info and reality don't match, a value was either returned or not unexpectedly in '{}'",
                                call_info.name
                            );
                        }

                        if let CallSource::Variable(slot) = &call_info.source {
                            let callee = callee.unwrap();
                            let slot_index = self.resolve_variable_slot(slot);

                            if self.tracking {
                                self.increment_memory(&callee, program)?;
                            }

                            self.variables[slot_index] = callee;
                        }

                        value
                    }
                };

                if let Some(result) = result {
                    if self.tracking {
                        self.increment_memory(&result, program)?;
                    }

                    self.stack.push(result);
                }
            }
            Instruction::ImportedFunctionCall(program_index, function_index, arg_count) => {
                if self.tracking {
                    todo!("Imported function calls need to support partial execution");
                }

                let stack_len = self.stack.len();

                // Take off the stack and then put into the expected ordering
                let args: Vec<Box<dyn ProgramValue>> =
                    self.stack.drain(stack_len - arg_count..).collect();

                if self.tracking {
                    for arg in &args {
                        // Remove from memory as the new executor will account for them
                        self.decrement_memory(&arg);
                    }
                }

                // Find function
                let (_, program) = &program.imports[*program_index];
                let function = &program.functions[*function_index];

                // Execute for value
                let mut executor = if self.tracking {
                    // It gets the remaining memory
                    Executor::tracked(
                        self.instruction_limit - self.executed_instructions,
                        self.memory_limit - self.active_memory_usage,
                    )
                } else {
                    Executor::default()
                };
                let result = executor.run_function_with_any_return_impl(
                    program,
                    *function_index,
                    args,
                    function.return_type.is_some(),
                )?;

                if self.tracking {
                    let stats = executor.take_tracking();
                    self.executed_instructions += stats.executed_instructions;

                    // Take the current highest or active + the peak of the above execution
                    self.peak_memory_usage = self
                        .peak_memory_usage
                        .max(self.active_memory_usage + stats.memory_peak);
                }

                if result.is_some() != function.return_type.is_some() {
                    unreachable!(
                        "You wouldn't hit a return type with undefined behavior would ya?"
                    );
                    // Kaboom
                }

                if let Some(result) = result {
                    if self.tracking {
                        // Track the new memory
                        self.increment_memory(&result, program)?;
                    }

                    self.stack.push(result);
                }
            }
            Instruction::Drop(drop_count) => {
                // In the event we push something to the stack (like a function result)
                // and we don't want to use the value, we need to explicitly get rid of it.

                if self.tracking {
                    // Clearing the stack is more annoying when tracked so here's some magic to avoid cloning where shortly unused capacity will be

                    unsafe {
                        // SAFETY: Decrement memory just updates the memory usage, and the pointer is verified to be within range
                        if self.stack.len() >= *drop_count {
                            let stack_ptr = self.stack.as_ptr().add(self.stack.len() - drop_count);
                            for i in 0..*drop_count {
                                self.decrement_memory(&*stack_ptr.add(i));
                            }
                        }
                    }
                }

                self.stack
                    .truncate(self.stack.len().saturating_sub(*drop_count));
            }
        }

        self.counter += 1;
        Ok(StepResult::Continue)
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
