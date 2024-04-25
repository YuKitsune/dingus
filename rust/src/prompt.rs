use std::error::Error;
use crate::definitions::{PromptDefinition, SelectDefinition, ConfirmDefinition};

pub struct PromptExecutor {

}

impl PromptExecutor {
    pub fn execute(&self, definition: &PromptDefinition) -> Result<String, Box<dyn Error>> {
        todo!("Prompt")
    }
}

pub struct SelectExecutor {

}

impl SelectExecutor {
    pub fn execute(&self, definition: &SelectDefinition) -> Result<String, Box<dyn Error>> {
        todo!("Selections")
    }
}

pub struct ConfirmExecutor {

}

impl ConfirmExecutor {
    pub fn execute(&self, definition: &ConfirmDefinition) -> Result<bool, Box<dyn Error>> {
        todo!("Confirmations")
    }
}