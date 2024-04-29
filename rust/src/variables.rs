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