use std::error::Error;
use std::fmt;
use crate::args::{ALIAS_ARGS_NAME, ArgumentResolver};
use crate::config::{ActionConfig, AliasActionConfig, ExecutionConfigVariant};
use crate::config::RawCommandConfigVariant::Shorthand;
use crate::exec::{CommandExecutor, ExecutionError, ExitStatus};
use crate::variables::{substitute_variables, VariableMap};

pub struct ActionExecutor {
    pub command_executor: Box<dyn CommandExecutor>,
    pub arg_resolver: Box<dyn ArgumentResolver>
}

impl ActionExecutor {

    /// Executes the provided [`ActionConfig`] with the provided [`VariableMap`].
    pub fn execute(
        &self,
        action_config: &ActionConfig,
        variables: &VariableMap
    ) -> Result<(), ActionError> {
        match action_config {
            ActionConfig::SingleStep(single_command_action) =>
                self.execute_actions(vec![single_command_action.action.clone()], variables),

            ActionConfig::MultiStep(multi_command_action) =>
                self.execute_actions(multi_command_action.actions.clone(), variables),

            ActionConfig::Alias(alias_action) =>
                self.execute_alias(alias_action, variables)
        }
    }

    fn execute_actions(&self, exec_configs: Vec<ExecutionConfigVariant>, variables: &VariableMap) -> Result<(), ActionError> {
        for (idx, execution_config) in exec_configs.iter().enumerate() {

            let result = self.command_executor.execute(&execution_config, &variables);

            match result {
                Ok(status) => {
                    match status {
                        ExitStatus::Success => continue,

                        // Re-map non-zero exit codes to errors
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

    fn execute_alias(&self, alias_action_config: &AliasActionConfig, variables: &VariableMap) -> Result<(), ActionError> {

        // Replace variables in the alias text
        let alias_text = substitute_variables(alias_action_config.alias.as_str(), variables);

        // Get the args and append them to the alias
        let args = self.arg_resolver.get_many(&ALIAS_ARGS_NAME.to_string()).expect("couldn't find alias args");
        let joined_args: String = args.join(" ");
        let full_command_text = format!("{} {}", alias_text, joined_args);

        // Execute it!
        let exec = ExecutionConfigVariant::RawCommand(Shorthand(full_command_text));
        self.command_executor.execute(&exec, variables)
            .map_err(|err| ActionError::new(0, ActionErrorKind::ExecutionError(err)))?;

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
