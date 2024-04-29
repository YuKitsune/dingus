use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::process::ExitStatus;
use clap::ArgMatches;
use crate::config::{VariableConfig};
use crate::shell::{ShellExecutorFactory};
use crate::prompt::{PromptExecutor, SelectExecutor};

pub type Variables = HashMap<String, String>;

pub struct ArgumentResolver {
    args: HashMap<String, String>
}

impl ArgumentResolver {
    pub fn from_arg_matches(arg_matches: &ArgMatches) -> ArgumentResolver {
        let ids = arg_matches.ids();
        let mut args = HashMap::new();
        for id in ids {
            if let Some(value) = arg_matches.get_one::<String>(id.as_str()) {
                args.insert(id.to_string(), value.clone());
            }
        }

        ArgumentResolver {args}
    }

    pub fn get(&self, key: &String) -> Option<String> {
        if let Some(value) = self.args.get(key) {
            return Some(value.clone());
        }

        return None;
    }
}

pub struct VariableResolver {
    pub shell_executor_factory: Box<dyn ShellExecutorFactory>,
    pub prompt_executor: PromptExecutor,
    pub select_executor: SelectExecutor,
    pub argument_resolver: ArgumentResolver
}

impl VariableResolver {
    pub fn resolve_variables(
        &self,
        variable_configs: &HashMap<String, VariableConfig>) -> Result<Variables, Box<dyn Error>> {
        variable_configs.iter()
            .map(|(key, config)| -> Result<(String, String), Box<dyn Error>> {

                let arg_name = config.arg_name(key);

                // Check the args first
                if let Some(arg_value) = self.argument_resolver.get(&arg_name) {
                    return Ok((key.clone(), arg_value.clone()))
                }

                return match config {
                    VariableConfig::Literal(value) => Ok((key.clone(), value.clone())),

                    VariableConfig::LiteralExtended(extended_literal_def) =>
                        Ok((key.clone(), extended_literal_def.value.clone())),

                    VariableConfig::Execution(execution_def) => {

                        let shell_executor = match &execution_def.execution.shell {
                            Some(shell) => self.shell_executor_factory.create(shell),
                            None => self.shell_executor_factory.create_default(),
                        };

                        let output = shell_executor.get_output(&execution_def.execution.shell_command)?;

                        if !output.status.success() {
                            return Err(Box::new(VariableResolutionError::UnsuccessfulShellExecution(output.status.clone())));
                        }

                        // TODO: Add an option to fail resolution if anything was send to stderr
                        // if !output.stderr.is_empty() {

                        // }

                        let value = String::from_utf8(output.stdout)?;
                        Ok((key.clone(), value.clone()))
                    }

                    VariableConfig::Prompt(prompt_def) => {
                        let value = self.prompt_executor.execute(&prompt_def.clone().prompt)?;
                        Ok((key.clone(), value.clone()))
                    }

                    VariableConfig::Select(select_def) => {
                        let value = self.select_executor.execute(&select_def.clone().select)?;
                        Ok((key.clone(), value.clone()))
                    }
                }
            })
            .collect()
    }
}

#[derive(Debug)]
enum VariableResolutionError {
    UnsuccessfulShellExecution(ExitStatus)
}

impl Error for VariableResolutionError {}

impl fmt::Display for VariableResolutionError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            VariableResolutionError::UnsuccessfulShellExecution(exit_status) => write!(f, "shell command failed: {}", exit_status),
        }
    }
}

#[cfg(test)]
mod tests {
    use clap::{Arg, Command};
    use crate::variables::ArgumentResolver;

    #[test]
    fn argresolver_resolves_arg() {

        // Arrange
        let arg = arg_long(&"name".to_string());

        // Act
        let value = "Dingus";
        let matches = Command::new("shiji")
            .arg(arg)
            .get_matches_from(vec!["shiji", "--name", value]);

        let arg_resolver = ArgumentResolver::from_arg_matches(&matches);

        // Assert
        let found_value = arg_resolver.get(&"name".to_string());
        assert_eq!(found_value, Some(value.to_string()));
    }

    #[test]
    fn argresolver_resolves_arg_from_subcommand() {

        // Arrange
        let arg = arg_long(&"name".to_string());
        let greet_command = Command::new("greet")
            .arg(arg);

        let root_command = Command::new("shiji")
            .subcommand(greet_command);

        let value = "Dingus";
        let root_matches = root_command.get_matches_from(vec!["shiji", "greet", "--name", value]);
        let (subcommand_name, subcommand_matches) = root_matches.subcommand().unwrap();
        assert_eq!(subcommand_name, "greet");

        let arg_resolver = ArgumentResolver::from_arg_matches(&subcommand_matches);

        // Assert
        let found_value = arg_resolver.get(&"name".to_string());
        assert_eq!(found_value, Some(value.to_string()));
    }

    #[test]
    fn argresolver_resolves_multiple_args() {

        // Arrange
        let name_arg = arg_long(&"name".to_string());
        let age_arg = arg_long(&"age".to_string());
        let greet_command = Command::new("greet")
            .arg(name_arg)
            .arg(age_arg);

        let root_command = Command::new("shiji")
            .subcommand(greet_command);

        // Act
        let name_value = "Dingus";
        let age_value = "42";
        let root_matches = root_command.get_matches_from(vec!["shiji", "greet", "--name", name_value, "--age", age_value]);
        let (subcommand_name, subcommand_matches) = root_matches.subcommand().unwrap();
        assert_eq!(subcommand_name, "greet");

        let arg_resolver = ArgumentResolver::from_arg_matches(&subcommand_matches);

        // Assert
        let found_name_value = arg_resolver.get(&"name".to_string());
        assert_eq!(found_name_value, Some(name_value.to_string()));

        let found_age_value = arg_resolver.get(&"age".to_string());
        assert_eq!(found_age_value, Some(age_value.to_string()));
    }

    fn arg_long(name: &String) -> Arg {
        return Arg::new(name.clone())
            .long(name.clone());
    }
}