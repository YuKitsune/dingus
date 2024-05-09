use std::collections::HashMap;
use std::error::Error;
use inquire::{Confirm, Password, PasswordDisplayMode, Select, Text};
use crate::config::{ConfirmationCommandActionConfig, PromptConfig, PromptOptionsVariant, SelectOptionsConfig, SelectPromptOptions, TextPromptOptions};
use crate::shell::{ShellExecutorFactory};

pub trait PromptExecutor {
    fn execute(&self, prompt_config: &PromptConfig) -> Result<String, Box<dyn Error>>;
}

pub struct TerminalPromptExecutor {
    shell_executor_factory: Box<dyn ShellExecutorFactory>
}

impl TerminalPromptExecutor {
    pub fn new(shell_executor_factory: Box<dyn ShellExecutorFactory>) -> TerminalPromptExecutor {
        return TerminalPromptExecutor{shell_executor_factory}
    }
}

impl PromptExecutor for TerminalPromptExecutor {
    fn execute(&self, prompt_config: &PromptConfig) -> Result<String, Box<dyn Error>> {
        match prompt_config.clone().options {
            PromptOptionsVariant::Text(text_prompt_options) =>
                execute_text_prompt(prompt_config.message.as_str(), &text_prompt_options),
            PromptOptionsVariant::Select(select_prompt_config) =>
                execute_select_prompt(prompt_config.message.as_str(), &select_prompt_config, &self.shell_executor_factory),
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
    shell_executor_factory: &Box<dyn ShellExecutorFactory>) -> Result<String, Box<dyn Error>> {
    let options = get_options(&select_prompt_options.options, shell_executor_factory)?;
    let result = Select::new(message, options).prompt();
    match result {
        Ok(value) => Ok(value),
        Err(err) => Err(Box::new(err)),
    }
}

fn get_options(select_options_config: &SelectOptionsConfig, shell_executor_factory: &Box<dyn ShellExecutorFactory>) -> Result<Vec<String>, Box<dyn Error>> {
    match select_options_config {
        SelectOptionsConfig::Literal(options) => {
            Ok(options.clone())
        }
        SelectOptionsConfig::Execution(execution_config) => {
            let shell_executor = match &execution_config.shell {
                Some(shell) => shell_executor_factory.create(&shell),
                None => shell_executor_factory.create_default(),
            };

            let output = shell_executor.execute(&execution_config.shell_command, &HashMap::new())?;
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