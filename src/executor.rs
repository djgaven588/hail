use std::any::{Any, TypeId, type_name, type_name_of_val};

use crate::instructor::{Program, ProgramValue};

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
        // -> Result<Option<T>, > {
        while let Some(instruction) = program.instructions.get(self.counter) {
            println!("Executing instruction: {instruction:?}");
            match instruction {
                crate::instructor::Instruction::Constant(index) => {
                    self.stack.push(program.constants[*index].clone_box());
                }
                crate::instructor::Instruction::Unary(_) => todo!(),
                crate::instructor::Instruction::Binary(action) => {
                    let b = self.stack.pop().expect("Stack Binary B should exist.");
                    action(
                        self.stack.last_mut().expect("Stack Binary A should exist."),
                        b,
                    );
                }
                crate::instructor::Instruction::SetVariable(variable_slot) => {
                    let value = self
                        .stack
                        .pop()
                        .expect("Set variable stack value should exist.");

                    if variable_slot.is_global {
                        self.variables[variable_slot.index] = value;
                    } else {
                        todo!("Need frames")
                        //self.variables[self.]
                    }
                }
            }

            self.counter += 1;
        }

        Ok(())
    }
}
