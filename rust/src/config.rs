use std::{fmt, fs, io};
use std::collections::{HashMap};
use std::error::Error;
use std::path::Path;
use serde::{Deserialize, Serialize};
use crate::shell::ShellCommand;

const CONFIG_FILE_NAMES: [&str;2] = ["shiji.yaml", "shiji.yml"];

pub fn load() -> Result<Config, ConfigError> {
    for config_file_name in CONFIG_FILE_NAMES {
        if !Path::new(config_file_name).exists() {
            continue
        }

        let config_text: String = fs::read_to_string(config_file_name).map_err(|err| ConfigError::ReadFailed(err))?;
        let config: Config = serde_yaml::from_str(&config_text).map_err(|err| ConfigError::ParseFailed(err))?;

        return Ok(config);
    }

    return Err(ConfigError::FileNotFound)
}

#[derive(Debug)]
pub enum ConfigError {
    FileNotFound,
    ReadFailed(io::Error),
    ParseFailed(serde_yaml::Error)
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ConfigError::FileNotFound => write!(f, "config file not found"),
            ConfigError::ReadFailed(io_err) => write!(f, "failed to read config file: {}", io_err),
            ConfigError::ParseFailed(parse_err) => write!(f, "failed to parse config file: {}", parse_err)
        }
    }
}

impl Error for ConfigError {}

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    pub description: String,

    #[serde(default = "default_shell")]
    pub default_shell: Shell,

    #[serde(default = "default_variables")]
    pub variables: HashMap<String, VariableConfig>,

    pub commands: HashMap<String, CommandConfig>,
}

fn default_shell() -> Shell { Shell::Bash }

fn default_variables() -> HashMap<String, VariableConfig> { HashMap::new() }

fn default_commands() -> HashMap<String, CommandConfig> { HashMap::new() }

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub enum Shell {
    Bash
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
#[serde(untagged)]
pub enum VariableConfig {
    Literal(String),
    LiteralExtended(ExtendedLiteralVariableConfig),
    Execution(ExecutionVariableConfig),
    Prompt(PromptVariableConfig)
}

impl VariableConfig {
    pub fn arg_name(&self, key: &String) -> String {
        match self {
            VariableConfig::Literal(_) => None,
            VariableConfig::LiteralExtended(extended_literal_def) => extended_literal_def.clone().argument_name,
            VariableConfig::Execution(execution_def) => execution_def.clone().argument_name,
            VariableConfig::Prompt(prompt_def) => {
                match prompt_def.clone().prompt {
                    PromptVariableConfigVariant::Text(text_prompt_def) => text_prompt_def.clone().argument_name,
                    PromptVariableConfigVariant::Select(select_prompt_def) => select_prompt_def.clone().argument_name,
                }
            },
        }.unwrap_or(key.clone())
    }

    pub fn description(&self) -> Option<String> {
        return match self {
            VariableConfig::Literal(_) => None,
            VariableConfig::LiteralExtended(extended_literal_def) => extended_literal_def.clone().description,
            VariableConfig::Execution(execution_def) => execution_def.clone().description,
            VariableConfig::Prompt(prompt_def) => {
                match prompt_def.clone().prompt {
                    PromptVariableConfigVariant::Text(text_prompt_def) => text_prompt_def.clone().argument_name,
                    PromptVariableConfigVariant::Select(select_prompt_def) => select_prompt_def.clone().argument_name,
                }
            },
        }
    }
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct ExtendedLiteralVariableConfig {
    pub value: String,
    pub description: Option<String>,

    #[serde(rename(deserialize = "arg"))]
    pub argument_name: Option<String>
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct ExecutionVariableConfig {

    #[serde(flatten)]
    pub execution: ExecutionConfig,
    pub description: Option<String>,

    #[serde(rename(deserialize = "arg"))]
    pub argument_name: Option<String>
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct ExecutionConfig {
    pub shell: Option<Shell>,

    #[serde(rename(deserialize = "exec"))]
    pub shell_command: ShellCommand
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct PromptVariableConfig {
    pub prompt: PromptVariableConfigVariant
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
#[serde(untagged)]
pub enum PromptVariableConfigVariant {
    Text(TextPromptVariableConfig),
    Select(SelectPromptVariableConfig)
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct TextPromptVariableConfig {
    pub description: Option<String>,

    #[serde(rename(deserialize = "arg"))]
    pub argument_name: Option<String>,

    pub message: String,

    #[serde(default = "default_multi_line")]
    pub multi_line: bool
}

fn default_multi_line() -> bool { false }

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct SelectPromptVariableConfig {
    pub description: Option<String>,

    #[serde(rename(deserialize = "arg"))]
    pub argument_name: Option<String>,

    pub message: String,
    pub options: SelectOptionsConfig,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct SelectConfig {
    pub message: String,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
#[serde(untagged)]
pub enum SelectOptionsConfig {
    Literal(Vec<String>),
    Execution(ExecutionConfig)
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct CommandConfig {
    pub description: String,

    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(default = "default_aliases")]
    pub aliases: Vec<String>,

    #[serde(default = "default_variables")]
    pub variables: HashMap<String, VariableConfig>,

    // Todo: Need to enforce an invariant here:
    // - If no action exists, then one or more subcommands _must_ exist.
    #[serde(default = "default_commands")]
    pub commands: HashMap<String, CommandConfig>,

    #[serde(flatten)]
    pub action: Option<CommandActionConfigVariant>
}

fn default_aliases() -> Vec<String> {
    Vec::new()
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
#[serde(untagged)]
pub enum CommandActionConfigVariant {
    SingleStep(SingleCommandActionConfig),
    MultiStep(MultiCommandActionConfig),
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct SingleCommandActionConfig {
    pub action: CommandActionConfig
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct MultiCommandActionConfig {
    pub actions: Vec<CommandActionConfig>
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
#[serde(untagged)]
pub enum CommandActionConfig {
    Execution(ShellCommand),
    ExtendedExecution(ExecutionConfig),
    Confirmation(ConfirmationCommandActionConfig)
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct ConfirmationCommandActionConfig {
    pub confirm: String,
}
