use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use crate::shell::ShellCommand;

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct Config {
    pub description: String,

    #[serde(skip_serializing_if = "HashMap::is_empty")]
    #[serde(default = "default_variables")]
    pub variables: HashMap<String, VariableDefinition>,

    pub commands: HashMap<String, CommandDefinition>,
}

fn default_variables() -> HashMap<String, VariableDefinition> {
    HashMap::new()
}

fn default_commands() -> HashMap<String, CommandDefinition> {
    HashMap::new()
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
#[serde(untagged)]
pub enum VariableDefinition {
    Literal(String),
    LiteralExtended(ExtendedLiteralVariableDefinition),
    Execution(ExecutionVariableDefinition),
    Prompt(PromptVariableDefinition),
    Select(SelectVariableDefinition),
}

impl VariableDefinition {
    pub fn arg_name(&self, key: &String) -> String {
        match self {
            VariableDefinition::Literal(_) => None,
            VariableDefinition::LiteralExtended(extended_literal_def) => extended_literal_def.clone().flag,
            VariableDefinition::Execution(execution_def) => execution_def.clone().flag,
            VariableDefinition::Prompt(prompt_def) => prompt_def.clone().prompt.flag,
            VariableDefinition::Select(select_def) => select_def.clone().select.flag,
        }.unwrap_or(key.clone())
    }
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct ExtendedLiteralVariableDefinition {
    pub value: String,
    pub description: Option<String>,
    pub flag: Option<String>
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct ExecutionVariableDefinition {
    #[serde(rename(deserialize = "exec"))]
    pub shell_command: ShellCommand,
    pub description: Option<String>,
    pub flag: Option<String>
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct PromptVariableDefinition {
    pub prompt: PromptDefinition,
    pub description: Option<String>,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct SelectVariableDefinition {
    pub select: SelectDefinition,
    pub description: Option<String>,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct PromptDefinition {
    pub message: String,
    pub flag: Option<String>,

    #[serde(default="default_multi_line")]
    pub multi_line: bool
}

fn default_multi_line() -> bool {
    false
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct SelectDefinition {
    pub message: String,
    pub flag: Option<String>,
    pub options: SelectOptions,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
#[serde(untagged)]
pub enum SelectOptions {
    Literal(Vec<String>),
    Invocation(InvocationSelectOptions)
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct InvocationSelectOptions {
    #[serde(rename(deserialize = "exec"))]
    pub shell_command: ShellCommand
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct CommandDefinition {
    pub description: String,

    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(default = "default_aliases")]
    pub aliases: Vec<String>,

    #[serde(skip_serializing_if = "HashMap::is_empty")]
    #[serde(default = "default_variables")]
    pub variables: HashMap<String, VariableDefinition>,

    #[serde(skip_serializing_if = "HashMap::is_empty")]
    #[serde(default = "default_commands")]
    pub commands: HashMap<String, CommandDefinition>,

    #[serde(flatten)]
    pub action: Option<CommandActionsVariant>
}

fn default_aliases() -> Vec<String> {
    Vec::new()
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
#[serde(untagged)]
pub enum CommandActionsVariant {
    SingleStep(SingleCommandAction),
    MultiStep(MultiCommandAction),
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct SingleCommandAction {
    pub action: CommandAction
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct MultiCommandAction {
    pub actions: Vec<CommandAction>
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
#[serde(untagged)]
pub enum CommandAction {
    Execution(ShellCommand),
    Confirmation(ConfirmationCommandActionDefinition)
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct ConfirmationCommandActionDefinition {
    pub confirm: String,
}
