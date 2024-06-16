use std::error::Error;
use std::fmt;
use crate::config::{ActionConfig};
use crate::exec::{CommandExecutor, ExecutionError, ExitStatus};
use crate::variables::{VariableMap};

pub struct ActionExecutor {
    pub command_executor: Box<dyn CommandExecutor>,
}

impl ActionExecutor {

    /// Executes the provided [`ActionConfig`] with the provided [`VariableMap`].
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
                            return Err(ActionError::new(idx, ActionErrorKind::StatusCode(status)))
                        },
                    }
                }
                Err(err) => {
                    return Err(ActionError::new(idx, ActionErrorKind::ExecutionError(err)))
                }
            }
        }

        return Ok(())
    }
}

#[derive(Debug)]
pub enum ActionErrorKind {
    ExecutionError(ExecutionError),
    StatusCode(ExitStatus)
}

#[derive(Debug)]
pub struct ActionError {
    index: usize,
    kind: ActionErrorKind
}

impl ActionError {
    pub fn new(index: usize, kind: ActionErrorKind) -> ActionError {
        ActionError {
            index,
            kind
        }
    }
}

impl Error for ActionError {}

impl fmt::Display for ActionError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self.kind {
            ActionErrorKind::ExecutionError(err) => write!(f, "failed to execute action {}: {}", self.index, err),
            ActionErrorKind::StatusCode(err) => write!(f, "failed to execute action {}: {}", self.index, err),
        }
    }
}
