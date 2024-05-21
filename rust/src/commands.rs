use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use crate::config::{ActionConfig, VariableConfig};
use crate::exec::CommandExecutor;
use crate::prompt::ConfirmExecutor;
use crate::variables::{VariableResolver};

pub struct ActionExecutor {
    pub command_executor: Box<dyn CommandExecutor>,
    pub confirm_executor: ConfirmExecutor,
    pub variable_resolver: VariableResolver
}

impl ActionExecutor {
    pub fn execute(
        &self,
        action_config: &ActionConfig,
        variable_configs: &HashMap<String, VariableConfig>,
    ) -> Result<(), Box<dyn Error>> {

        let variables = self.variable_resolver.resolve_variables(variable_configs)?;

        return match action_config {
            ActionConfig::Execution(execution_config) => {
                let result = self.command_executor.execute(&execution_config, &variables);
                if let Err(err) = result {
                    return Err(Box::new(err))
                }

                Ok(())
            },
            ActionConfig::Confirmation(confirm_config) => {
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
