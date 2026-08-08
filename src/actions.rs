use crate::args::{ArgumentResolver, ALIAS_ARGS_NAME};
use crate::config::RawCommandConfigVariant::Shorthand;
use crate::config::{ActionConfig, AliasActionConfig, ExecutionConfigVariant};
use crate::exec::{CommandExecutor, ExecutionError, ExitStatus};
use crate::variables::{substitute_variables, VariableMap};
use thiserror::Error;

pub struct ActionExecutor {
    pub command_executor: Box<dyn CommandExecutor>,
    pub arg_resolver: Box<dyn ArgumentResolver>,
}

impl ActionExecutor {
    /// Executes the provided [`ActionConfig`] with the provided [`VariableMap`].
    pub fn execute(
        &self,
        action_config: &ActionConfig,
        variables: &VariableMap,
    ) -> Result<(), ActionError> {
        match action_config {
            ActionConfig::SingleStep(single_command_action) => {
                self.execute_actions(vec![single_command_action.action.clone()], variables)
            }

            ActionConfig::MultiStep(multi_command_action) => {
                self.execute_actions(multi_command_action.actions.clone(), variables)
            }

            ActionConfig::Alias(alias_action) => self.execute_alias(alias_action, variables),
        }
    }

    fn execute_actions(
        &self,
        exec_configs: Vec<ExecutionConfigVariant>,
        variables: &VariableMap,
    ) -> Result<(), ActionError> {
        for (idx, execution_config) in exec_configs.iter().enumerate() {
            let result = self.command_executor.execute(&execution_config, &variables);

            match result {
                Ok(status) => {
                    match status {
                        ExitStatus::Success => continue,

                        // Re-map non-zero exit codes to errors
                        _ => return Err(ActionError::StatusCode { index: idx, status }),
                    }
                }
                Err(err) => {
                    return Err(ActionError::Execution {
                        index: idx,
                        source: err,
                    })
                }
            }
        }

        Ok(())
    }

    fn execute_alias(
        &self,
        alias_action_config: &AliasActionConfig,
        variables: &VariableMap,
    ) -> Result<(), ActionError> {
        // Replace variables in the alias text
        let alias_text = substitute_variables(alias_action_config.alias.as_str(), variables);

        // Get the args and append them to the alias
        let command_text =
            if let Some(args) = self.arg_resolver.get_many(&ALIAS_ARGS_NAME.to_string()) {
                let joined_args: String = args.join(" ");
                format!("{} {}", alias_text, joined_args)
            } else {
                alias_text
            };

        // Execute it!
        let exec = ExecutionConfigVariant::RawCommand(Shorthand(command_text));
        self.command_executor
            .execute(&exec, variables)
            .map_err(|err| ActionError::Execution {
                index: 0,
                source: err,
            })?;

        return Ok(());
    }
}

#[derive(Error, Debug)]
pub enum ActionError {
    #[error("failed to execute action {index}")]
    Execution {
        index: usize,
        source: ExecutionError,
    },

    // TODO: Reconsider whether a non-zero exit codes should be treated as errors
    #[error("failed to execute action {index}: {status}")]
    StatusCode { index: usize, status: ExitStatus },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        args::MockArgumentResolver,
        config::{MultiActionConfig, RawCommandConfigVariant, SingleActionConfig},
        exec::MockCommandExecutor,
    };
    use mockall::{predicate::eq, Sequence};

    #[test]
    fn execute_single_step() {
        // Arrange
        let mut variables = VariableMap::new();
        variables.insert("name".to_string(), "Alice".to_string());

        let command_text = "echo Hello, $name!";

        let mut command_executor = MockCommandExecutor::new();
        command_executor
            .expect_execute()
            .times(1)
            .with(
                eq(ExecutionConfigVariant::RawCommand(
                    RawCommandConfigVariant::Shorthand(command_text.to_string()),
                )),
                eq(variables.clone()),
            )
            .returning(|_, _| Ok(ExitStatus::Success));

        let mut arg_resolver = MockArgumentResolver::new();
        arg_resolver.expect_get_many().times(0).returning(|_| None);

        // Act
        let action = ActionConfig::SingleStep(SingleActionConfig {
            action: ExecutionConfigVariant::RawCommand(RawCommandConfigVariant::Shorthand(
                command_text.to_string(),
            )),
        });

        let action_executor = ActionExecutor {
            command_executor: Box::new(command_executor),
            arg_resolver: Box::new(arg_resolver),
        };

        let result = action_executor.execute(&action, &variables.clone());

        // Assert
        assert!(result.is_ok())
    }

    #[test]
    fn execute_multi_step() {
        // Arrange
        let mut variables = VariableMap::new();
        variables.insert("name".to_string(), "Alice".to_string());

        let command_text_1 = "echo Hello, $name!";
        let command_text_2 = "echo Deleting your boot sector...";
        let command_text_3 = "echo Goodbye, $name!";

        let commands = vec![command_text_1, command_text_2, command_text_3];

        let mut seq = Sequence::new();
        let mut command_executor = MockCommandExecutor::new();

        for command_text in commands {
            command_executor
                .expect_execute()
                .once()
                .in_sequence(&mut seq)
                .with(
                    eq(ExecutionConfigVariant::RawCommand(
                        RawCommandConfigVariant::Shorthand(command_text.to_string()),
                    )),
                    eq(variables.clone()),
                )
                .returning(|_, _| Ok(ExitStatus::Success));
        }

        let mut arg_resolver = MockArgumentResolver::new();
        arg_resolver.expect_get_many().times(0).returning(|_| None);

        // Act
        let action = ActionConfig::MultiStep(MultiActionConfig {
            actions: vec![
                ExecutionConfigVariant::RawCommand(RawCommandConfigVariant::Shorthand(
                    command_text_1.to_string(),
                )),
                ExecutionConfigVariant::RawCommand(RawCommandConfigVariant::Shorthand(
                    command_text_2.to_string(),
                )),
                ExecutionConfigVariant::RawCommand(RawCommandConfigVariant::Shorthand(
                    command_text_3.to_string(),
                )),
            ],
        });

        let action_executor = ActionExecutor {
            command_executor: Box::new(command_executor),
            arg_resolver: Box::new(arg_resolver),
        };

        let result = action_executor.execute(&action, &variables.clone());

        // Assert
        assert!(result.is_ok())
    }

    #[test]
    fn execute_alias() {
        // Arrange
        let mut variables = VariableMap::new();
        variables.insert("name".to_string(), "Alice".to_string());

        let command_text = "docker compose";

        let mut command_executor = MockCommandExecutor::new();
        command_executor
            .expect_execute()
            .times(1)
            .with(
                eq(ExecutionConfigVariant::RawCommand(
                    RawCommandConfigVariant::Shorthand("docker compose up -d".to_string()),
                )),
                eq(variables.clone()),
            )
            .returning(|_, _| Ok(ExitStatus::Success));

        let alias_text = "up -d";
        let mut arg_resolver = MockArgumentResolver::new();
        arg_resolver
            .expect_get_many()
            .with(eq(ALIAS_ARGS_NAME.to_string()))
            .once()
            .returning(|_| Some(vec![alias_text.to_string()]));

        // Act
        let action = ActionConfig::Alias(AliasActionConfig {
            alias: command_text.to_string(),
        });

        let action_executor = ActionExecutor {
            command_executor: Box::new(command_executor),
            arg_resolver: Box::new(arg_resolver),
        };

        let result = action_executor.execute(&action, &variables.clone());

        // Assert
        assert!(result.is_ok())
    }
}
