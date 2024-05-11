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
        let config = parse_config(&config_text)?;

        return Ok(config);
    }

    return Err(ConfigError::FileNotFound)
}

fn parse_config(text: &str) -> Result<Config, ConfigError> {
    let result = serde_yaml::from_str(text);
    return match result {
        Ok(config) => Ok(config),
        Err(parse_err) => Err(ConfigError::ParseFailed(parse_err))
    }
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
    pub description: Option<String>,

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
    pub fn arg_name(&self, key: &str) -> String {
        match self {
            VariableConfig::Literal(_) => None,
            VariableConfig::LiteralExtended(extended_literal_def) => extended_literal_def.clone().argument_name,
            VariableConfig::Execution(execution_def) => execution_def.clone().argument_name,
            VariableConfig::Prompt(prompt_config) => prompt_config.clone().argument_name,
        }.unwrap_or(key.to_string())
    }

    pub fn description(&self) -> Option<String> {
        return match self {
            VariableConfig::Literal(_) => None,
            VariableConfig::LiteralExtended(extended_literal_def) => extended_literal_def.clone().description,
            VariableConfig::Execution(execution_def) => execution_def.clone().description,
            VariableConfig::Prompt(prompt_config) => prompt_config.clone().description,
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
    pub description: Option<String>,

    #[serde(rename(deserialize = "arg"))]
    pub argument_name: Option<String>,

    pub prompt: PromptConfig
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct PromptConfig {
    pub message: String,

    #[serde(flatten)]
    pub options: PromptOptionsVariant
}

impl Default for PromptOptionsVariant {
    fn default() -> Self {
        return PromptOptionsVariant::Text(TextPromptOptions {
            multi_line: false,
            sensitive: false,
        })
    }
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
#[serde(untagged)]
pub enum PromptOptionsVariant {
    // Note: Select needs to come first here because SelectPromptOptions is the most specific.
    // Serde will use the type it matches on.
    Select(SelectPromptOptions),
    Text(TextPromptOptions)
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct TextPromptOptions {

    #[serde(default = "default_multi_line")]
    pub multi_line: bool,

    #[serde(default = "default_sensitive")]
    pub sensitive: bool
}

fn default_multi_line() -> bool { false }

fn default_sensitive() -> bool { false }

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct SelectPromptOptions {
    pub options: SelectOptionsConfig,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
#[serde(untagged)]
pub enum SelectOptionsConfig {
    Literal(Vec<String>),
    Execution(ExecutionConfig)
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct CommandConfig {
    pub description: Option<String>,

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variable_get_arg_returns_correct_arg_name() {

        let literal = VariableConfig::Literal("Dingus".to_string());
        assert_eq!("key", literal.arg_name("key"));

        let extended_literal_no_arg = VariableConfig::LiteralExtended(ExtendedLiteralVariableConfig{
            value: "Dingus".to_string(),
            description: None,
            argument_name: None,
        });
        assert_eq!("key", extended_literal_no_arg.arg_name("key"));

        let extended_literal_with_arg = VariableConfig::LiteralExtended(ExtendedLiteralVariableConfig{
            value: "Dingus".to_string(),
            description: None,
            argument_name: Some("name".to_string()),
        });
        assert_eq!("name", extended_literal_with_arg.arg_name("key"));

        let exec_no_arg = VariableConfig::Execution(ExecutionVariableConfig{
            execution: ExecutionConfig { shell: None, shell_command: "echo \"Dingus\"".to_string() },
            description: None,
            argument_name: None,
        });
        assert_eq!("key", exec_no_arg.arg_name("key"));

        let exec_with_arg = VariableConfig::Execution(ExecutionVariableConfig{
            execution: ExecutionConfig { shell: None, shell_command: "echo \"Dingus\"".to_string() },
            description: None,
            argument_name: Some("name".to_string()),
        });
        assert_eq!("name", exec_with_arg.arg_name("key"));

        let prompt_no_arg = VariableConfig::Prompt(PromptVariableConfig {
            description: None,
            argument_name: None,
            prompt: PromptConfig { message: "".to_string(), options: PromptOptionsVariant::default() },
        });
        assert_eq!("key", prompt_no_arg.arg_name("key"));

        let prompt_with_arg = VariableConfig::Prompt(PromptVariableConfig {
            description: None,
            argument_name: Some("name".to_string()),
            prompt: PromptConfig { message: "".to_string(), options: PromptOptionsVariant::default() },
        });
        assert_eq!("name", prompt_with_arg.arg_name("key"));
    }

    #[test]
    fn empty_root_variables_allowed() {
        let yaml =
"commands:
    demo:
        action: echo \"Hello, World!\"";
        let config = parse_config(yaml).unwrap();

        assert!(config.variables.is_empty());
    }

    #[test]
    fn literal_variable_parsed() {
        let yaml =
            "variables:
    my-root-var: My root value
commands:
    demo:
        variables:
            my-command-var: My command value
        action: echo \"Hello, World!\"";
        let config = parse_config(yaml).unwrap();

        assert!(!config.variables.is_empty());

        let root_variable = config.variables.get("my-root-var").unwrap();
        assert_eq!(root_variable, &VariableConfig::Literal("My root value".to_string()));

        let demo_command = config.commands.get("demo").unwrap();
        let command_variable = demo_command.variables.get("my-command-var").unwrap();
        assert_eq!(command_variable, &VariableConfig::Literal("My command value".to_string()))
    }

    #[test]
    fn extended_variable_parsed() {
        let yaml =
            "variables:
    my-root-var:
        value: My root value
commands:
    demo:
        variables:
            my-command-var:
                value: My command value
                description: Command level variable
                arg: command-arg
        action: echo \"Hello, World!\"";
        let config = parse_config(yaml).unwrap();

        assert!(!config.variables.is_empty());

        let root_variable = config.variables.get("my-root-var").unwrap();
        assert_eq!(root_variable, &VariableConfig::LiteralExtended(ExtendedLiteralVariableConfig {
            value: "My root value".to_string(),
            description: None,
            argument_name: None,
        }));

        let demo_command = config.commands.get("demo").unwrap();
        let command_variable = demo_command.variables.get("my-command-var").unwrap();
        assert_eq!(command_variable, &VariableConfig::LiteralExtended(ExtendedLiteralVariableConfig {
            value: "My command value".to_string(),
            description: Some("Command level variable".to_string()),
            argument_name: Some("command-arg".to_string()),
        }))
    }

    #[test]
    fn exec_variable_parsed() {
        let yaml =
            "variables:
    my-root-var:
        exec: echo \"My root value\"
commands:
    demo:
        variables:
            my-command-var:
                exec: echo \"My command value\"
                shell: Bash
                description: Command level variable
                arg: command-arg
        action: echo \"Hello, World!\"";
        let config = parse_config(yaml).unwrap();

        assert!(!config.variables.is_empty());

        let root_variable = config.variables.get("my-root-var").unwrap();
        assert_eq!(root_variable, &VariableConfig::Execution(ExecutionVariableConfig {
            execution: ExecutionConfig { shell: None, shell_command: "echo \"My root value\"".to_string() },
            description: None,
            argument_name: None,
        }));

        let demo_command = config.commands.get("demo").unwrap();
        let command_variable = demo_command.variables.get("my-command-var").unwrap();
        assert_eq!(command_variable, &VariableConfig::Execution(ExecutionVariableConfig {
            execution: ExecutionConfig { shell: Some(Shell::Bash), shell_command: "echo \"My command value\"".to_string() },
            description: Some("Command level variable".to_string()),
            argument_name: Some("command-arg".to_string()),
        }))
    }

    // Todo: Prompt variable - Pass

    #[test]
    fn prompt_variable_parsed() {
        let yaml =
            "variables:
    name:
        prompt:
            message: What's your name?
    food:
        description: Favourite food
        arg: food
        prompt:
            message: What's your favourite food?
            options:
                - Burger
                - Pizza
                - Fries
commands:
    demo:
        variables:
            password:
                prompt:
                    message: What's your password?
                    sensitive: true
            life-story:
                prompt:
                    message: What's your life story?
                    multi_line: true
            favourite-line:
                prompt:
                    message: What's your favourite line?
                    options:
                        exec: cat example.txt

        action: echo \"Hello, World!\"";
        let config = parse_config(yaml).unwrap();

        assert!(!config.variables.is_empty());

        let name_variable = config.variables.get("name").unwrap();
        assert_eq!(name_variable, &VariableConfig::Prompt(PromptVariableConfig {
            description: None,
            argument_name: None,
            prompt: PromptConfig {
                message: "What's your name?".to_string(),
                options: PromptOptionsVariant::Text(TextPromptOptions {
                    multi_line: false,
                    sensitive: false,
                })
            },
        }));

        let food_variable = config.variables.get("food").unwrap();
        assert_eq!(food_variable, &VariableConfig::Prompt(PromptVariableConfig {
            description: Some("Favourite food".to_string()),
            argument_name: Some("food".to_string()),
            prompt: PromptConfig {
                message: "What's your favourite food?".to_string(),
                options: PromptOptionsVariant::Select(SelectPromptOptions {
                    options: SelectOptionsConfig::Literal(vec![
                        "Burger".to_string(),
                        "Pizza".to_string(),
                        "Fries".to_string()
                    ])
                })
            },
        }));

        let demo_command = config.commands.get("demo").unwrap();
        let password_variable = demo_command.variables.get("password").unwrap();
        assert_eq!(password_variable, &VariableConfig::Prompt(PromptVariableConfig {
            description: None,
            argument_name: None,
            prompt: PromptConfig {
                message: "What's your password?".to_string(),
                options: PromptOptionsVariant::Text(TextPromptOptions {
                    multi_line: false,
                    sensitive: true
                })
            },
        }));

        let life_story_variable = demo_command.variables.get("life-story").unwrap();
        assert_eq!(life_story_variable, &VariableConfig::Prompt(PromptVariableConfig {
            description: None,
            argument_name: None,
            prompt: PromptConfig {
                message: "What's your life story?".to_string(),
                options: PromptOptionsVariant::Text(TextPromptOptions {
                    multi_line: true,
                    sensitive: false
                })
            },
        }));

        let fav_line_variable = demo_command.variables.get("favourite-line").unwrap();
        assert_eq!(fav_line_variable, &VariableConfig::Prompt(PromptVariableConfig {
            description: None,
            argument_name: None,
            prompt: PromptConfig {
                message: "What's your favourite line?".to_string(),
                options: PromptOptionsVariant::Select(SelectPromptOptions {
                    options: SelectOptionsConfig::Execution(ExecutionConfig {
                        shell: None,
                        shell_command: "cat example.txt".to_string() }),
                })
            },
        }))
    }

    // Todo: Basic command
    // Todo: Command with all the properties
    // Todo: Command with subcommands and an action
    // Todo: Command with subcommands only
    // Todo: Command with action only
    // Todo: Command with no subcommands or action - Fail

    // Todo: Basic execute action
    // Todo: Extended execute action
    // Todo: Confirmation action
}