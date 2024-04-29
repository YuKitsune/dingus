use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use crate::config::{CommandActionConfig, VariableConfig};
use crate::prompt::ConfirmExecutor;
use crate::shell::{ShellExecutorFactory};
use crate::variables::{VariableResolver};

pub struct ActionExecutor {
    pub shell_executor_factory: Box<dyn ShellExecutorFactory>,
    pub confirm_executor: ConfirmExecutor,
    pub variable_resolver: VariableResolver
}

impl ActionExecutor {
    pub fn execute(
        &self,
        command_action: &CommandActionConfig,
        variable_configs: &HashMap<String, VariableConfig>,
        ) -> Result<(), Box<dyn Error>> {

        let variables = self.variable_resolver.resolve_variables(variable_configs)?;

        return match command_action {
            CommandActionConfig::Execution(shell_command) => {

                let shell_executor = self.shell_executor_factory.create_default();
                let result = shell_executor.execute(shell_command, &variables);

                // Todo: If the command fails to execute, fail the remaining steps, or seek user input (continue or abort)
                if let Err(err) = result {
                    return Err(Box::new(err))
                }

                Ok(())
            },
            CommandActionConfig::ExtendedExecution(execution_config) => {

                let shell_executor = match &execution_config.shell {
                    Some(shell) => self.shell_executor_factory.create(&shell),
                    None => self.shell_executor_factory.create_default(),
                };

                let result = shell_executor.execute(&execution_config.shell_command, &variables);

                // Todo: If the command fails to execute, fail the remaining steps, or seek user input (continue or abort)
                if let Err(err) = result {
                    return Err(Box::new(err))
                }

                Ok(())
            },
            CommandActionConfig::Confirmation(confirm_config) => {
                let result = self.confirm_executor.execute(confirm_config)?;
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
