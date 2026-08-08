use std::fmt;
use std::fmt::Formatter;
use crate::args::{ArgumentResolver};
use crate::config::{DeferConfig, ExecutionConfigVariant};
use crate::exec::{CommandExecutor, ExecutionError, ExitStatus};
use crate::variables::{VariableMap};
use thiserror::Error;

pub struct DeferExecutor {
    pub command_executor: Box<dyn CommandExecutor>,
}

impl DeferExecutor {
    /// Executes the provided [`DeferConfig`] with the provided [`VariableMap`].
    pub fn execute(
        &self,
        defer_config: &DeferConfig,
        variables: &VariableMap,
    ) -> Result<(), DeferErrors> {
        match defer_config {
            DeferConfig::SingleStep(exec_config) => {
                self.execute_actions(vec![exec_config.clone()], variables)
            }

            DeferConfig::MultiStep(exec_configs) => {
                self.execute_actions(exec_configs.clone(), variables)
            }
        }
    }

    fn execute_actions(
        &self,
        exec_configs: Vec<ExecutionConfigVariant>,
        variables: &VariableMap,
    ) -> Result<(), DeferErrors> {

        let mut errors: Vec<DeferError> = Vec::new();

        for (idx, execution_config) in exec_configs.iter().enumerate() {

            let result = self.command_executor.execute(&execution_config, &variables);

            match result {
                Ok(status) => {
                    match status {
                        ExitStatus::Success => continue,

                        // Re-map non-zero exit codes to errors
                        _ => errors.push(DeferError::StatusCode { index: idx, status }),
                    }
                }
                Err(err) => {
                    errors.push(DeferError::Execution {
                        index: idx,
                        source: err,
                    })
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(DeferErrors { errors })
        }
    }
}

#[derive(Error, Debug)]
pub struct DeferErrors {
    pub errors: Vec<DeferError>,
}

impl fmt::Display for DeferErrors {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        for error in &self.errors {
            writeln!(f, "{}", error)?
        }

        Ok(())
    }
}

#[derive(Error, Debug)]
pub enum DeferError {

    #[error("failed to execute deferred action {index}")]
    Execution {
        index: usize,
        source: ExecutionError,
    },

    // TODO: Reconsider whether a non-zero exit codes should be treated as errors
    #[error("failed to execute deferredaction {index}: {status}")]
    StatusCode { index: usize, status: ExitStatus },
}

#[cfg(test)]
mod tests {
    use std::{io};
    use super::*;
    use crate::{
        args::MockArgumentResolver,
        config::{RawCommandConfigVariant},
        exec::MockCommandExecutor,
    };
    use mockall::{predicate::eq, Sequence};
    use crate::defer::DeferError;

    #[test]
    fn execute_single_step() {
        // Arrange
        let mut variables = VariableMap::new();
        variables.insert("name".to_string(), "Dingus".to_string());

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
        let defer = DeferConfig::SingleStep(
            ExecutionConfigVariant::RawCommand(RawCommandConfigVariant::Shorthand(
                command_text.to_string(),
            ))
        );

        let defer_executor = DeferExecutor {
            command_executor: Box::new(command_executor),
        };

        let result = defer_executor.execute(&defer, &variables.clone());

        // Assert
        assert!(result.is_ok())
    }

    #[test]
    fn execute_multi_step() {
        // Arrange
        let mut variables = VariableMap::new();
        variables.insert("name".to_string(), "Dingus".to_string());

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
        let defer = DeferConfig::MultiStep(vec![
                ExecutionConfigVariant::RawCommand(RawCommandConfigVariant::Shorthand(
                    command_text_1.to_string(),
                )),
                ExecutionConfigVariant::RawCommand(RawCommandConfigVariant::Shorthand(
                    command_text_2.to_string(),
                )),
                ExecutionConfigVariant::RawCommand(RawCommandConfigVariant::Shorthand(
                    command_text_3.to_string(),
                )),
            ]
        );

        let defer_executor = DeferExecutor {
            command_executor: Box::new(command_executor),
        };

        let result = defer_executor.execute(&defer, &variables.clone());

        // Assert
        assert!(result.is_ok())
    }

    #[test]
    fn all_errors_returned() {
        // Arrange
        let variables = VariableMap::new();
        let command_text_1 = "exit 1";
        let command_text_2 = "exit 2";
        let command_text_3 = "ls";

        let mut seq = Sequence::new();
        let mut command_executor = MockCommandExecutor::new();

        // Command 1: Exit-code 1
        command_executor
            .expect_execute()
            .once()
            .in_sequence(&mut seq)
            .with(
                eq(ExecutionConfigVariant::RawCommand(
                    RawCommandConfigVariant::Shorthand(command_text_1.to_string()),
                )),
                eq(variables.clone()),
            )
            .returning(|_, _| Ok(ExitStatus::Fail(1)));

        // Command 2: IO error
        command_executor
            .expect_execute()
            .once()
            .in_sequence(&mut seq)
            .with(
                eq(ExecutionConfigVariant::RawCommand(
                    RawCommandConfigVariant::Shorthand(command_text_2.to_string()),
                )),
                eq(variables.clone()),
            )
            .returning(|_, _| Err(ExecutionError::IO(io::Error::new(io::ErrorKind::NotFound, "blah"))));

        // Command 3: Success
        command_executor
            .expect_execute()
            .once()
            .in_sequence(&mut seq)
            .with(
                eq(ExecutionConfigVariant::RawCommand(
                    RawCommandConfigVariant::Shorthand(command_text_3.to_string()),
                )),
                eq(variables.clone()),
            )
            .returning(|_, _| Ok(ExitStatus::Success));

        let mut arg_resolver = MockArgumentResolver::new();
        arg_resolver.expect_get_many().times(0).returning(|_| None);

        // Act
        let defer = DeferConfig::MultiStep(vec![
                ExecutionConfigVariant::RawCommand(RawCommandConfigVariant::Shorthand(
                    command_text_1.to_string(),
                )),
                ExecutionConfigVariant::RawCommand(RawCommandConfigVariant::Shorthand(
                    command_text_2.to_string(),
                )),
                ExecutionConfigVariant::RawCommand(RawCommandConfigVariant::Shorthand(
                    command_text_3.to_string(),
                ))
            ]
        );

        let defer_executor = DeferExecutor {
            command_executor: Box::new(command_executor),
        };

        let result = defer_executor.execute(&defer, &variables.clone());

        // Assert
        assert!(result.is_err());
        let err = result.unwrap_err();

        assert_eq!(
            err.to_string(),
            DeferErrors {
                errors: vec![
                    DeferError::StatusCode { index: 0, status: ExitStatus::Fail(1) },
                    DeferError::Execution { index: 1, source: ExecutionError::IO(io::Error::new(io::ErrorKind::NotFound, "blah")) }
                ]
            }.to_string());
    }
}
