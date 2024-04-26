use std::collections::HashMap;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct Config {
    pub description: String,

    #[serde(skip_serializing_if = "HashMap::is_empty")]
    #[serde(default = "default_variables")]
    pub variables: HashMap<String, VariableDefinition>,

    pub commands: HashMap<String, CommandDefinition>,
}


#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct PromptCommandAction {
    // The variable to assign the result of the prompt to
    pub set: String,
    pub prompt: PromptDefinition
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct SelectCommandAction {
    // The variable to assign the result of the selection to
    pub set: String,
    #[serde(flatten)]
    pub select: SelectDefinition
}

fn default_variables() -> HashMap<String, VariableDefinition> {
    HashMap::new()
}

fn default_commands() -> HashMap<String, CommandDefinition> {
    HashMap::new()
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct CommandDefinition {
    pub description: String,

    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(default = "default_aliases")]
    pub alias: Vec<String>,

    #[serde(skip_serializing_if = "HashMap::is_empty")]
    #[serde(default = "default_variables")]
    pub variables: HashMap<String, VariableDefinition>,


    #[serde(skip_serializing_if = "HashMap::is_empty")]
    #[serde(default = "default_commands")]
    pub commands: HashMap<String, CommandDefinition>,

    #[serde(flatten)]
    pub action: CommandActions
}
#[derive(Serialize, Deserialize, PartialEq, Debug)]
#[serde(untagged)]
pub enum CommandActions {
    SingleStep(SingleCommandAction),
    MultiStep(MultiCommandAction),
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct SingleCommandAction {
    pub action: CommandAction
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct MultiCommandAction {
    pub actions: Vec<CommandAction>
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
#[serde(untagged)]
pub enum CommandAction {
    Invocation(String),
    Confirmation(ConfirmDefinition)
}

fn default_aliases() -> Vec<String> {
    Vec::new()
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
#[serde(untagged)]
pub enum VariableDefinition {
    Literal(String),
    Invocation(Execution),
    Prompt(PromptVariableDefinition),
    Select(SelectVariableDefinition),
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct PromptVariableDefinition {
    pub prompt: PromptDefinition
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct SelectVariableDefinition {
    pub select: SelectDefinition
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct Execution {
    pub exec: String
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct ConfirmDefinition {
    pub confirm: String,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct PromptDefinition {
    pub description: String,
    pub flag: Option<String>,

    #[serde(default="default_multi_line")]
    pub multi_line: bool
}

fn default_multi_line() -> bool {
    false
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct SelectDefinition {
    pub description: String,
    pub flag: Option<String>,
    pub options: SelectPromptOptions,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
#[serde(untagged)]
pub enum SelectPromptOptions {
    Literal(Vec<String>),
    Invocation(Execution)
}
