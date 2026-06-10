use std::{
    any::{Any, type_name},
    fmt::Debug,
    sync::Arc,
};

use crate::{
    ImportResolver, Location,
    constructor::{AExpr, AStmt, CallSource, FinalizedConstruct, ValueType, VariableSlot},
    module::{FunctionKind, Module},
    parser::LogicalOp,
};

pub trait ProgramValue: 'static {
    fn clone_box(&self) -> Box<dyn ProgramValue>;
    fn into_any(self: Box<Self>) -> Box<dyn Any>;
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn display_name(&self) -> String;
}

impl Debug for dyn ProgramValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.display_name())
    }
}

impl<T: Clone + 'static> ProgramValue for T {
    /// Clone a program value
    /// NOTE: As this is usually wrapped in a box, it needs to have an .as_ref() to get inside right for typing
    fn clone_box(&self) -> Box<dyn ProgramValue> {
        Box::new(self.clone())
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn display_name(&self) -> String {
        type_name::<T>().to_owned()
    }
}

pub trait SyncProgramValue: ProgramValue + 'static + Sync + Send {
    fn clone_sync_box(&self) -> Box<dyn SyncProgramValue>;
}

impl Debug for dyn SyncProgramValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.display_name())
    }
}

impl<T: Clone + 'static + Send + Sync> SyncProgramValue for T {
    /// Clone a program value
    /// NOTE: As this is usually wrapped in a box, it needs to have an .as_ref() to get inside right for typing
    fn clone_sync_box(&self) -> Box<dyn SyncProgramValue> {
        Box::new(self.clone())
    }
}

#[derive(Debug, Clone)]
pub struct FunctionEntry {
    pub name: String,
    pub instruction: usize,
    pub local_count: usize,
    pub params: Vec<ValueType>,
    pub return_type: Option<ValueType>,
}

#[derive(Debug, Clone)]
pub struct CallInfo {
    pub name: String,
    pub params: Vec<ValueType>,
    pub return_type: Option<ValueType>,
    pub call_fn: FunctionKind,
    pub source: CallSource,
}

/// Note: Try not to clone this, just wrap it in an Arc plz
pub struct Program {
    pub instructions: Vec<Instruction>,
    pub locations: Vec<Location>,
    pub functions: Vec<FunctionEntry>,
    pub call_info: Vec<CallInfo>,
    pub imports: Vec<(String, Program)>,
    pub program_source: String,
    pub source_path: Option<String>,
    pub script_resolver: Arc<dyn ImportResolver>,
    pub module: Arc<Module>,
}

impl Debug for Program {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Program")
            .field("instructions", &self.instructions)
            .field("locations", &self.locations)
            .field("functions", &self.functions)
            .field("call_info", &self.call_info)
            .field("imports", &self.imports)
            .field("program_source", &self.program_source)
            .field("source_path", &self.source_path)
            .finish()
    }
}

#[derive(Debug)]
pub enum Instruction {
    Print,

    // Const
    Constant(Box<dyn SyncProgramValue>),

    // Pop a variable into a slot
    SetVariable(VariableSlot),
    ReserveVariable(VariableSlot),
    // Drop X values from the stack
    Drop(usize),

    // Push a variable onto the stack
    GetVariable(VariableSlot),
    // Jump address
    JumpIfFalse(usize),
    JumpIfTrue(usize),
    Jump(usize),

    // Symbol address
    Call(usize),

    // Push a ScriptFunction value onto the stack by index
    LoadFunction(usize),
    // Call user defined script. Index, arg count
    CallScript(usize, usize),
    // Call ScriptFunction on stack
    CallDynamic(usize), // arg_count
    // Return from a script function. Has return value
    Return(bool),
    // Program index, function index, arg count
    ImportedFunctionCall(usize, usize, usize),
}

pub struct Instructor {
    program: Program,
    // Stack of (break_targets, continue_targets) per nested loop level
    // Each entry holds a list of instruction indices that need patching
    pending_jumps: Vec<(Vec<usize>, Vec<usize>)>,
}

impl Instructor {
    pub fn new(
        module: Arc<Module>,
        resolver: Arc<dyn ImportResolver>,
        source_path: Option<String>,
    ) -> Instructor {
        Instructor {
            program: Program {
                instructions: vec![],
                locations: vec![],
                functions: vec![],
                call_info: vec![],
                imports: vec![],
                program_source: "".to_string(),
                source_path,
                script_resolver: resolver,
                module,
            },
            pending_jumps: vec![],
        }
    }

    pub fn generate(mut self, construct: FinalizedConstruct, source_code: String) -> Program {
        self.program.imports = construct.imports;
        self.program.program_source = source_code;

        // Emit all function definitions first so every call site can resolve addresses
        for stmt in &construct.stmts {
            if matches!(stmt, AStmt::Function(..)) {
                self.statement(stmt);
            }
        }
        // Emit non-function statements
        for stmt in &construct.stmts {
            if !matches!(stmt, AStmt::Function(..)) {
                self.statement(stmt);
            }
        }
        self.program
    }

    fn push_instruction(&mut self, location: Location, instruction: Instruction) {
        self.program.locations.push(location);
        self.program.instructions.push(instruction);
    }

    fn statement(&mut self, stmt: &AStmt) {
        match stmt {
            AStmt::Print(location, aexpr) => {
                self.expression(aexpr);

                let value_type = aexpr
                    .get_value_type()
                    .expect("Should have return value type")
                    .value_type();
                if value_type != ValueType::String {
                    todo!("UHHHH");
                    /*
                    self.push_instruction(
                        *location,
                        Instruction::Stringify(
                            *self
                                .module
                                .stringify_ops
                                .get(&value_type)
                                .expect("Should have stringify op"),
                        ),
                    );*/
                }

                self.push_instruction(*location, Instruction::Print);
            }
            AStmt::SetVariable(location, variable_slot, _, aexpr) => {
                // Push the variable to the stack
                if let Some(initializer) = aexpr {
                    self.expression(initializer);

                    // Consume the variable into a slot
                    self.push_instruction(*location, Instruction::SetVariable(*variable_slot));
                } else {
                    // Set the slot to an empty value
                    self.push_instruction(*location, Instruction::ReserveVariable(*variable_slot));
                }
            }
            AStmt::While(location, condition, body) => {
                let loop_start = self.program.instructions.len();

                // Push a new level for this loop's break / continue targets
                self.pending_jumps.push((vec![], vec![]));

                // Evaluate condition
                self.expression(condition);

                // Jump to end if condition is false
                let jump_if_false_idx = self.program.instructions.len();
                self.push_instruction(*location, Instruction::JumpIfFalse(0));

                // The body
                self.expression(body);

                // Go back to condition
                let continue_target = self.program.instructions.len();
                self.push_instruction(*location, Instruction::Jump(loop_start));

                // Patch Jump targets to land on the unconditional jump back
                if let Some(pending) = self.pending_jumps.last_mut() {
                    for idx in &pending.1 {
                        self.program.instructions[*idx] = Instruction::Jump(continue_target);
                    }
                }

                // Patch JumpIfFalse to jump to end of loop
                let break_target = self.program.instructions.len();
                self.program.instructions[jump_if_false_idx] =
                    Instruction::JumpIfFalse(break_target);

                // Patch Jump targets to jump to end of loop
                if let Some(pending) = self.pending_jumps.last_mut() {
                    for idx in &pending.0 {
                        self.program.instructions[*idx] = Instruction::Jump(break_target);
                    }
                }

                // Pop this loop level
                self.pending_jumps.pop();
            }
            AStmt::Loop(expr) => {
                let loop_start = self.program.instructions.len();

                // Push a new level for this loop's break / continue targets
                self.pending_jumps.push((vec![], vec![]));

                // The body
                self.expression(expr);

                // Go back to start
                let continue_target = self.program.instructions.len();
                self.push_instruction(expr.get_location(), Instruction::Jump(loop_start));

                // Patch Jump targets to land on the unconditional jump back
                if let Some(pending) = self.pending_jumps.last_mut() {
                    for idx in &pending.1 {
                        self.program.instructions[*idx] = Instruction::Jump(continue_target);
                    }
                }

                // Patch Jump targets to jump to end of loop
                let break_target = self.program.instructions.len();
                if let Some(pending) = self.pending_jumps.last_mut() {
                    for idx in &pending.0 {
                        self.program.instructions[*idx] = Instruction::Jump(break_target);
                    }
                }

                // Pop this loop level
                self.pending_jumps.pop();
            }
            AStmt::Expression(_, aexpr, produces) => {
                self.expression(aexpr);

                // Get it off the stack if it has a value but it won't be used
                if aexpr.get_value_type().is_some() && !produces {
                    //todo!("AAAA");
                    //self.push_instruction(aexpr.get_location(), Instruction::Drop(1));
                }
            }
            AStmt::Function(location, name, index, params, body, return_type, local_count) => {
                // Emit a Jump to skip the function body at definition time
                let jump_index = self.program.instructions.len();
                self.push_instruction(*location, Instruction::Jump(0));

                // Pad function index
                if self.program.functions.len() <= *index {
                    self.program.functions.resize(
                        *index + 1,
                        FunctionEntry {
                            name: String::new(),
                            instruction: 0,
                            local_count: 0,
                            params: vec![],
                            return_type: None,
                        },
                    );
                }

                // Modify function
                let entry = self.program.instructions.len();
                self.program.functions[*index] = FunctionEntry {
                    name: name.to_string(),
                    instruction: entry,
                    local_count: *local_count,
                    params: params.iter().map(|v| v.2.clone()).collect(),
                    return_type: return_type.clone(),
                };

                // Body
                self.expression(body);

                // Return
                self.push_instruction(*location, Instruction::Return(return_type.is_some()));

                // Patch the jump to skip past the body + return instruction
                self.program.instructions[jump_index] =
                    Instruction::Jump(self.program.instructions.len());
            }
            AStmt::Return(location, expr) => {
                let has_value = expr.is_some();
                if let Some(expr) = expr {
                    self.expression(expr);
                }
                self.push_instruction(*location, Instruction::Return(has_value));
            }
            AStmt::Break(location) => {
                // Emit Jump with placeholder address (patched later by the enclosing while loop)
                self.push_instruction(*location, Instruction::Jump(0));
                // Record this instruction index for patching
                if let Some(pending) = self.pending_jumps.last_mut() {
                    pending.0.push(self.program.instructions.len() - 1);
                }
            }
            AStmt::Continue(location) => {
                // Emit Jump with placeholder address (patched later by the enclosing while loop)
                self.push_instruction(*location, Instruction::Jump(0));
                // Record this instruction index for patching
                if let Some(pending) = self.pending_jumps.last_mut() {
                    pending.1.push(self.program.instructions.len() - 1);
                }
            }
            AStmt::Import(_, _, _, _) => {
                // Import doesn't emit instructions, it's used by other AStmts
            }
        }
    }

    fn expression(&mut self, expr: &AExpr) {
        match expr {
            AExpr::Constant(location, val, _) => {
                self.push_instruction(
                    *location,
                    Instruction::Constant(val.as_ref().clone_sync_box()),
                );
            }
            AExpr::AssignVariable(location, variable_slot, _, aexpr) => {
                // Push the variable to the stack
                self.expression(aexpr);

                // Regular assignment
                self.push_instruction(*location, Instruction::SetVariable(*variable_slot));
            }
            AExpr::RetrieveVariable(location, variable_slot, _, _) => {
                self.push_instruction(*location, Instruction::GetVariable(*variable_slot));
            }
            AExpr::Logical(aexpr_a, logical_op, aexpr_b) => {
                match logical_op {
                    LogicalOp::And => {
                        // Eval A
                        self.expression(aexpr_a);

                        // A short circuit
                        let fail_a_index = self.program.instructions.len();
                        self.push_instruction(aexpr_a.get_location(), Instruction::JumpIfFalse(0));

                        // Eval B if we don't short circuit
                        self.expression(aexpr_b);

                        // Jump to fail if failed
                        let fail_b_index = self.program.instructions.len();
                        self.push_instruction(aexpr_b.get_location(), Instruction::JumpIfFalse(0));

                        // Success to stack
                        self.push_instruction(
                            aexpr_b.get_location(),
                            Instruction::Constant(Box::new(true)),
                        );

                        // Jump to end
                        let jump_success_index = self.program.instructions.len();
                        self.push_instruction(aexpr_b.get_location(), Instruction::Jump(0));

                        // Fail path, update jumps
                        let fail_path_idx = self.program.instructions.len();
                        self.program.instructions[fail_a_index] =
                            Instruction::JumpIfFalse(fail_path_idx);
                        self.program.instructions[fail_b_index] =
                            Instruction::JumpIfFalse(fail_path_idx);

                        // Push fail value
                        self.push_instruction(
                            aexpr_a.get_location(),
                            Instruction::Constant(Box::new(false)),
                        );

                        // Update happy case end jump to go to the next executed instruction
                        self.program.instructions[jump_success_index] =
                            Instruction::Jump(self.program.instructions.len());
                    }
                    LogicalOp::Or => {
                        // Eval A
                        self.expression(aexpr_a);

                        // A short circuit
                        let success_a_index = self.program.instructions.len();
                        self.push_instruction(aexpr_a.get_location(), Instruction::JumpIfTrue(0));

                        // Eval B if we don't short circuit
                        self.expression(aexpr_b);

                        // Jump to success if success
                        let success_b_index = self.program.instructions.len();
                        self.push_instruction(aexpr_b.get_location(), Instruction::JumpIfTrue(0));

                        // Fail to stack
                        self.push_instruction(
                            aexpr_b.get_location(),
                            Instruction::Constant(Box::new(false)),
                        );

                        // Jump to end
                        let jump_fail_index = self.program.instructions.len();
                        self.push_instruction(aexpr_b.get_location(), Instruction::Jump(0));

                        // Success path, update jumps
                        let success_path_idx = self.program.instructions.len();
                        self.program.instructions[success_a_index] =
                            Instruction::JumpIfTrue(success_path_idx);
                        self.program.instructions[success_b_index] =
                            Instruction::JumpIfTrue(success_path_idx);

                        // Push success value
                        self.push_instruction(
                            aexpr_a.get_location(),
                            Instruction::Constant(Box::new(true)),
                        );

                        // Update fail case end jump to go to the next executed instruction
                        self.program.instructions[jump_fail_index] =
                            Instruction::Jump(self.program.instructions.len());
                    }
                }
            }
            AExpr::Block(_, astmts, _) => {
                // Before, implementations would handle scoping for blocks explicitly
                // Here though, with slots, it *shouldn't* need to as only frames matter
                // So... No realloc of the variables..?

                // Execute
                for stmt in astmts {
                    self.statement(stmt);
                }

                // What would be an exit scope
            }
            AExpr::Call(location, func_name, source, params, call_fn, return_type) => {
                // Evaluate all parameter expressions left to right
                for param in params {
                    self.expression(param);
                }

                // Collect parameter types
                let param_types: Vec<ValueType> = params
                    .iter()
                    .map(|p| {
                        p.get_value_type()
                            .expect("Param should have value type")
                            .value_type()
                    })
                    .collect();

                // Emit the Call instruction with all metadata needed at runtime
                let call_info_index = self.program.call_info.len();
                self.program.call_info.push(CallInfo {
                    name: func_name.clone(),
                    params: param_types,
                    return_type: return_type.clone(),
                    call_fn: call_fn.clone(),
                    source: source.clone(),
                });

                self.push_instruction(*location, Instruction::Call(call_info_index));
            }
            AExpr::ScriptCall(location, index, params, _return_type) => {
                // Evaluate args left to right
                let arg_count = params.len();
                for param in params {
                    self.expression(param);
                }
                self.push_instruction(*location, Instruction::CallScript(*index, arg_count));
            }
            AExpr::FunctionReference(location, index, _params, _return_type) => {
                // Push ScriptFunction value onto the stack, the executor resolves the entry
                self.push_instruction(*location, Instruction::LoadFunction(*index));
            }
            AExpr::DynamicCall(location, callee, params, _return_type) => {
                // Push the function value first, then the arguments
                self.expression(callee);
                let arg_count = params.len();
                for param in params {
                    self.expression(param);
                }
                self.push_instruction(*location, Instruction::CallDynamic(arg_count));
            }
            AExpr::ImportedFunctionCall(
                location,
                program_index,
                function_index,
                params,
                _return_type,
                args,
            ) => {
                for arg in args {
                    self.expression(arg);
                }

                //todo!("Verify");
                self.push_instruction(
                    *location,
                    Instruction::ImportedFunctionCall(
                        *program_index,
                        *function_index,
                        params.len(),
                    ),
                );
            }
            AExpr::If(condition, body, otherwise) => {
                // Evaluate condition
                self.expression(condition);

                // Jump to else if false (address to be fixed up)
                let jump_false_idx = self.program.instructions.len();
                self.push_instruction(condition.get_location(), Instruction::JumpIfFalse(0));

                // Execute body
                self.expression(body);

                // If there's an else, jump over it after body
                let end_label_idx = if otherwise.is_some() {
                    let jump_idx = self.program.instructions.len();
                    self.push_instruction(condition.get_location(), Instruction::Jump(0));
                    Some(jump_idx)
                } else {
                    None
                };

                // Fix up the JumpIfFalse to point to else/start of else
                self.program.instructions[jump_false_idx] =
                    Instruction::JumpIfFalse(self.program.instructions.len());

                // Execute otherwise
                if let Some(else_expr) = otherwise {
                    self.expression(else_expr);
                }

                // If there was an else, fix up the jump over it
                if let Some(end_label_idx) = end_label_idx {
                    self.program.instructions[end_label_idx] =
                        Instruction::Jump(self.program.instructions.len());
                }
            }
        }
    }
}
