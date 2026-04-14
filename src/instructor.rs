use std::{
    any::{Any, type_name},
    fmt::Debug,
    sync::Arc,
};

use crate::{
    Location,
    constructor::{AExpr, AStmt, ValueType, VariableSlot},
    module::Module,
    parser::AssignmentOp,
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

#[derive(Debug)]
pub struct Program {
    pub constants: Vec<Box<dyn ProgramValue>>,
    pub instructions: Vec<Instruction>,
    pub locations: Vec<Location>,
}

#[derive(Debug)]
pub enum Instruction {
    Print,

    // Const
    Constant(Box<dyn ProgramValue>),
    // Unary op on value
    Unary(fn(&mut Box<dyn ProgramValue>)),
    // Binary op on values
    Binary(fn(&mut Box<dyn ProgramValue>, Box<dyn ProgramValue>)),
    // A -> "A"
    Stringify(fn(&mut Box<dyn ProgramValue>)),

    // Pop a variable into a slot
    SetVariable(VariableSlot),
    ReserveVariable(VariableSlot),
    // Modify an existing variable
    AssignVariable(
        VariableSlot,
        fn(&mut Box<dyn ProgramValue>, Box<dyn ProgramValue>),
    ),

    // Push a variable onto the stack
    GetVariable(VariableSlot),
    // Jump address
    JumpIfFalse(usize),
    Jump(usize),
}

pub struct Instructor {
    module: Arc<Module>,
    program: Program,
}

impl Instructor {
    pub fn new(module: Arc<Module>) -> Instructor {
        Instructor {
            module,
            program: Program {
                instructions: vec![],
                locations: vec![],
                constants: vec![],
            },
        }
    }

    pub fn generate(mut self, stmts: &[AStmt]) -> Program {
        for stmt in stmts {
            self.statement(stmt);
        }

        self.program
    }

    fn push_instruction(&mut self, location: Location, instruction: Instruction) {
        self.program.locations.push(location);
        self.program.instructions.push(instruction);
    }

    fn statement(&mut self, stmt: &AStmt) {
        match stmt {
            AStmt::Print(location, astmt) => {
                self.statement(astmt);

                let value_type = astmt
                    .get_value_type()
                    .expect("Should have return value type");
                if value_type != ValueType::String {
                    self.push_instruction(
                        *location,
                        Instruction::Stringify(
                            *self
                                .module
                                .stringify_ops
                                .get(&value_type)
                                .expect("Should have stringify op"),
                        ),
                    );
                }

                self.push_instruction(*location, Instruction::Print);
            }
            AStmt::SetVariable(location, variable_slot, _, astmt) => {
                // Push the variable to the stack
                if let Some(initializer) = astmt {
                    self.statement(initializer);

                    // Consume the variable into a slot
                    self.push_instruction(*location, Instruction::SetVariable(*variable_slot));
                } else {
                    // Set the slot to an empty value
                    self.push_instruction(*location, Instruction::ReserveVariable(*variable_slot));
                }
            }
            AStmt::AssignVariable(location, variable_slot, _, op, astmt) => {
                // Push the variable to the stack
                self.statement(astmt);

                if op == &AssignmentOp::Equal {
                    // Regular assignment
                    self.push_instruction(*location, Instruction::SetVariable(*variable_slot));
                    return;
                }

                let assign_op = self
                    .module
                    .assignment_ops
                    .get(&(astmt.get_value_type().expect("Should have value type"), *op))
                    .expect("Assign operator should be available?");

                // Consume the variable into a slot
                self.push_instruction(
                    *location,
                    Instruction::AssignVariable(*variable_slot, *assign_op),
                );
            }
            AStmt::While(location, condition, body) => {
                let start_instruction_count = self.program.instructions.len();
                self.statement(condition);

                let jump_if_count = self.program.instructions.len();
                // Jump to after if we fail, we'll setup the address shortly
                self.push_instruction(*location, Instruction::JumpIfFalse(0));

                // The body
                self.statement(body);

                // Go back to condition
                self.push_instruction(*location, Instruction::Jump(start_instruction_count));

                // We're back for that jump instruction
                // Go to the end
                self.program.instructions[jump_if_count] =
                    Instruction::JumpIfFalse(self.program.instructions.len());
            }
            AStmt::Expression(_, aexpr) => {
                self.expression(aexpr);
            }
            AStmt::Block(_, astmts, _) => {
                // Before, implementations would handle scoping for blocks explicitly
                // Here though, with slots, it *shouldn't* need to as only frames matter
                // So... No realloc of the variables..?

                // Execute
                for stmt in astmts {
                    self.statement(stmt);
                }

                // What would be an exit scope
            }
            AStmt::Function(location, name, params, body, return_type) => todo!(),
        }
    }

    fn expression(&mut self, expr: &AExpr) {
        match expr {
            AExpr::Constant(location, val, _) => {
                self.push_instruction(*location, Instruction::Constant(val.as_ref().clone_box()));
            }
            AExpr::Unary(location, unary_op, aexpr, _) => {
                self.expression(aexpr);

                self.push_instruction(*location, Instruction::Unary(*unary_op));
            }
            AExpr::Binary(location, expr_a, binary_op, expr_b, _) => {
                self.expression(expr_a);
                self.expression(expr_b);

                self.push_instruction(*location, Instruction::Binary(*binary_op));
            }
            AExpr::RetrieveVariable(location, variable_slot, _, _) => {
                self.push_instruction(*location, Instruction::GetVariable(*variable_slot));
            }
        }
    }
}
