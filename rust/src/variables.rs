use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::process::ExitStatus;
use clap::ArgMatches;
use crate::definitions::VariableDefinition;
use crate::shell::ShellExecutor;
use crate::prompt::{PromptExecutor, SelectExecutor};

pub type Variables = HashMap<String, String>;

pub struct FlagResolver {
    flags: HashMap<String, String>
}

impl FlagResolver {
    pub fn from_arg_matches(arg_matches: &ArgMatches) -> FlagResolver {
        let ids = arg_matches.ids();
        let mut flags = HashMap::new();
        for id in ids {
            if let Some(value) = arg_matches.get_one::<String>(id.as_str()) {
                flags.insert(id.to_string(), value.clone());
            }
        }

        FlagResolver {flags}
    }

    pub fn get(&self, key: &String) -> Option<String> {
        if let Some(value) = self.flags.get(key) {
            return Some(value.clone());
        }

        return None;
    }
}

pub struct VariableResolver {
    pub shell_executor: Box<dyn ShellExecutor>,
    pub prompt_executor: PromptExecutor,
    pub select_executor: SelectExecutor
}

impl VariableResolver {
    pub fn resolve_variables(
        &self,
        variable_definitions: &HashMap<String, VariableDefinition>,
        flag_resolver: &FlagResolver) -> Result<Variables, Box<dyn Error>> {
        variable_definitions.iter()
            .map(|(key, definition)| -> Result<(String, String), Box<dyn Error>> {

                let arg_name = definition.arg_name(key);

                return match definition {
                    VariableDefinition::Literal(value) => {
                        Ok((key.clone(), value.clone()))
                    }
                    VariableDefinition::LiteralExtended(extended_literal_def) => {

                        // Check the args first
                        if let Some(flag_value) = flag_resolver.get(&arg_name) {
                            return Ok((key.clone(), flag_value.clone()))
                        }

                        Ok((key.clone(), extended_literal_def.value.clone()))
                    }
                    VariableDefinition::Execution(execution_def) => {

                        // Check the args first
                        if let Some(flag_value) = flag_resolver.get(&arg_name) {
                            return Ok((key.clone(), flag_value.clone()))
                        }

                        let output = self.shell_executor.as_ref().get_output(&execution_def.clone().shell_command)?;

                        if !output.status.success() {
                            return Err(Box::new(VariableResolutionError::UnsuccessfulShellExecution(output.status.clone())));
                        }

                        // TODO: Add an option to fail resolution if anything was send to stderr
                        // if !output.stderr.is_empty() {

                        // }

                        let value = String::from_utf8(output.stdout)?;
                        Ok((key.clone(), value.clone()))
                    }
                    VariableDefinition::Prompt(prompt_def) => {

                        // Check the flags first
                        if let Some(flag_value) = flag_resolver.get(&arg_name) {
                            return Ok((key.clone(), flag_value.clone()))
                        }

                        let value = self.prompt_executor.execute(&prompt_def.clone().prompt)?;
                        Ok((key.clone(), value.clone()))
                    }
                    VariableDefinition::Select(select_def) => {

                        // Check the flags first
                        if let Some(flag_value) = flag_resolver.get(&arg_name) {
                            return Ok((key.clone(), flag_value.clone()))
                        }

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