use std::error::Error;
use std::fmt;
use crate::config::{ActionConfig, VariableConfigMap};
use crate::exec::{CommandExecutor, ExecutionError, ExitStatus};
use crate::prompt::{ConfirmExecutor, PromptError};
use crate::variables::{VariableResolutionError, VariableResolver};

pub struct ActionExecutor {
    pub command_executor: Box<dyn CommandExecutor>,
    pub confirm_executor: ConfirmExecutor,
    pub variable_resolver: VariableResolver
}

impl ActionExecutor {
    pub fn execute(
        &self,
        action_id: ActionId,
        action_config: &ActionConfig,
        variable_config_map: &VariableConfigMap,
    ) -> Result<(), Box<ActionError>> {

        let variables = self.variable_resolver.resolve_variables(variable_config_map)
            .map_err(|err| ActionError::new(action_id.clone(), InnerActionError::VariableResolutionError(err)))?;

        return match action_config {
            ActionConfig::Execution(execution_config) => {
                let result = self.command_executor.execute(&execution_config, &variables);

                return match result {
                    Ok(status) => {
                        return match status {
                            ExitStatus::Success => Ok(()),
                            _ => Err(ActionError::new(action_id.clone(), InnerActionError::StatusCode(status))),
                        }
                    }
                    Err(err) => Err(ActionError::new(action_id.clone(), InnerActionError::ExecutionError(err)))
                }
            },
            ActionConfig::Confirmation(confirm_config) => {
                let result = self.confirm_executor.execute(confirm_config)
                    .map_err(|err| ActionError::new(action_id.clone(), InnerActionError::PromptError(err)))?;
                if result == false {
                    return Err(ActionError::new(action_id, InnerActionError::ConfirmationError(ConfirmationError)))
                }

                Ok(())
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct ActionId {
    pub command_name: String,
    pub action: ActionKey
}

impl fmt::Display for ActionId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.action.clone() {
            // ActionKey::Named(action_name) => write!(f, "{}/[\"{}\"]", self.command_name, action_name),
            ActionKey::Unnamed(action_index) => write!(f, "{}[{}]", self.command_name, action_index)
        }
    }
}

#[derive(Debug, Clone)]
pub enum ActionKey {
    Unnamed(usize)
}

#[derive(Debug)]
pub enum InnerActionError {
    VariableResolutionError(VariableResolutionError),
    ExecutionError(ExecutionError),
    StatusCode(ExitStatus),
    ConfirmationError(ConfirmationError),
    PromptError(PromptError)
}

impl Error for InnerActionError {}

impl fmt::Display for InnerActionError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            InnerActionError::VariableResolutionError(err) => write!(f, "{}", err),
            InnerActionError::ExecutionError(err) => write!(f, "{}", err),
            InnerActionError::StatusCode(err) => write!(f, "{}", err),
            InnerActionError::ConfirmationError(err) => write!(f, "{}", err),
            InnerActionError::PromptError(err) => write!(f, "{}", err),
        }
    }
}


#[derive(Debug)]
pub struct ActionError {
    action_id: ActionId,
    inner: InnerActionError
}

impl Error for ActionError {}

impl ActionError {
    fn new(id: ActionId, error: InnerActionError) -> Box<ActionError> {
        return Box::new(ActionError {
            action_id: id,
            inner: error
        })
    }
}

impl fmt::Display for ActionError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "failed to execute action {}: {}", self.action_id.clone(), self.inner)
    }
}

#[derive(Debug, Clone)]
pub struct ConfirmationError;

impl fmt::Display for ConfirmationError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "confirmation resulted in a negative result")
    }
}

impl Error for ConfirmationError { }

// Todo: Tests
// - Fails when variable resolution fails
// - Executes command with resolved variables
// - Command failures are propagated