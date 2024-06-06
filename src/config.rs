use std::{fmt, fs, io};
use std::collections::HashMap;
use std::error::Error;
use std::path::Path;
use linked_hash_map::LinkedHashMap;
use serde::{Deserialize, Serialize};

const CONFIG_FILE_NAMES: [&str;4] = [
    "gecko.yaml",
    "Gecko.yaml",
    "gecko.yml",
    "Gecko.yml"
];

const DEFAULT_CONFIG_FILE: &str =
    "description: My Gecko file

variables:
  name: Godzilla

commands:
  greet:
    action: echo \"Hello, $name!\"";

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

pub fn init() -> Result<String, ConfigError> {
    let file_name = CONFIG_FILE_NAMES[0];

    fs::write(file_name, DEFAULT_CONFIG_FILE).map_err(|io_err| ConfigError::WriteFailed(io_err))?;
    return Ok(file_name.to_string());
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
    WriteFailed(io::Error),
    ParseFailed(serde_yaml::Error)
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ConfigError::FileNotFound => write!(f, "config file not found"),
            ConfigError::ReadFailed(io_err) => write!(f, "failed to read config file: {}", io_err),
            ConfigError::WriteFailed(io_err) => write!(f, "failed to write config file: {}", io_err),
            ConfigError::ParseFailed(parse_err) => write!(f, "failed to parse config file: {}", parse_err),
        }
    }
}

impl Error for ConfigError {}

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    #[serde(alias = "desc")]
    pub description: Option<String>,

    #[serde(default = "default_variables")]
    #[serde(alias = "vars")]
    pub variables: VariableConfigMap,

    #[serde(alias = "cmds")]
    pub commands: CommandConfigMap,
}

fn default_variables() -> VariableConfigMap { VariableConfigMap::new() }

fn default_commands() -> CommandConfigMap { CommandConfigMap::new() }

pub type VariableConfigMap = LinkedHashMap<String, VariableConfig>;

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
#[serde(untagged)]
pub enum VariableConfig {
    ShorthandLiteral(String),
    Literal(LiteralVariableConfig),
    Execution(ExecutionVariableConfig),
    Prompt(PromptVariableConfig)
}

impl VariableConfig {
    pub fn arg_name(&self, key: &str) -> String {
        match self {
            VariableConfig::ShorthandLiteral(_) => None,
            VariableConfig::Literal(literal_conf) => literal_conf.clone().argument_name,
            VariableConfig::Execution(execution_conf) => execution_conf.clone().argument_name,
            VariableConfig::Prompt(prompt_conf) => prompt_conf.clone().argument_name,
        }.unwrap_or(key.to_string())
    }

    pub fn description(&self) -> Option<String> {
        return match self {
            VariableConfig::ShorthandLiteral(_) => None,
            VariableConfig::Literal(literal_conf) => literal_conf.clone().description,
            VariableConfig::Execution(execution_conf) => execution_conf.clone().description,
            VariableConfig::Prompt(prompt_config) => prompt_config.clone().description,
        }
    }
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct LiteralVariableConfig {
    pub value: String,

    #[serde(alias = "desc")]
    pub description: Option<String>,

    #[serde(rename(deserialize = "arg"))]
    pub argument_name: Option<String>
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct ExecutionVariableConfig {

    #[serde(rename = "exec")]
    pub execution: ExecutionConfigVariant,

    #[serde(alias = "desc")]
    pub description: Option<String>,

    #[serde(rename(deserialize = "arg"))]
    pub argument_name: Option<String>
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct PromptVariableConfig {
    #[serde(alias = "desc")]
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
    Execution(ExecutionSelectOptionsConfig),
    Literal(Vec<String>)
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct ExecutionSelectOptionsConfig {
    #[serde(rename = "exec")]
    pub execution: ExecutionConfigVariant
}

pub type CommandConfigMap = HashMap<String, CommandConfig>;

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct CommandConfig {
    pub description: Option<String>,

    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(default = "default_aliases")]
    pub aliases: Vec<String>,

    #[serde(default = "default_variables")]
    pub variables: VariableConfigMap,

    // Todo: Need to enforce an invariant here:
    // - If no action exists, then one or more subcommands _must_ exist.
    #[serde(default = "default_commands")]
    pub commands: CommandConfigMap,

    #[serde(flatten)]
    pub action: Option<CommandActionConfigVariant>
}

fn default_aliases() -> Vec<String> {
    Vec::new()
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
#[serde(untagged)]
pub enum CommandActionConfigVariant {
    SingleStep(SingleActionConfig),
    MultiStep(MultiActionConfig),
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct SingleActionConfig {
    pub action: ActionConfig
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct MultiActionConfig {
    pub actions: Vec<ActionConfig>
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
#[serde(untagged)]
pub enum ActionConfig {
    Execution(ExecutionConfigVariant),
    Confirmation(ConfirmationCommandActionConfig)
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
#[serde(untagged)]
pub enum ExecutionConfigVariant {
    RawCommand(RawCommandConfigVariant),
    ShellCommand(ShellCommandConfigVariant)
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
#[serde(untagged)]
pub enum RawCommandConfigVariant {
    Shorthand(String),
    RawCommandConfig(RawCommandConfig)
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct RawCommandConfig {
    #[serde(alias = "wd")]
    #[serde(alias = "workdir")]
    pub working_directory: Option<String>,

    #[serde(alias = "cmd")]
    #[serde(alias = "exec")]
    pub command: String
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
#[serde(untagged)]
pub enum ShellCommandConfigVariant {
    Bash(BashCommandConfig)
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct BashCommandConfig {
    #[serde(alias = "wd")]
    #[serde(alias = "workdir")]
    pub working_directory: Option<String>,

    #[serde(rename = "bash")]
    #[serde(alias = "sh")]
    pub command: String
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct ConfirmationCommandActionConfig {
    pub confirm: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bash_exec(command: &str, workdir: Option<String>) -> ExecutionConfigVariant {
        return ExecutionConfigVariant::ShellCommand(ShellCommandConfigVariant::Bash(BashCommandConfig {
            working_directory: workdir,
            command: command.to_string(),
        }))
    }

    fn raw_exec(command: &str) -> ExecutionConfigVariant {
        return ExecutionConfigVariant::RawCommand(RawCommandConfigVariant::Shorthand(command.to_string()))
    }

    #[test]
    fn variable_get_arg_returns_correct_arg_name() {

        let literal = VariableConfig::ShorthandLiteral("Dingus".to_string());
        assert_eq!("key", literal.arg_name("key"));

        let literal_no_arg = VariableConfig::Literal(LiteralVariableConfig{
            value: "Dingus".to_string(),
            description: None,
            argument_name: None,
        });
        assert_eq!("key", literal_no_arg.arg_name("key"));

        let literal_with_arg = VariableConfig::Literal(LiteralVariableConfig{
            value: "Dingus".to_string(),
            description: None,
            argument_name: Some("name".to_string()),
        });
        assert_eq!("name", literal_with_arg.arg_name("key"));

        let exec_no_arg = VariableConfig::Execution(ExecutionVariableConfig{
            execution: bash_exec("echo \"Dingus\"", None),
            description: None,
            argument_name: None,
        });
        assert_eq!("key", exec_no_arg.arg_name("key"));

        let exec_with_arg = VariableConfig::Execution(ExecutionVariableConfig{
            execution: bash_exec("echo \"Dingus\"", None),
            description: None,
            argument_name: Some("name".to_string()),
        });
        assert_eq!("name", exec_with_arg.arg_name("key"));

        let prompt_no_arg = VariableConfig::Prompt(PromptVariableConfig {
            description: None,
            argument_name: None,
            prompt: PromptConfig { message: "".to_string(), options: Default::default() },
        });
        assert_eq!("key", prompt_no_arg.arg_name("key"));

        let prompt_with_arg = VariableConfig::Prompt(PromptVariableConfig {
            description: None,
            argument_name: Some("name".to_string()),
            prompt: PromptConfig { message: "".to_string(), options: Default::default() },
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
    fn shorthand_literal_variable_parsed() {
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
        assert_eq!(root_variable, &VariableConfig::ShorthandLiteral("My root value".to_string()));

        let demo_command = config.commands.get("demo").unwrap();
        let command_variable = demo_command.variables.get("my-command-var").unwrap();
        assert_eq!(command_variable, &VariableConfig::ShorthandLiteral("My command value".to_string()))
    }

    #[test]
    fn literal_variable_parsed() {
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
        assert_eq!(root_variable, &VariableConfig::Literal(LiteralVariableConfig {
            value: "My root value".to_string(),
            description: None,
            argument_name: None,
        }));

        let demo_command = config.commands.get("demo").unwrap();
        let command_variable = demo_command.variables.get("my-command-var").unwrap();
        assert_eq!(command_variable, &VariableConfig::Literal(LiteralVariableConfig {
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
        exec:
            sh: echo \"My root value\"
            working_directory: ../
commands:
    demo:
        variables:
            my-command-var:
                exec:
                    bash: echo \"My command value\"
                description: Command level variable
                arg: command-arg
        action: echo \"Hello, World!\"";
        let config = parse_config(yaml).unwrap();

        assert!(!config.variables.is_empty());

        let root_variable = config.variables.get("my-root-var").unwrap();
        assert_eq!(root_variable, &VariableConfig::Execution(ExecutionVariableConfig {
            execution: bash_exec("echo \"My root value\"", Some("../".to_string())),
            description: None,
            argument_name: None,
        }));

        let demo_command = config.commands.get("demo").unwrap();
        let command_variable = demo_command.variables.get("my-command-var").unwrap();
        assert_eq!(command_variable, &VariableConfig::Execution(ExecutionVariableConfig {
            execution: bash_exec("echo \"My command value\"", None),
            description: Some("Command level variable".to_string()),
            argument_name: Some("command-arg".to_string()),
        }))
    }

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
                    options: SelectOptionsConfig::Execution(ExecutionSelectOptionsConfig{
                        execution: raw_exec("cat example.txt")
                    }),
                })
            }
        }))
    }

    #[test]
    fn variable_order_is_preserved() {
        let yaml =
            "variables:
    root-var-3: Root value 3
    root-var-2: Root value 2
    root-var-1: Root value 1
commands:
    demo:
        variables:
            command-var-2: Command value 2
            command-var-1: Command value 1
            command-var-3: Command value 3
        action: echo \"Hello, World!\"";
        let config = parse_config(yaml).unwrap();

        assert!(!config.variables.is_empty());

        let root_variable_names: Vec<String> = config.variables.iter().map(|kv| kv.0.to_string()).collect();
        assert_eq!(root_variable_names[0], "root-var-3".to_string());
        assert_eq!(root_variable_names[1], "root-var-2".to_string());
        assert_eq!(root_variable_names[2], "root-var-1".to_string());

        let demo_command = config.commands.get("demo").unwrap();
        let command_variable_names: Vec<String> = demo_command.variables.iter().map(|kv| kv.0.to_string()).collect();
        assert_eq!(command_variable_names[0], "command-var-2".to_string());
        assert_eq!(command_variable_names[1], "command-var-1".to_string());
        assert_eq!(command_variable_names[2], "command-var-3".to_string());
    }

    // Todo: Command order is preserved

    #[test]
    fn single_action_command_parses() {
        let yaml =
            "commands:
    demo:
        action: ls";
        let config = parse_config(yaml).unwrap();

        let demo_command = config.commands.get("demo").unwrap();
        assert_eq!(demo_command, &CommandConfig {
            description: None,
            aliases: vec![],
            variables: Default::default(),
            commands: Default::default(),
            action: Some(CommandActionConfigVariant::SingleStep(SingleActionConfig {
                action: ActionConfig::Execution(ExecutionConfigVariant::RawCommand(RawCommandConfigVariant::Shorthand("ls".to_string()))),
            })),
        });
    }

    #[test]
    fn single_action_command_with_optional_fields_parses() {
        let yaml =
            "commands:
    demo:
        description: Says hello.
        aliases:
          - greet
          - hello
        action: ls";
        let config = parse_config(yaml).unwrap();

        let demo_command = config.commands.get("demo").unwrap();
        assert_eq!(demo_command, &CommandConfig {
            description: Some("Says hello.".to_string()),
            aliases: vec![
                "greet".to_string(),
                "hello".to_string()
            ],
            variables: Default::default(),
            commands: Default::default(),
            action: Some(CommandActionConfigVariant::SingleStep(SingleActionConfig {
                action: ActionConfig::Execution(ExecutionConfigVariant::RawCommand(RawCommandConfigVariant::Shorthand("ls".to_string()))),
            })),
        });
    }

    #[test]
    fn action_with_subcommands_parses() {
        let yaml =
            "commands:
    demo:
        commands:
            gday:
                action: ls
        action: cat example.txt";
        let config = parse_config(yaml).unwrap();

        let demo_command = config.commands.get("demo").unwrap();
        let gday_command = demo_command.commands.get("gday").unwrap();

        assert_eq!(gday_command, &CommandConfig {
            description: None,
            aliases: vec![],
            variables: Default::default(),
            commands: Default::default(),
            action: Some(CommandActionConfigVariant::SingleStep(SingleActionConfig {
                action: ActionConfig::Execution(ExecutionConfigVariant::RawCommand(RawCommandConfigVariant::Shorthand("ls".to_string()))),
            })),
        });

        let mut map = CommandConfigMap::new();
        map.insert("gday".to_string(), gday_command.clone());

        assert_eq!(demo_command, &CommandConfig {
            description: None,
            aliases: vec![],
            variables: Default::default(),
            commands: map,
            action: Some(CommandActionConfigVariant::SingleStep(SingleActionConfig {
                action: ActionConfig::Execution(ExecutionConfigVariant::RawCommand(RawCommandConfigVariant::Shorthand("cat example.txt".to_string()))),
            })),
        });
    }

    #[test]
    fn command_with_subcommands_only_parses() {
        let yaml =
            "commands:
    demo:
        commands:
            gday:
                action: ls";
        let config = parse_config(yaml).unwrap();

        let demo_command = config.commands.get("demo").unwrap();
        let gday_command = demo_command.commands.get("gday").unwrap();

        assert_eq!(gday_command, &CommandConfig {
            description: None,
            aliases: vec![],
            variables: Default::default(),
            commands: Default::default(),
            action: Some(CommandActionConfigVariant::SingleStep(SingleActionConfig {
                action: ActionConfig::Execution(ExecutionConfigVariant::RawCommand(RawCommandConfigVariant::Shorthand("ls".to_string()))),
            })),
        });

        let mut map = CommandConfigMap::new();
        map.insert("gday".to_string(), gday_command.clone());

        assert_eq!(demo_command, &CommandConfig {
            description: None,
            aliases: vec![],
            variables: Default::default(),
            commands: map,
            action: None,
        });
    }

    // Todo: Command with no subcommands or action - Fail

    #[test]
    fn command_with_multiple_actions_parses() {
        let yaml =
            "commands:
    demo:
        actions:
            - cat example.txt
            - ls";
        let config = parse_config(yaml).unwrap();

        let demo_command = config.commands.get("demo").unwrap();
        assert_eq!(demo_command, &CommandConfig {
            description: None,
            aliases: vec![],
            variables: Default::default(),
            commands: Default::default(),
            action: Some(CommandActionConfigVariant::MultiStep(MultiActionConfig {
                actions: vec![
                    ActionConfig::Execution(ExecutionConfigVariant::RawCommand(RawCommandConfigVariant::Shorthand("cat example.txt".to_string()))),
                    ActionConfig::Execution(ExecutionConfigVariant::RawCommand(RawCommandConfigVariant::Shorthand("ls".to_string()))),
                ],
            })),
        });
    }

    #[test]
    fn shell_action_parses() {
        let yaml =
            "commands:
    demo:
        actions:
            - bash: echo \"Hello, World!\"
            - bash: pwd
              working_directory: /";
        let config = parse_config(yaml).unwrap();

        let demo_command = config.commands.get("demo").unwrap();
        assert_eq!(demo_command, &CommandConfig {
            description: None,
            aliases: vec![],
            variables: Default::default(),
            commands: Default::default(),
            action: Some(CommandActionConfigVariant::MultiStep(MultiActionConfig {
                actions: vec![
                    ActionConfig::Execution(
                        ExecutionConfigVariant::ShellCommand(
                            ShellCommandConfigVariant::Bash(BashCommandConfig {
                                working_directory: None,
                                command: "echo \"Hello, World!\"".to_string(),
                            })
                        )
                    ),
                    ActionConfig::Execution(
                        ExecutionConfigVariant::ShellCommand(
                            ShellCommandConfigVariant::Bash(BashCommandConfig {
                                working_directory: Some("/".to_string()),
                                command: "pwd".to_string(),
                            })
                        )
                    ),
                ]
            })),
        });
    }

    #[test]
    fn confirm_action_parses() {
        let yaml =
            "commands:
    demo:
        action:
            confirm: Are you sure?";
        let config = parse_config(yaml).unwrap();

        let demo_command = config.commands.get("demo").unwrap();
        assert_eq!(demo_command, &CommandConfig {
            description: None,
            aliases: vec![],
            variables: Default::default(),
            commands: Default::default(),
            action: Some(CommandActionConfigVariant::SingleStep(SingleActionConfig {
                action: ActionConfig::Confirmation(ConfirmationCommandActionConfig {
                    confirm: "Are you sure?".to_string(),
                }),
            })),
        });
    }
}