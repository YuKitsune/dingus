use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use crate::definitions::{CommandAction, VariableDefinition};
use crate::prompt::ConfirmExecutor;
use crate::shell::ShellExecutor;
use crate::variables::{VariableResolver};

pub struct ActionExecutor {
    pub shell_executor: Box<dyn ShellExecutor>,
    pub confirm_executor: ConfirmExecutor,
    pub variable_resolver: VariableResolver
}

type Reason = String;

impl ActionExecutor {
    pub fn execute(
        &self,
        command_action: &CommandAction,
        variable_definitions: &HashMap<String, VariableDefinition>,
        ) -> Result<(), Box<dyn Error>> {

        let variables = self.variable_resolver.resolve_variables(variable_definitions)?;

        return match command_action {
            CommandAction::Execution(shell_command) => {

                let result = self.shell_executor.execute(shell_command, &variables);

                // Todo: If the command fails to execute, fail the remaining steps, or seek user input (continue or abort)
                if let Err(err) = result {
                    return Err(Box::new(err))
                }

                Ok(())
            },
            CommandAction::Confirmation(confirm_definition) => {
                let result = self.confirm_executor.execute(confirm_definition)?;
                if result == false {
                    return Err(Box::new(ConfirmationError))
                }

                Ok(())
            }
        }
    }
}

#[derive(Debug, Clone)]
struct ConfirmationError;

impl fmt::Display for ConfirmationError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "confirmation resulted in a negative result")
    }
}

impl Error for ConfirmationError { }
