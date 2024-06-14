use std::error::Error;
use std::fmt;
use crate::config::{ActionConfig};
use crate::exec::{CommandExecutor, ExecutionError, ExitStatus};
use crate::variables::{VariableMap};

pub struct ActionExecutor {
    pub command_executor: Box<dyn CommandExecutor>,
}

impl ActionExecutor {
    pub fn execute(
        &self,
        action_config: &ActionConfig,
        variables: &VariableMap
    ) -> Result<(), ActionError> {

        // Coalesce single actions into multistep actions.
        // Makes the execution part easier.
        let actions = match action_config {
            ActionConfig::SingleStep(single_command_action) =>
                vec![single_command_action.action.clone()],

            ActionConfig::MultiStep(multi_command_action) =>
                multi_command_action.actions.clone()
        };

        for (idx, execution_config) in actions.iter().enumerate() {

            let result = self.command_executor.execute(&execution_config, &variables);

            match result {
                Ok(status) => {
                    match status {
                        ExitStatus::Success => continue,
                        _ => {
                            return Err(ActionError::StatusCode((idx, status)))
                        },
                    }
                }
                Err(err) => {
                    return Err(ActionError::ExecutionError((idx, err)))
                }
            }
        }

        return Ok(())
    }
}

#[derive(Debug)]
pub enum ActionError {
    ExecutionError((usize, ExecutionError)),
    StatusCode((usize, ExitStatus))
}
impl Error for ActionError {}

impl fmt::Display for ActionError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ActionError::ExecutionError((idx, err)) => write!(f, "failed to execute action {}: {}", idx, err),
            ActionError::StatusCode((idx, err)) => write!(f, "failed to execute action {}: {}", idx, err),
        }
    }
}
