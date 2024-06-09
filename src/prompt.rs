use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::fmt::{Formatter};
use std::string::FromUtf8Error;
use inquire::{Confirm, InquireError, Password, PasswordDisplayMode, Select, Text};
use crate::config::{ConfirmationCommandActionConfig, PromptConfig, PromptOptionsVariant, SelectOptionsConfig, SelectPromptOptions, TextPromptOptions};
use crate::exec::{CommandExecutor, ExecutionError};

#[derive(Debug)]
pub enum PromptError {
    InquireError(InquireError),
    ExecutionError(ExecutionError),
    ParseError(FromUtf8Error)
}

impl Error for PromptError {}

impl fmt::Display for PromptError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            PromptError::InquireError(err) => write!(f, "failed to execute prompt: {}", err),
            PromptError::ExecutionError(err) => write!(f, "failed to evaluate prompt options: {}", err),
            PromptError::ParseError(err) => write!(f, "failed to parse prompt options: {}", err),
        }
    }
}

pub trait PromptExecutor {
    fn execute(&self, prompt_config: &PromptConfig) -> Result<String, PromptError>;
}

pub struct TerminalPromptExecutor {
    command_executor: Box<dyn CommandExecutor>
}

impl TerminalPromptExecutor {
    pub fn new(command_executor: Box<dyn CommandExecutor>) -> TerminalPromptExecutor {
        return TerminalPromptExecutor{command_executor}
    }
}

impl PromptExecutor for TerminalPromptExecutor {
    fn execute(&self, prompt_config: &PromptConfig) -> Result<String, PromptError> {
        match prompt_config.clone().options {
            PromptOptionsVariant::Text(text_prompt_options) =>
                execute_text_prompt(prompt_config.message.as_str(), &text_prompt_options),
            PromptOptionsVariant::Select(select_prompt_config) =>
                execute_select_prompt(prompt_config.message.as_str(), &select_prompt_config, &self.command_executor),
        }
    }
}

fn execute_text_prompt(message: &str, text_prompt_options: &TextPromptOptions) -> Result<String, PromptError> {
    let result = if text_prompt_options.sensitive {
        Password::new(message)
            .with_display_mode(PasswordDisplayMode::Masked)
            .without_confirmation()
            .prompt()
    } else {
        Text::new(message).prompt()
    };

    match result {
        Ok(value) => Ok(value),
        Err(err) => Err(PromptError::InquireError(err)),
    }
}

fn execute_select_prompt(
    message: &str,
    select_prompt_options: &SelectPromptOptions,
    command_executor: &Box<dyn CommandExecutor>) -> Result<String, PromptError> {
    let options = get_options(&select_prompt_options.options, command_executor)?;
    let result = Select::new(message, options).prompt();
    match result {
        Ok(value) => Ok(value),
        Err(err) => Err(PromptError::InquireError(err)),
    }
}

fn get_options(select_options_config: &SelectOptionsConfig, command_executor: &Box<dyn CommandExecutor>) -> Result<Vec<String>, PromptError> {
    match select_options_config {
        SelectOptionsConfig::Literal(options) => {
            Ok(options.clone())
        }
        SelectOptionsConfig::Execution(execution_config) => {
            let output = command_executor.get_output(&execution_config.execution, &HashMap::new())
                .map_err(|err| PromptError::ExecutionError(err))?;
            let stdout = String::from_utf8(output.stdout)
                .map_err(|err| PromptError::ParseError(err))?;
            let options = stdout.clone().lines().map(|s| String::from(s)).collect();
            Ok(options)
        }
    }
}

pub trait ConfirmExecutor {
    fn execute(&self, confirmation_config: &ConfirmationCommandActionConfig) -> Result<bool, PromptError>;
}


pub struct InquireConfirmExecutor { }

impl ConfirmExecutor for InquireConfirmExecutor{
    fn execute(&self, confirmation_config: &ConfirmationCommandActionConfig) -> Result<bool, PromptError> {
        let result = Confirm::new(confirmation_config.confirm.as_str())
            .with_default(false)
            .prompt()
            .map_err(|err| PromptError::InquireError(err))?;

        return Ok(result)
    }
}

// This is hard to write tests for. Fow now, let's assume the Inquire crate has sufficient tests.