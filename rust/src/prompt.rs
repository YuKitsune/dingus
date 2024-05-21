use std::collections::HashMap;
use std::error::Error;
use inquire::{Confirm, Password, PasswordDisplayMode, Select, Text};
use crate::config::{ConfirmationCommandActionConfig, PromptConfig, PromptOptionsVariant, SelectOptionsConfig, SelectPromptOptions, TextPromptOptions};
use crate::exec::CommandExecutor;

pub trait PromptExecutor {
    fn execute(&self, prompt_config: &PromptConfig) -> Result<String, Box<dyn Error>>;
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
    fn execute(&self, prompt_config: &PromptConfig) -> Result<String, Box<dyn Error>> {
        match prompt_config.clone().options {
            PromptOptionsVariant::Text(text_prompt_options) =>
                execute_text_prompt(prompt_config.message.as_str(), &text_prompt_options),
            PromptOptionsVariant::Select(select_prompt_config) =>
                execute_select_prompt(prompt_config.message.as_str(), &select_prompt_config, &self.command_executor),
        }
    }
}

fn execute_text_prompt(message: &str, text_prompt_options: &TextPromptOptions) -> Result<String, Box<dyn Error>> {
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
        Err(err) => Err(Box::new(err)),
    }
}

fn execute_select_prompt(
    message: &str,
    select_prompt_options: &SelectPromptOptions,
    command_executor: &Box<dyn CommandExecutor>) -> Result<String, Box<dyn Error>> {
    let options = get_options(&select_prompt_options.options, command_executor)?;
    let result = Select::new(message, options).prompt();
    match result {
        Ok(value) => Ok(value),
        Err(err) => Err(Box::new(err)),
    }
}

fn get_options(select_options_config: &SelectOptionsConfig, command_executor: &Box<dyn CommandExecutor>) -> Result<Vec<String>, Box<dyn Error>> {
    match select_options_config {
        SelectOptionsConfig::Literal(options) => {
            Ok(options.clone())
        }
        SelectOptionsConfig::Execution(execution_config) => {
            let output = command_executor.get_output(&execution_config, &HashMap::new())?;
            let stdout = String::from_utf8(output.stdout)?;
            let options = stdout.clone().lines().map(|s| String::from(s)).collect();
            Ok(options)
        }
    }
}

pub struct ConfirmExecutor { }

impl ConfirmExecutor {
    pub fn execute(&self, confirmation_config: &ConfirmationCommandActionConfig) -> Result<bool, Box<dyn Error>> {
        let result = Confirm::new(confirmation_config.confirm.as_str())
            .with_default(false)
            .prompt();
        match result {
            Ok(value) => Ok(value),
            Err(err) => Err(Box::new(err)),
        }
    }
}