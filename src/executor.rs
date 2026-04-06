use std::any::{Any, TypeId, type_name, type_name_of_val};

use crate::{
    constructor::VariableSlot,
    instructor::{Instruction, Program, ProgramValue},
};

#[derive(Debug)]
pub enum ExecutorError {
    // Expected name, provided name
    WrongReturnValue(String, String),
    NoReturnValue,
}

#[derive(Default)]
pub struct Executor {
    counter: usize,
    variables: Vec<Box<dyn ProgramValue>>,
    stack: Vec<Box<dyn ProgramValue>>,
}

impl Executor {
    pub fn run(&mut self, program: &Program) -> Result<(), ExecutorError> {
        self.execute(program)?;

        Ok(())
    }

    pub fn run_with_return<T: ProgramValue>(
        &mut self,
        program: &Program,
    ) -> Result<T, ExecutorError> {
        self.execute(program)?;

        if let Some(val) = self.stack.pop() {
            match val.into_any().downcast::<T>() {
                Ok(val) => Ok(*val),
                Err(val) => Err(ExecutorError::WrongReturnValue(
                    type_name::<T>().to_owned(),
                    type_name_of_val(&val).to_owned(),
                )),
            }
        } else {
            Err(ExecutorError::NoReturnValue)
        }
    }

    fn execute(&mut self, program: &Program) -> Result<(), ExecutorError> {
        while self.counter < program.instructions.len() {
            let instruction = &program.instructions[self.counter];
            //std::thread::sleep(std::time::Duration::from_secs(1));
            //println!("Executing instruction: {instruction:?}");
            match instruction {
                Instruction::Constant(index) => {
                    self.stack.push(program.constants[*index].clone_box());
                }
                Instruction::Unary(_) => todo!(),
                Instruction::Binary(action) => {
                    let b = self.stack.pop().expect("Stack Binary B should exist.");
                    action(
                        self.stack.last_mut().expect("Stack Binary A should exist."),
                        b,
                    );
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
                Instruction::JumpIfFalse(address) => {
                    let value = self
                        .stack
                        .pop()
                        .expect("Jump condition value should exist.");

                    let condition = value.into_any().downcast::<bool>().expect("Should be bool");

                    // Jump if *false*
                    if !condition.as_ref() {
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
                Instruction::AssignVariable(info) => {
                    let (variable_slot, action) = info.as_ref();
                    let b = self
                        .stack
                        .pop()
                        .expect("Set variable stack value should exist.");

                    let slot_index = self.resolve_variable_slot(variable_slot);

                    action(&mut self.variables[slot_index], b);
                }
            }

            self.counter += 1;
        }

        Ok(())
    }

    fn resolve_variable_slot(&self, slot: &VariableSlot) -> usize {
        if slot.is_global { slot.index } else { todo!() }
    }
}
