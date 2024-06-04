use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::string::FromUtf8Error;
use crate::args::ArgumentResolver;
use crate::config::{VariableConfig, VariableConfigMap};
use crate::prompt::{PromptError, PromptExecutor};
use crate::exec::{ExitStatus, CommandExecutor, ExecutionError};

pub type VariableMap = HashMap<String, String>;

pub struct VariableResolver {
    pub command_executor: Box<dyn CommandExecutor>,
    pub prompt_executor: Box<dyn PromptExecutor>,
    pub argument_resolver: Box<dyn ArgumentResolver>
}

impl VariableResolver {
    pub fn resolve_variables(
        &self,
        variable_configs: &VariableConfigMap) -> Result<VariableMap, Box<VariableResolutionError>> {
        variable_configs.iter()
            .map(|(key, config)| -> Result<(String, String), Box<VariableResolutionError>> {

                let arg_name = config.arg_name(key);

                // Check the args first
                if let Some(arg_value) = self.argument_resolver.get(&arg_name) {
                    return Ok((key.clone(), arg_value.clone()))
                }

                return match config {
                    VariableConfig::ShorthandLiteral(value) => Ok((key.clone(), value.clone())),

                    VariableConfig::Literal(literal_conf) =>
                        Ok((key.clone(), literal_conf.value.clone())),

                    VariableConfig::Execution(execution_conf) => {

                        let output = self.command_executor.get_output(&execution_conf.execution, &HashMap::new())
                            .map_err(|err| VariableResolutionError::Execution(err))?;

                        if let ExitStatus::Fail(_) = output.status {
                            return Err(Box::new(VariableResolutionError::ExitStatus(output.status.clone())));
                        }

                        let value = String::from_utf8(output.stdout)
                            .map_err(|err| VariableResolutionError::Parse(err))?;
                        let trimmed_value = value.trim_end().to_string();
                        Ok((key.clone(), trimmed_value.clone()))
                    }

                    VariableConfig::Prompt(prompt_config) => {
                        let value = self.prompt_executor.execute(&prompt_config.prompt)
                            .map_err(|err| VariableResolutionError::Prompt(err))?;
                        Ok((key.clone(), value.clone()))
                    }
                }
            })
            .collect()
    }
}

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

#[derive(Debug)]
pub enum VariableResolutionError {
    Execution(ExecutionError),
    ExitStatus(ExitStatus),
    Parse(FromUtf8Error),
    Prompt(PromptError)
}

impl Error for VariableResolutionError {}

impl fmt::Display for VariableResolutionError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            VariableResolutionError::Execution(execution_err) => write!(f, "failed to evaluate variable: {}", execution_err),
            VariableResolutionError::ExitStatus(status) => write!(f, "failed to evaluate variable: {}", status),
            VariableResolutionError::Parse(utf8_err) => write!(f, "failed to evaluate variable: {}", utf8_err),
            VariableResolutionError::Prompt(prompt_err) => write!(f, "failed to evaluate variable: {}", prompt_err)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use crate::args::ArgumentResolver;
    use crate::config::{BashCommandConfig, ExecutionConfigVariant, ExecutionVariableConfig, LiteralVariableConfig, PromptConfig, PromptOptionsVariant, PromptVariableConfig, SelectOptionsConfig, SelectPromptOptions, ShellCommandConfigVariant, VariableConfig};
    use crate::config::VariableConfig::Prompt;
    use crate::prompt::PromptExecutor;
    use crate::exec::{ExitStatus, Output, CommandExecutor};

    #[test]
    fn variable_resolver_resolves_shorthand_literal() {

        // Arrange
        let command_executor = Box::new(MockCommandExecutor { output: Output {
            status: ExitStatus::Success,
            stdout: vec![],
            stderr: vec![],
        }});
        let argument_resolver = Box::new(MockArgumentResolver{ args: HashMap::new()});
        let prompt_executor = Box::new(MockPromptExecutor{ response: None });

        let variable_resolver = VariableResolver{
            command_executor,
            prompt_executor,
            argument_resolver,
        };

        let name = "name";
        let value = "Dingus";
        let mut variable_configs = VariableConfigMap::new();
        variable_configs.insert(name.to_string(), VariableConfig::ShorthandLiteral(value.to_string()));

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
        let command_executor = Box::new(MockCommandExecutor{ output: Output {
            status: ExitStatus::Success,
            stdout: vec![],
            stderr: vec![],
        } });
        let argument_resolver = Box::new(MockArgumentResolver{ args: HashMap::new()});
        let prompt_executor = Box::new(MockPromptExecutor{ response: None });

        let variable_resolver = VariableResolver{
            command_executor,
            prompt_executor,
            argument_resolver,
        };

        let name = "name";
        let value = "Dingus";
        let mut variable_configs = VariableConfigMap::new();
        variable_configs.insert(name.to_string(), VariableConfig::Literal(LiteralVariableConfig{
            value: value.to_string(),
            description: None,
            argument_name: None,
        }));

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
        let command_executor = Box::new(MockCommandExecutor{
            output: Output {
                status: ExitStatus::Success,
                stdout: format!("{value}\n").as_bytes().to_vec(),
                stderr: vec![],
            }
        });
        let argument_resolver = Box::new(MockArgumentResolver{ args: HashMap::new()});
        let prompt_executor = Box::new(MockPromptExecutor{ response: None });

        let variable_resolver = VariableResolver{
            command_executor,
            prompt_executor,
            argument_resolver,
        };

        let name = "name";
        let mut variable_configs = VariableConfigMap::new();
        variable_configs.insert(
            name.to_string(),
            VariableConfig::Execution(ExecutionVariableConfig {
                description: None,
                argument_name: None,
                execution: ExecutionConfigVariant::ShellCommand(
                    ShellCommandConfigVariant::Bash(BashCommandConfig {
                        working_directory: None,
                        command: format!("echo \"{value}\"")
                    })
                ),
            })
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
        let command_executor = Box::new(MockCommandExecutor{ output: Output {
            status: ExitStatus::Success,
            stdout: vec![],
            stderr: vec![],
        } });
        let argument_resolver = Box::new(MockArgumentResolver{ args: HashMap::new()});

        let value = "Dingus";
        let prompt_executor = Box::new(MockPromptExecutor{ response: Some(value.to_string()) });

        let variable_resolver = VariableResolver{
            command_executor,
            prompt_executor,
            argument_resolver,
        };

        let name = "name";
        let mut variable_configs = VariableConfigMap::new();
        variable_configs.insert(name.to_string(), Prompt(PromptVariableConfig{
            description: None,
            argument_name: None,
            prompt: PromptConfig { message: "Enter your name".to_string(), options: Default::default() },
        }));

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
        let command_executor = Box::new(MockCommandExecutor{ output: Output {
            status: ExitStatus::Success,
            stdout: vec![],
            stderr: vec![],
        } });
        let argument_resolver = Box::new(MockArgumentResolver{ args: HashMap::new()});

        let value = "Dingus";
        let prompt_executor = Box::new(MockPromptExecutor{ response: Some(value.to_string()) });

        let variable_resolver = VariableResolver{
            command_executor,
            prompt_executor,
            argument_resolver,
        };

        let name = "name";
        let mut variable_configs = VariableConfigMap::new();
        variable_configs.insert(name.to_string(), Prompt(PromptVariableConfig{
            description: None,
            argument_name: None,
            prompt: PromptConfig {
                message: "Select your name".to_string(),
                options: PromptOptionsVariant::Select(SelectPromptOptions{
                    options: SelectOptionsConfig::Literal(vec!["Alice".to_string(), "Bob".to_string(), "Charlie".to_string(), "Dingus".to_string()])
                }),
            },
        }));

        // Act
        let resolved_variables = variable_resolver.resolve_variables(&variable_configs);

        // Assert
        assert!(!resolved_variables.is_err());

        let binding = resolved_variables.unwrap().clone();
        let resolved_value = binding.get(name).unwrap().as_str();
        assert_eq!(resolved_value, value);
    }

    #[test]
    fn substitute_variables_substitutes_variables(){

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
    fn substitute_variables_ignores_escaped(){

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
    fn substitute_variables_allows_underscores(){

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
    fn substitute_variables_allows_adjacent(){

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
    fn substitute_variables_does_not_parse_hyphen(){

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

    struct MockCommandExecutor {
        output: Output
    }

    impl CommandExecutor for MockCommandExecutor {
        fn execute(&self, _: &ExecutionConfigVariant, _: &VariableMap) -> crate::exec::ExecutionResult {
            Ok(())
        }

        fn get_output(&self, _: &ExecutionConfigVariant, _: &VariableMap) -> crate::exec::ExecutionOutputResult {
            Ok(self.output.clone())
        }
    }

    struct MockArgumentResolver {
        args: HashMap<String, String>
    }

    impl ArgumentResolver for MockArgumentResolver {
        fn get(&self, key: &String) -> Option<String> {
            if let Some(value) = self.args.get(key) {
                return Some(value.clone())
            }

            return None;
        }
    }

    struct MockPromptExecutor {
        response: Option<String>
    }

    impl PromptExecutor for MockPromptExecutor {
        fn execute(&self, _: &PromptConfig) -> Result<String, PromptError> {
            Ok(self.response.clone().unwrap())
        }
    }
}