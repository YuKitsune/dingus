use std::error::Error;
use inquire::{Confirm, Select, Text};
use crate::config::{ConfirmationCommandActionConfig, PromptVariableConfig, SelectOptionsConfig, SelectPromptVariableConfig, TextPromptVariableConfig};
use crate::shell::{ShellExecutorFactory};

pub trait PromptExecutor {
    fn execute(&self, prompt_config: &PromptVariableConfig) -> Result<String, Box<dyn Error>>;
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
    fn execute(&self, prompt_config: &PromptVariableConfig) -> Result<String, Box<dyn Error>> {
        match prompt_config {
            PromptVariableConfig::Text(text_prompt_config) =>
                execute_text_prompt(text_prompt_config),
            PromptVariableConfig::Select(select_prompt_config) =>
                execute_select_prompt(select_prompt_config, &self.shell_executor_factory),
        }
    }
}

fn execute_text_prompt(text_prompt_variable_config: &TextPromptVariableConfig) -> Result<String, Box<dyn Error>> {
    let result = Text::new(text_prompt_variable_config.message.as_str()).prompt();
    match result {
        Ok(value) => Ok(value),
        Err(err) => Err(Box::new(err)),
    }
}

fn execute_select_prompt(
    select_prompt_variable_config: &SelectPromptVariableConfig,
    shell_executor_factory: &Box<dyn ShellExecutorFactory>) -> Result<String, Box<dyn Error>> {
    let options = get_options(&select_prompt_variable_config.options, shell_executor_factory)?;
    let result = Select::new(&select_prompt_variable_config.message, options).prompt();
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

            let output = shell_executor.get_output(&execution_config.shell_command)?;
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