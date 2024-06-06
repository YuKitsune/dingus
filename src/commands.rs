use std::error::Error;
use std::fmt;
use crate::config::{ActionConfig, VariableConfigMap};
use crate::exec::CommandExecutor;
use crate::prompt::ConfirmExecutor;
use crate::variables::VariableResolver;

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
            .map_err(|err| ActionError::new(action_id.clone(), err))?;

        return match action_config {
            ActionConfig::Execution(execution_config) => {
                let result = self.command_executor.execute(&execution_config, &variables);
                if let Err(err) = result {
                    return Err(ActionError::new(action_id.clone(), Box::new(err)))
                }

                Ok(())
            },
            ActionConfig::Confirmation(confirm_config) => {
                let result = self.confirm_executor.execute(confirm_config)
                    .map_err(|err| ActionError::new(action_id.clone(), Box::new(err)))?;
                if result == false {
                    return Err(ActionError::new(action_id, Box::new(ConfirmationError)))
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
            ActionKey::Named(action_name) => write!(f, "{}/[\"{}\"]", self.command_name, action_name),
            ActionKey::Unnamed(action_index) => write!(f, "{}[{}]", self.command_name, action_index)
        }
    }
}

#[derive(Debug, Clone)]
pub enum ActionKey {
    Named(String),
    Unnamed(usize)
}

#[derive(Debug)]
pub struct ActionError {
    action_id: ActionId,
    inner: Box<dyn Error>
}

impl Error for ActionError {}

impl ActionError {
    fn new(id: ActionId, error: Box<dyn Error>) -> Box<ActionError> {
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
struct ConfirmationError;

impl fmt::Display for ConfirmationError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "confirmation resulted in a negative result")
    }
}

impl Error for ConfirmationError { }
