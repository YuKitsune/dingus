use std::error::Error;
use inquire::{Confirm, Select, Text};
use crate::definitions::{PromptDefinition, SelectDefinition, ConfirmDefinition, SelectPromptOptions};
use crate::shell::ShellExecutor;

pub struct PromptExecutor {}

impl PromptExecutor {
    pub fn execute(&self, definition: &PromptDefinition) -> Result<String, Box<dyn Error>> {
        let result = Text::new(definition.description.as_str()).prompt();
        match result {
            Ok(value) => Ok(value),
            Err(err) => Err(Box::new(err)),
        }
    }
}

pub struct SelectExecutor {
    pub command_executor: Box<dyn ShellExecutor>
}

impl SelectExecutor {
    pub fn execute(&self, definition: &SelectDefinition) -> Result<String, Box<dyn Error>> {
        let options = self.get_options(&definition.options)?;
        let result = Select::new(&definition.description, options).prompt();
        match result {
            Ok(value) => Ok(value),
            Err(err) => Err(Box::new(err)),
        }
    }

    fn get_options(&self, select_prompt_options: &SelectPromptOptions) -> Result<Vec<String>, Box<dyn Error>> {
        match select_prompt_options {
            SelectPromptOptions::Literal(options) => {
                Ok(options.clone())
            }
            SelectPromptOptions::Invocation(execution) => {
                let output = self.command_executor.get_output(&execution.clone().exec)?;
                let stdout = String::from_utf8(output.stdout)?;
                let options = stdout.clone().lines().map(|s| String::from(s)).collect();
                Ok(options)
            }
        }
    }
}

pub struct ConfirmExecutor { }

impl ConfirmExecutor {
    pub fn execute(&self, definition: &ConfirmDefinition) -> Result<bool, Box<dyn Error>> {
        let result = Confirm::new(definition.confirm.as_str())
            .with_default(false)
            .prompt();
        match result {
            Ok(value) => Ok(value),
            Err(err) => Err(Box::new(err)),
        }
    }
}