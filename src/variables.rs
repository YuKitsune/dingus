use crate::args::ArgumentResolver;
use crate::config::{DingusOptions, PromptOptionsVariant, VariableConfig, VariableConfigMap};
use crate::exec::{CommandExecutor, ExecutionError, ExitStatus};
use crate::prompt::{PromptError, PromptExecutor};
use colored::Colorize;
use std::collections::HashMap;
use std::string::FromUtf8Error;
use thiserror::Error;

/// A [`HashMap`] where the key is the variable name, and the value is that variables value.
pub type VariableMap = HashMap<String, String>;

pub trait VariableResolver {
    /// Resolves variables from the provided [`VariableConfigMap`] into a [`VariableMap`].
    fn resolve_variables(
        &self,
        variable_configs: &VariableConfigMap,
    ) -> Result<VariableMap, VariableResolutionError>;
}

pub struct RealVariableResolver {
    pub command_executor: Box<dyn CommandExecutor>,
    pub prompt_executor: Box<dyn PromptExecutor>,
    pub argument_resolver: Box<dyn ArgumentResolver>,
    pub dingus_options: DingusOptions,
}

impl VariableResolver for RealVariableResolver {
    fn resolve_variables(
        &self,
        variable_configs: &VariableConfigMap,
    ) -> Result<VariableMap, VariableResolutionError> {
        // The names of sensitive variables are added to a separate vec so that the logging stuff
        // below knows to obfuscate them.
        let mut resolved_variables = VariableMap::new();
        let mut sensitive_variable_names: Vec<String> = vec![];

        for (key, config) in variable_configs.iter() {
            // Args from the command-line have the highest priority, check there first.
            let arg_name = config.arg_name(key);

            let name = config.environment_variable_name(key);

            if let Some(arg_value) = self.argument_resolver.get(&arg_name) {
                resolved_variables.insert(name.clone(), arg_value.clone());
            } else {
                _ = match config {
                    VariableConfig::ShorthandLiteral(value) => {
                        resolved_variables.insert(name.clone(), value.clone());
                    }

                    VariableConfig::Literal(literal_conf) => {
                        resolved_variables.insert(name.clone(), literal_conf.value.clone());
                    }

                    VariableConfig::Execution(execution_conf) => {
                        // Exec variables need access to the variables defined above them.
                        let output = self
                            .command_executor
                            .get_output(&execution_conf.execution, &resolved_variables)
                            .map_err(|err| VariableResolutionError::Execution {
                                key: key.clone(),
                                source: err,
                            })?;

                        // TODO: Make this configurable.
                        // If the command has a non-zero exit code, we probably shouldn't trust it's output.
                        // Return an error instead.
                        if let ExitStatus::Fail(_) = output.status {
                            return Err(VariableResolutionError::ExitStatus {
                                key: key.clone(),
                                status: output.status.clone(),
                            });
                        }

                        let value = String::from_utf8(output.stdout)
                            .map_err(|err| VariableResolutionError::Parse {
                                key: key.clone(),
                                source: err,
                            })?
                            .trim_end()
                            .to_string();

                        resolved_variables.insert(name.clone(), value.clone());
                    }

                    VariableConfig::Prompt(prompt_config) => {
                        let value = self
                            .prompt_executor
                            .execute(&prompt_config.prompt)
                            .map_err(|err| VariableResolutionError::Prompt {
                                key: key.clone(),
                                source: err,
                            })?;

                        resolved_variables.insert(name.clone(), value.clone());

                        if is_variable_sensitive(config) {
                            sensitive_variable_names.push(name.clone());
                        }
                    }
                }
            }
        }

        self.log_variables(&resolved_variables, &sensitive_variable_names);

        Ok(resolved_variables)
    }
}

impl RealVariableResolver {
    fn log_variables(&self, variables: &VariableMap, sensitive_variable_names: &Vec<String>) {
        if !self.dingus_options.print_variables {
            return;
        }

        for (name, value) in variables {
            let is_sensitive = sensitive_variable_names.contains(name);

            let variable_to_print = if is_sensitive {
                "********".to_string() // Hard coded value to obscure the length
            } else {
                value.clone()
            };

            println!("{}={}", name, variable_to_print.green());
        }
    }
}

fn is_variable_sensitive(variable_config: &VariableConfig) -> bool {
    match variable_config {
        VariableConfig::ShorthandLiteral(_) => false,
        VariableConfig::Literal(_) => false,
        VariableConfig::Execution(_) => false,
        VariableConfig::Prompt(prompt_variable) => match prompt_variable.clone().prompt.options {
            PromptOptionsVariant::Select(_) => false,
            PromptOptionsVariant::Text(text_prompt_options) => text_prompt_options.sensitive,
        },
    }
}

/// Uses bash-style variable substitution to replace variable names with their values.
pub fn substitute_variables(template: &str, variables: &VariableMap) -> String {
    let mut result = String::new();
    let mut chars = template.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\\' {
            // Look ahead to the next character
            if let Some(&next_ch) = chars.peek() {
                if next_ch == '$' {
                    // It's an escaped '$', so just append it and consume the next character
                    result.push('$');
                    chars.next();
                } else {
                    // It's a regular backslash, append it
                    result.push(ch);
                }
            } else {
                // It's a single backslash at the end of the string
                result.push(ch);
            }
        } else if ch == '$' {
            // Start of a variable, collect the variable name
            let mut var_name = String::new();
            while let Some(&next_ch) = chars.peek() {
                if next_ch.is_alphanumeric() || next_ch == '_' {
                    var_name.push(next_ch);
                    chars.next();
                } else {
                    break;
                }
            }
            // Substitute the variable if it exists
            if let Some(value) = variables.get(&var_name) {
                result.push_str(value);
            } else {
                // If the variable is not found, leave it as is (including the $ sign)
                result.push('$');
                result.push_str(&var_name);
            }
        } else {
            // Regular character, just append it
            result.push(ch);
        }
    }

    result
}

#[derive(Error, Debug)]
#[error("failed to resolve variable \"{key}\"")]
pub enum VariableResolutionError {
    Execution {
        key: String,
        source: ExecutionError,
    },

    #[error("failed to resolve variable \"{key}\": {status}")]
    ExitStatus {
        key: String,
        status: ExitStatus,
    },

    Parse {
        key: String,
        source: FromUtf8Error,
    },

    Prompt {
        key: String,
        source: PromptError,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::args::MockArgumentResolver;
    use crate::config::VariableConfig::Prompt;
    use crate::config::{
        BashCommandConfig, ExecutionConfigVariant, ExecutionVariableConfig, LiteralVariableConfig,
        PromptConfig, PromptOptionsVariant, PromptVariableConfig, SelectOptionsConfig,
        SelectPromptOptions, ShellCommandConfigVariant, VariableConfig,
    };
    use crate::exec::{ExitStatus, MockCommandExecutor, Output};
    use crate::prompt::MockPromptExecutor;
    use inquire::validator::ErrorMessage::Default;

    #[test]
    fn variable_resolver_resolves_shorthand_literal() {
        // Arrange
        let command_executor = MockCommandExecutor::new();
        let mut argument_resolver = MockArgumentResolver::new();
        argument_resolver
            .expect_get()
            .times(0..)
            .returning(|_| None);

        let prompt_executor = MockPromptExecutor::new();

        let variable_resolver = RealVariableResolver {
            command_executor: Box::new(command_executor),
            prompt_executor: Box::new(prompt_executor),
            argument_resolver: Box::new(argument_resolver),
            dingus_options: Default::default(),
        };

        let name = "name";
        let value = "Dingus";
        let mut variable_configs = VariableConfigMap::new();
        variable_configs.insert(
            name.to_string(),
            VariableConfig::ShorthandLiteral(value.to_string()),
        );

        // Act
        let resolved_variables = variable_resolver.resolve_variables(&variable_configs);

        // Assert
        assert!(!resolved_variables.is_err());

        let binding = resolved_variables.unwrap().clone();
        let resolved_value = binding.get(name).unwrap().as_str();
        assert_eq!(resolved_value, value);
    }

    #[test]
    fn variable_resolver_resolves_literal() {
        // Arrange
        let command_executor = MockCommandExecutor::new();
        let mut argument_resolver = MockArgumentResolver::new();
        argument_resolver
            .expect_get()
            .times(0..)
            .returning(|_| None);
        let prompt_executor = MockPromptExecutor::new();

        let variable_resolver = RealVariableResolver {
            command_executor: Box::new(command_executor),
            prompt_executor: Box::new(prompt_executor),
            argument_resolver: Box::new(argument_resolver),
            dingus_options: Default::default(),
        };

        let name = "name";
        let value = "Dingus";
        let mut variable_configs = VariableConfigMap::new();
        variable_configs.insert(
            name.to_string(),
            VariableConfig::Literal(LiteralVariableConfig {
                value: value.to_string(),
                description: None,
                argument_name: None,
                environment_variable_name: None,
            }),
        );

        // Act
        let resolved_variables = variable_resolver.resolve_variables(&variable_configs);

        // Assert
        assert!(!resolved_variables.is_err());

        let binding = resolved_variables.unwrap().clone();
        let resolved_value = binding.get(name).unwrap().as_str();
        assert_eq!(resolved_value, value);
    }

    #[test]
    fn variable_resolver_resolves_execution_variable() {
        // Arrange
        let value = "Dingus";
        let mut command_executor = MockCommandExecutor::new();
        command_executor.expect_get_output().returning(move |_, _| {
            Ok(Output {
                status: ExitStatus::Success,
                stdout: value.as_bytes().to_vec(),
                stderr: vec![],
            })
        });

        let mut argument_resolver = MockArgumentResolver::new();
        argument_resolver
            .expect_get()
            .times(0..)
            .returning(|_| None);
        let prompt_executor = MockPromptExecutor::new();

        let variable_resolver = RealVariableResolver {
            command_executor: Box::new(command_executor),
            prompt_executor: Box::new(prompt_executor),
            argument_resolver: Box::new(argument_resolver),
            dingus_options: Default::default(),
        };

        let name = "name";
        let mut variable_configs = VariableConfigMap::new();
        variable_configs.insert(
            name.to_string(),
            VariableConfig::Execution(ExecutionVariableConfig {
                description: None,
                argument_name: None,
                environment_variable_name: None,
                execution: ExecutionConfigVariant::ShellCommand(ShellCommandConfigVariant::Bash(
                    BashCommandConfig {
                        working_directory: None,
                        command: format!("echo \"{value}\""),
                    },
                )),
            }),
        );

        // Act
        let resolved_variables = variable_resolver.resolve_variables(&variable_configs);

        // Assert
        assert!(!resolved_variables.is_err());

        let binding = resolved_variables.unwrap().clone();
        let resolved_value = binding.get(name).unwrap().as_str();
        assert_eq!(resolved_value, value);
    }

    #[test]
    fn variable_resolver_resolves_text_prompt_variable() {
        // Arrange
        let command_executor = MockCommandExecutor::new();

        let mut argument_resolver = MockArgumentResolver::new();
        argument_resolver
            .expect_get()
            .times(0..)
            .returning(|_| None);

        let value = "Dingus";
        let mut prompt_executor = MockPromptExecutor::new();
        prompt_executor
            .expect_execute()
            .once()
            .returning(|_| Ok(value.to_string()));

        let variable_resolver = RealVariableResolver {
            command_executor: Box::new(command_executor),
            prompt_executor: Box::new(prompt_executor),
            argument_resolver: Box::new(argument_resolver),
            dingus_options: Default::default(),
        };

        let name = "name";
        let mut variable_configs = VariableConfigMap::new();
        variable_configs.insert(
            name.to_string(),
            Prompt(PromptVariableConfig {
                description: None,
                argument_name: None,
                environment_variable_name: None,
                prompt: PromptConfig {
                    message: "Enter your name".to_string(),
                    options: Default::default(),
                },
            }),
        );

        // Act
        let resolved_variables = variable_resolver.resolve_variables(&variable_configs);

        // Assert
        assert!(!resolved_variables.is_err());

        let binding = resolved_variables.unwrap().clone();
        let resolved_value = binding.get(name).unwrap().as_str();
        assert_eq!(resolved_value, value);
    }

    #[test]
    fn variable_resolver_resolves_select_prompt_variable() {
        // Arrange
        let command_executor = MockCommandExecutor::new();

        let mut argument_resolver = MockArgumentResolver::new();
        argument_resolver
            .expect_get()
            .times(0..)
            .returning(|_| None);

        let value = "Dingus";
        let mut prompt_executor = MockPromptExecutor::new();
        prompt_executor
            .expect_execute()
            .once()
            .returning(|_| Ok(value.to_string()));

        let variable_resolver = RealVariableResolver {
            command_executor: Box::new(command_executor),
            prompt_executor: Box::new(prompt_executor),
            argument_resolver: Box::new(argument_resolver),
            dingus_options: Default::default(),
        };

        let name = "name";
        let mut variable_configs = VariableConfigMap::new();
        variable_configs.insert(
            name.to_string(),
            Prompt(PromptVariableConfig {
                description: None,
                argument_name: None,
                environment_variable_name: None,
                prompt: PromptConfig {
                    message: "Select your name".to_string(),
                    options: PromptOptionsVariant::Select(SelectPromptOptions {
                        options: SelectOptionsConfig::Literal(vec![
                            "Alice".to_string(),
                            "Bob".to_string(),
                            "Charlie".to_string(),
                            "Dingus".to_string(),
                        ]),
                    }),
                },
            }),
        );

        // Act
        let resolved_variables = variable_resolver.resolve_variables(&variable_configs);

        // Assert
        assert!(!resolved_variables.is_err());

        let binding = resolved_variables.unwrap().clone();
        let resolved_value = binding.get(name).unwrap().as_str();
        assert_eq!(resolved_value, value);
    }

    #[test]
    fn variable_resolver_uses_custom_env_var() {
        // Arrange
        let command_executor = MockCommandExecutor::new();
        let mut argument_resolver = MockArgumentResolver::new();
        argument_resolver
            .expect_get()
            .times(0..)
            .returning(|_| None);
        let prompt_executor = MockPromptExecutor::new();

        let variable_resolver = RealVariableResolver {
            command_executor: Box::new(command_executor),
            prompt_executor: Box::new(prompt_executor),
            argument_resolver: Box::new(argument_resolver),
            dingus_options: Default::default(),
        };

        let name = "name";
        let value = "Dingus";
        let env_var_name = "USER_NAME";
        let mut variable_configs = VariableConfigMap::new();
        variable_configs.insert(
            name.to_string(),
            VariableConfig::Literal(LiteralVariableConfig {
                value: value.to_string(),
                description: None,
                argument_name: None,
                environment_variable_name: Some(env_var_name.to_string()),
            }),
        );

        // Act
        let resolved_variables = variable_resolver.resolve_variables(&variable_configs);

        // Assert
        assert!(!resolved_variables.is_err());

        let binding = resolved_variables.unwrap().clone();
        let resolved_value = binding.get(env_var_name).unwrap().as_str();
        assert_eq!(resolved_value, value);
    }

    #[test]
    fn substitute_variables_substitutes_variables() {
        // Arrange
        let template = "Hello, $name! You are $age years old.";
        let mut variables = VariableMap::new();
        variables.insert("name".to_string(), "Dingus".to_string());
        variables.insert("age".to_string(), "100".to_string());

        // Act
        let result = substitute_variables(template, &variables);

        // Assert
        assert_eq!(result, "Hello, Dingus! You are 100 years old.")
    }

    #[test]
    fn substitute_variables_ignores_escaped() {
        // Arrange
        let template = "Hello, $name! You are \\$age years old.";
        let mut variables = VariableMap::new();
        variables.insert("name".to_string(), "Dingus".to_string());
        variables.insert("age".to_string(), "100".to_string());

        // Act
        let result = substitute_variables(template, &variables);

        // Assert
        assert_eq!(result, "Hello, Dingus! You are $age years old.")
    }

    #[test]
    fn substitute_variables_allows_underscores() {
        // Arrange
        let template = "Hello, $first_name $last_name!";
        let mut variables = VariableMap::new();
        variables.insert("first_name".to_string(), "Dingus".to_string());
        variables.insert("last_name".to_string(), "Bingus".to_string());

        // Act
        let result = substitute_variables(template, &variables);

        // Assert
        assert_eq!(result, "Hello, Dingus Bingus!")
    }

    #[test]
    fn substitute_variables_allows_adjacent() {
        // Arrange
        let template = "Hello, $first_name$last_name!";
        let mut variables = VariableMap::new();
        variables.insert("first_name".to_string(), "Dingus".to_string());
        variables.insert("last_name".to_string(), "Bingus".to_string());

        // Act
        let result = substitute_variables(template, &variables);

        // Assert
        assert_eq!(result, "Hello, DingusBingus!")
    }

    #[test]
    fn substitute_variables_does_not_parse_hyphen() {
        // Arrange
        let template = "Hello, $first_name-the-$last_name!";
        let mut variables = VariableMap::new();
        variables.insert("first_name".to_string(), "Dingus".to_string());
        variables.insert("last_name".to_string(), "Bingus".to_string());

        // Act
        let result = substitute_variables(template, &variables);

        // Assert
        assert_eq!(result, "Hello, Dingus-the-Bingus!")
    }
}
