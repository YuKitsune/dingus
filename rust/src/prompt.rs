use std::error::Error;
use inquire::{Confirm, Select, Text};
use crate::config::{ConfirmationCommandActionConfig, PromptConfig, SelectConfig, SelectOptionsConfig};
use crate::shell::{ShellExecutorFactory};

pub struct PromptExecutor {}

impl PromptExecutor {
    pub fn execute(&self, prompt_config: &PromptConfig) -> Result<String, Box<dyn Error>> {
        let result = Text::new(prompt_config.message.as_str()).prompt();
        match result {
            Ok(value) => Ok(value),
            Err(err) => Err(Box::new(err)),
        }
    }
}

pub struct SelectExecutor {
    pub shell_executor_factory: Box<dyn ShellExecutorFactory>
}

impl SelectExecutor {
    pub fn execute(&self, select_config: &SelectConfig) -> Result<String, Box<dyn Error>> {
        let options = self.get_options(&select_config.options)?;
        let result = Select::new(&select_config.message, options).prompt();
        match result {
            Ok(value) => Ok(value),
            Err(err) => Err(Box::new(err)),
        }
    }

    fn get_options(&self, select_options_config: &SelectOptionsConfig) -> Result<Vec<String>, Box<dyn Error>> {
        match select_options_config {
            SelectOptionsConfig::Literal(options) => {
                Ok(options.clone())
            }
            SelectOptionsConfig::Execution(execution_config) => {
                let shell_executor = match &execution_config.shell {
                    Some(shell) => self.shell_executor_factory.create(&shell),
                    None => self.shell_executor_factory.create_default(),
                };

                let output = shell_executor.get_output(&execution_config.shell_command)?;
                let stdout = String::from_utf8(output.stdout)?;
                let options = stdout.clone().lines().map(|s| String::from(s)).collect();
                Ok(options)
            }
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