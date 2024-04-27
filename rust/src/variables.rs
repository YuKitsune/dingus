use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::process::ExitStatus;
use clap::ArgMatches;
use crate::definitions::VariableDefinition;
use crate::shell::ShellExecutor;
use crate::prompt::{PromptExecutor, SelectExecutor};

pub type Variables = HashMap<String, String>;

pub struct VariableResolver {
    pub shell_executor: Box<dyn ShellExecutor>,
    pub prompt_executor: PromptExecutor,
    pub select_executor: SelectExecutor
}

impl VariableResolver {
    pub fn resolve_variables(
        &self,
        variable_definitions: &HashMap<String, VariableDefinition>,
        arg_matches: &ArgMatches) -> Result<Variables, Box<dyn Error>> {
        variable_definitions.iter()
            .map(|(key, definition)| -> Result<(String, String), Box<dyn Error>> {
                return match definition {
                    VariableDefinition::Literal(value) => {
                        Ok((key.clone(), value.clone()))
                    }
                    VariableDefinition::Invocation(execution) => {
                        let output = self.shell_executor.as_ref().get_output(&execution.clone().exec)?;

                        if !output.status.success() {
                            return Err(Box::new(VariableResolutionError::UnsuccessfulShellExecution(output.status.clone())));
                        }

                        // TODO: Add an option to fail resolution if anything was send to stderr
                        // if !output.stderr.is_empty() {

                        // }

                        let value = String::from_utf8(output.stdout)?;
                        Ok((key.clone(), value.clone()))
                    }
                    VariableDefinition::Prompt(prompt_definition) => {

                        if let Some(flag_value) = get_argument_value(key.clone(), arg_matches) {
                            return Ok((key.clone(), flag_value.clone()))
                        }

                        if let Some(flag) = prompt_definition.clone().prompt.flag {
                            if let Some(flag_value) = get_argument_value(flag, arg_matches) {
                                return Ok((key.clone(), flag_value.clone()))
                            }
                        }

                        let value = self.prompt_executor.execute(&prompt_definition.clone().prompt)?;
                        Ok((key.clone(), value.clone()))
                    }
                    VariableDefinition::Select(select_definition) => {
                        if let Some(flag_value) = get_argument_value(key.clone(), arg_matches) {
                            return Ok((key.clone(), flag_value.clone()))
                        }

                        if let Some(flag) = select_definition.clone().select.flag {
                            if let Some(flag_value) = get_argument_value(flag, arg_matches) {
                                return Ok((key.clone(), flag_value.clone()))
                            }
                        }

                        let value = self.select_executor.execute(&select_definition.clone().select)?;
                        Ok((key.clone(), value.clone()))
                    }
                }
            })
            .collect()
    }
}

fn get_argument_value(name: String, arg_matches: &ArgMatches) -> Option<String> {
    if let Some(value) = arg_matches.get_one::<String>(name.as_str()) {
        return Some(value.clone())
    }

    None
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