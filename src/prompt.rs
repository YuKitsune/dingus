use crate::config::{
    PromptConfig, PromptOptionsVariant, SelectOptionsConfig, SelectPromptOptions, TextPromptOptions,
};
use crate::exec::{CommandExecutor, ExecutionError};
use inquire::{InquireError, Password, PasswordDisplayMode, Select, Text};
use mockall::automock;
use std::collections::HashMap;
use std::string::FromUtf8Error;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PromptError {
    #[error("prompt failed")]
    InquireError(#[source] InquireError),

    #[error("failed to determine prompt options")]
    ExecutionError(#[source] ExecutionError),

    #[error("failed to parse prompt options")]
    ParseError(#[source] FromUtf8Error),
}

#[automock]
pub trait PromptExecutor {
    /// Prompts the user using the provided [`PromptConfig`], returning the user's response.
    fn execute(&self, prompt_config: &PromptConfig) -> Result<String, PromptError>;
}

pub struct TerminalPromptExecutor {
    command_executor: Box<dyn CommandExecutor>,
}

impl TerminalPromptExecutor {
    pub fn new(command_executor: Box<dyn CommandExecutor>) -> TerminalPromptExecutor {
        return TerminalPromptExecutor { command_executor };
    }
}

impl PromptExecutor for TerminalPromptExecutor {
    fn execute(&self, prompt_config: &PromptConfig) -> Result<String, PromptError> {
        match prompt_config.clone().options {
            PromptOptionsVariant::Text(text_prompt_options) => {
                execute_text_prompt(prompt_config.message.as_str(), &text_prompt_options)
            }
            PromptOptionsVariant::Select(select_prompt_config) => execute_select_prompt(
                prompt_config.message.as_str(),
                &select_prompt_config,
                &self.command_executor,
            ),
        }
    }
}

fn execute_text_prompt(
    message: &str,
    text_prompt_options: &TextPromptOptions,
) -> Result<String, PromptError> {
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
    command_executor: &Box<dyn CommandExecutor>,
) -> Result<String, PromptError> {
    let options = get_options(&select_prompt_options.options, command_executor)?;
    let result = Select::new(message, options).prompt();
    match result {
        Ok(value) => Ok(value),
        Err(err) => Err(PromptError::InquireError(err)),
    }
}

fn get_options(
    select_options_config: &SelectOptionsConfig,
    command_executor: &Box<dyn CommandExecutor>,
) -> Result<Vec<String>, PromptError> {
    match select_options_config {
        SelectOptionsConfig::Literal(options) => Ok(options.clone()),
        SelectOptionsConfig::Execution(execution_config) => {
            let output = command_executor
                .get_output(&execution_config.execution, &HashMap::new())
                .map_err(|err| PromptError::ExecutionError(err))?;
            let stdout =
                String::from_utf8(output.stdout).map_err(|err| PromptError::ParseError(err))?;
            let options = stdout.clone().lines().map(|s| String::from(s)).collect();
            Ok(options)
        }
    }
}

// This is hard to write tests for. Fow now, let's assume the Inquire crate has sufficient tests.
