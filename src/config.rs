use crate::platform::{current_platform_provider, is_current_platform};
use linked_hash_map::LinkedHashMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::IsTerminal;
use std::io::Read;
use std::path::PathBuf;
use std::{env, fs, io};
use thiserror::Error;

const CONFIG_FILE_NAMES: [&str; 4] = ["dingus.yaml", "Dingus.yaml", "dingus.yml", "Dingus.yml"];

const DEFAULT_CONFIG_FILE: &str = "description: My Dingus file

variables:
  name: Godzilla

commands:
  greet:
    action: echo \"Hello, $name!\"";

pub enum Source {
    Unknown,
    Stdin,
    File(PathBuf),
}

pub struct FoundConfig {
    pub source: Source,
    pub config: Config,
}

/// Loads the [`Config`] from stdin, or a file in the current directory.
pub fn load() -> Result<FoundConfig, ConfigError> {
    let input = io::stdin();

    let mut source = Source::Unknown;
    let mut config_text = String::new();

    if input.is_terminal() {
        let mut found = false;
        let mut directory = env::current_dir().unwrap();
        while !found {
            for config_file_name in CONFIG_FILE_NAMES {
                let config_file_path = directory.join(config_file_name);
                if !config_file_path.exists() {
                    continue;
                }

                source = Source::File(config_file_path.clone());
                config_text = fs::read_to_string(config_file_path)
                    .map_err(|err| ConfigError::ReadFailed(err))?;
                found = true;
                break;
            }

            if found {
                break;
            }

            if let Some(parent) = directory.parent() {
                directory = parent.to_owned();
            } else {
                break;
            }
        }

        if !found {
            return Err(ConfigError::FileNotFound);
        }
    } else {
        source = Source::Stdin;
        input
            .lock()
            .read_to_string(&mut config_text)
            .map_err(|err| ConfigError::ReadFailed(err))?;
    };

    let current_platform = current_platform_provider().get_platform();
    let config = parse_config(&config_text, current_platform)?;
    Ok(FoundConfig { source, config })
}

/// Creates a new config file in the current directory.
pub fn init() -> Result<String, ConfigError> {
    let file_name = CONFIG_FILE_NAMES[0];

    fs::write(file_name, DEFAULT_CONFIG_FILE).map_err(|io_err| ConfigError::WriteFailed(io_err))?;
    Ok(file_name.to_string())
}

fn parse_config_from(path: &String, current_platform: Platform) -> Result<Config, ConfigError> {
    let config_text = fs::read_to_string(path).map_err(|err| ConfigError::ReadFailed(err))?;

    parse_config(&config_text, current_platform)
}

fn parse_config(text: &String, current_platform: Platform) -> Result<Config, ConfigError> {
    // Parse the base config
    let mut base_config: Config =
        serde_yaml::from_str(text.as_str()).map_err(|err| ConfigError::ParseFailed(err))?;

    // Parse the imports too
    for import in &base_config.imports {
        // Don't even try parsing the import if it's not for the current platform
        if let Some(import_platform) = &import.platform {
            if !is_current_platform(current_platform.clone(), import_platform) {
                continue;
            }
        }

        let child_config =
            parse_config_from(&import.source, current_platform.clone()).map_err(|err| {
                ConfigError::ImportFailed {
                    alias: import.alias.clone(),
                    source: Box::new(err),
                }
            })?;

        // Create a top-level command for every import
        let command = CommandConfig {
            name: None,
            description: child_config.description,
            hidden: import.hidden,
            platform: import.platform.clone(),
            variables: child_config.variables,
            commands: child_config.commands,
            action: None,
        };

        base_config.commands.insert(import.alias.clone(), command);
    }

    Ok(base_config)
}

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("config file not found")]
    FileNotFound,

    #[error("failed to read config")]
    ReadFailed(#[source] io::Error),

    #[error("failed to write config file")]
    WriteFailed(#[source] io::Error),

    #[error("failed to parse config file")]
    ParseFailed(#[source] serde_yaml::Error),

    #[error("failed to import {alias}")]
    ImportFailed {
        alias: String,
        source: Box<ConfigError>, // Need to box this so the size isn't infinite
    },
}

/// The root-level of the Configuration.
#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    /// A list of additional config files to import.
    #[serde(default = "default_imports")]
    pub imports: Vec<Import>,

    /// A user-friendly description.
    #[serde(alias = "desc")]
    pub description: Option<String>,

    /// Root-level [`VariableConfig`]s that are available to all subsequent commands.
    #[serde(default = "default_variables")]
    #[serde(alias = "vars")]
    pub variables: VariableConfigMap,

    /// Top-level [`CommandConfig`]s.
    #[serde(alias = "cmds")]
    pub commands: CommandConfigMap,

    #[serde(default)]
    #[serde(alias = "opts")]
    pub options: DingusOptions,
}

fn default_imports() -> Vec<Import> {
    Vec::new()
}

fn default_variables() -> VariableConfigMap {
    VariableConfigMap::new()
}

fn default_commands() -> CommandConfigMap {
    CommandConfigMap::new()
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Import {
    pub alias: String,
    pub source: String, // TODO: Separate types for path, url, etc.

    /// Whether the imported commands should be hidden from the --help output.
    #[serde(default = "default_hidden")]
    pub hidden: bool,

    /// An optional platform to restrict this import to.
    /// When specified, the config will only be imported on the specified platforms.
    #[serde(flatten)]
    pub platform: Option<OneOrManyPlatforms>,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct DingusOptions {
    /// When set to `true`, commands will be printed to stdout before executing them.
    /// Defaults to `false`.
    #[serde(default = "default_print_commands")]
    pub print_commands: bool,

    /// When set to `true`, variables will be printed to stdout once they've been resolved.
    /// Defaults to `false`.
    #[serde(default = "default_print_variables")]
    pub print_variables: bool,

    /// When set to `true`, arguments will automatically be created for all variables.
    /// Defaults to `false`.
    #[serde(default = "default_auto_args")]
    pub auto_args: bool,
}

impl Default for DingusOptions {
    fn default() -> Self {
        DingusOptions {
            print_commands: default_print_commands(),
            print_variables: default_print_variables(),
            auto_args: default_auto_args(),
        }
    }
}

fn default_print_commands() -> bool {
    match env::var("DINGUS_PRINT_COMMANDS") {
        Ok(str) => is_truthy(str),
        Err(_) => false,
    }
}

fn default_print_variables() -> bool {
    match env::var("DINGUS_PRINT_VARIABLES") {
        Ok(str) => is_truthy(str),
        Err(_) => false,
    }
}

fn default_auto_args() -> bool {
    match env::var("DINGUS_AUTO_ARGS") {
        Ok(str) => is_truthy(str),
        Err(_) => false,
    }
}

fn is_truthy(s: String) -> bool {
    s == "true" || s == "TRUE" || s == "t" || s == "T"
}

/// A set of [`VariableConfig`].
/// Note that this uses a [`LinkedHashMap`] so that the order of insertion is retained.
pub type VariableConfigMap = LinkedHashMap<String, VariableConfig>;

/// The kind of variable.
#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
#[serde(untagged)]
pub enum VariableConfig {
    /// Denotes a shorthand literal variable.
    ShorthandLiteral(String),

    /// Encapsulates a [`LiteralVariableConfig`].
    Literal(LiteralVariableConfig),

    /// Encapsulates a [`ExecutionVariableConfig`].
    Execution(ExecutionVariableConfig),

    /// Encapsulates a [`PromptVariableConfig`].
    Prompt(PromptVariableConfig),
}

impl VariableConfig {
    pub fn environment_variable_name(&self, key: &str) -> String {
        match self {
            VariableConfig::ShorthandLiteral(_) => None,
            VariableConfig::Literal(literal_conf) => literal_conf.clone().environment_variable_name,
            VariableConfig::Execution(execution_conf) => {
                execution_conf.clone().environment_variable_name
            }
            VariableConfig::Prompt(prompt_conf) => prompt_conf.clone().environment_variable_name,
        }
        .unwrap_or(key.to_string())
    }

    pub fn description(&self) -> Option<String> {
        return match self {
            VariableConfig::ShorthandLiteral(_) => None,
            VariableConfig::Literal(literal_conf) => literal_conf.clone().description,
            VariableConfig::Execution(execution_conf) => execution_conf.clone().description,
            VariableConfig::Prompt(prompt_config) => prompt_config.clone().description,
        };
    }
}

/// Denotes a literal variable where the value is hard-coded.
///
/// Example:
/// ```yaml
/// name:
///     description: Your name
///     arg: name
///     value: Dingus
/// ```
#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct LiteralVariableConfig {
    /// An optional description for the variable.
    #[serde(alias = "desc")]
    pub description: Option<String>,

    /// An optional argument configuration.
    #[serde(rename(deserialize = "argument"))]
    #[serde(alias = "arg")]
    pub argument: Option<ArgumentConfigVariant>,

    /// An optional environment variable name.
    /// If specified, the environment variable for this variable will have the specified name.
    ///
    /// This is **not** the name of the environment variable to source the value from.
    /// If you want to source a variables value from an environment variable,
    /// use an [`ExecutionVariableConfig`].
    #[serde(rename(deserialize = "environment_variable"))]
    #[serde(alias = "env")]
    pub environment_variable_name: Option<String>,

    /// The value of the variable
    pub value: String,
}

/// Denotes a variable whose value is determined by the output of a command.
///
/// Example:
/// ```yaml
/// name:
///     description: Your name
///     arg: name
///     exec: cat name.txt
/// ```
#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct ExecutionVariableConfig {
    /// An optional description for the variable.
    #[serde(alias = "desc")]
    pub description: Option<String>,

    /// An optional argument configuration.
    #[serde(rename(deserialize = "argument"))]
    #[serde(alias = "arg")]
    pub argument: Option<ArgumentConfigVariant>,

    /// An optional environment variable name.
    /// If specified, the environment variable for this variable will have the specified name.
    ///
    /// This is **not** the name of the environment variable to source the value from.
    /// If you want to source a variables value from an environment variable,
    /// use an [`ExecutionVariableConfig`].
    #[serde(rename(deserialize = "environment_variable"))]
    #[serde(alias = "env")]
    pub environment_variable_name: Option<String>,

    /// The [`ExecutionConfigVariant`] to use to determine the value of this variable.
    #[serde(rename = "execute")]
    #[serde(alias = "exec")]
    pub execution: ExecutionConfigVariant,
}

/// Denotes a variable whose value is determined by prompting the user for input.
///
/// Example:
/// ```yaml
/// name:
///     description: Your name
///     arg: name
///     prompt:
///         message: What is your name?
/// ```
#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct PromptVariableConfig {
    /// An optional description for the variable.
    #[serde(alias = "desc")]
    pub description: Option<String>,

    /// An optional argument configuration.
    #[serde(rename(deserialize = "argument"))]
    #[serde(alias = "arg")]
    pub argument: Option<ArgumentConfigVariant>,

    /// An optional environment variable name.
    /// If specified, the environment variable for this variable will have the specified name.
    ///
    /// This is **not** the name of the environment variable to source the value from.
    /// If you want to source a variables value from an environment variable,
    /// use an [`ExecutionVariableConfig`].
    #[serde(rename(deserialize = "environment_variable"))]
    #[serde(alias = "env")]
    pub environment_variable_name: Option<String>,

    /// The [`PromptConfig`] to use for the prompt.
    pub prompt: PromptConfig,
}

/// The kind of argument configuration.
#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
#[serde(untagged)]
pub enum ArgumentConfigVariant {
    Shorthand(String),
    Named(NamedArgumentConfig),
    Positional(PositionalArgumentConfig)
}

/// The configuration for a command-line argument.
#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct NamedArgumentConfig {
    /// The long version of the argument without the preceding `--`.
    pub long: String,

    /// The short version of the argument without the preceding `-`.
    pub short: Option<char>,
}

/// The configuration for a positional command-line argument.
#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct PositionalArgumentConfig {

    /// The position of the argument.
    /// This refers to position according to other positional argument.
    /// It does not define the position in the argument list as a whole.
    /// https://docs.rs/clap/latest/clap/struct.Arg.html#method.index
    pub position: usize,
}

/// The configuration for a prompt to the user for input.
#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct PromptConfig {
    /// The message to display to the user.
    pub message: String,

    /// Additional, type-specific options for the prompt.
    #[serde(flatten)]
    pub options: PromptOptionsVariant,
}

impl Default for PromptOptionsVariant {
    fn default() -> Self {
        return PromptOptionsVariant::Text(TextPromptOptions {
            multi_line: false,
            sensitive: false,
        });
    }
}

/// The kind of prompt options.
#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
#[serde(untagged)]
pub enum PromptOptionsVariant {
    // Note: Select needs to come first here because SelectPromptOptions is the most specific.
    // Serde will use the type it matches on.
    /// Encapsulates a [`SelectPromptOptions]`, indicating that the prompt should be a select-style
    /// prompt.
    Select(SelectPromptOptions),

    /// Encapsulates a [`TextPromptOptions]`, indicating that the prompt should be a text prompt.
    Text(TextPromptOptions),
}

/// The options for a text prompt
#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct TextPromptOptions {
    /// Whether the prompt should be multi-line.
    #[serde(default = "default_multi_line")]
    pub multi_line: bool,

    /// Whether the prompt is for a sensitive value.
    /// When set to `true`, the input value will be obscured.
    #[serde(default = "default_sensitive")]
    pub sensitive: bool,
}

fn default_multi_line() -> bool {
    false
}

fn default_sensitive() -> bool {
    false
}

/// The options for a select prompt.
#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct SelectPromptOptions {
    /// The [`SelectOptionsConfig`] for determining the options the user can choose from.
    #[serde(alias = "opts")]
    pub options: SelectOptionsConfig,
}

/// The kind of select prompt options.
#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
#[serde(untagged)]
pub enum SelectOptionsConfig {
    /// Encapsulates an [`ExecutionSelectOptionsConfig`], indicating that the options should be
    /// sourced from the output of a command.
    Execution(ExecutionSelectOptionsConfig),

    /// Encapsulates a `Vec<String>` where each element is an option that the user can choose.
    Literal(Vec<String>),
}

/// Encapsulates a [`ExecutionConfigVariant`] for use in [`SelectOptionsConfig::Execution`].
#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct ExecutionSelectOptionsConfig {
    /// The [`ExecutionConfigVariant`] to use to determine the options.
    #[serde(rename = "execute")]
    #[serde(alias = "exec")]
    pub execution: ExecutionConfigVariant,
}

pub type CommandConfigMap = HashMap<String, CommandConfig>;

/// The configuration for a command.
#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct CommandConfig {
    /// An optional name for the command. Setting this will override the name provided by the key.
    pub name: Option<String>,

    /// An optional description for the command.
    #[serde(alias = "desc")]
    pub description: Option<String>,

    /// Whether the command should be hidden from the --help output.
    #[serde(default = "default_hidden")]
    pub hidden: bool,

    /// An optional platform to restrict this command to.
    /// When specified, the command will only be available on the specified platforms.
    #[serde(flatten)]
    pub platform: Option<OneOrManyPlatforms>,

    /// The [`VariableConfig`]s associated with this [`CommandConfig`] and it's subcommands.
    #[serde(default = "default_variables")]
    #[serde(alias = "vars")]
    pub variables: VariableConfigMap,

    // TODO: Need to enforce an invariant here:
    // - If no action exists, then one or more subcommands _must_ exist.
    /// Any sub-[`CommandConfig`]s.
    #[serde(default = "default_commands")]
    #[serde(alias = "cmds")]
    pub commands: CommandConfigMap,

    /// The [`ActionConfig`] that this command will perform when executed.
    #[serde(flatten)]
    pub action: Option<ActionConfig>,
}

fn default_hidden() -> bool {
    false
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
#[serde(untagged)]
pub enum OneOrManyPlatforms {
    One(OnePlatform),
    Many(ManyPlatforms),
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct OnePlatform {
    pub platform: Platform,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct ManyPlatforms {
    pub platforms: Vec<Platform>,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub enum Platform {
    MacOS,
    Windows,
    Linux,
}

/// Encapsulates either a single [`ExecutionConfigVariant`] ([`ActionConfig::SingleStep`] with a [`SingleActionConfig`])
/// or multiple [`ExecutionConfigVariant`] ([`ActionConfig::MultiStep`] with a [`MultiActionConfig`]).
#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
#[serde(untagged)]
pub enum ActionConfig {
    SingleStep(SingleActionConfig),
    MultiStep(MultiActionConfig),
    Alias(AliasActionConfig),
}

/// Contains the prefix for a command to execute.
#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct AliasActionConfig {
    pub alias: String,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct SingleActionConfig {
    pub action: ExecutionConfigVariant,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct MultiActionConfig {
    pub actions: Vec<ExecutionConfigVariant>,
}

/// The kind of command to execute.
#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
#[serde(untagged)]
pub enum ExecutionConfigVariant {
    /// Encapsulates a [`ShellCommandConfigVariant`].
    ShellCommand(ShellCommandConfigVariant),

    /// Encapsulates a [`RawCommandConfigVariant`].
    RawCommand(RawCommandConfigVariant),
}

/// The configuration for a raw command.
/// Raw commands are simply commands executed without a shell.
#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
#[serde(untagged)]
pub enum RawCommandConfigVariant {
    /// Denotes a shorthand execution.
    ///
    /// Example:
    /// ```yaml
    /// exec: cat example.txt
    /// ```
    Shorthand(String),

    /// Encapsulates a [`RawCommandConfig`].
    RawCommandConfig(RawCommandConfig),
}

/// The configuration for a raw command.
#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct RawCommandConfig {
    /// An optional working directory for the command to be executed in.
    /// If not specified, then the command will be executed in the current directory.
    #[serde(rename = "workdir")]
    #[serde(alias = "wd")]
    pub working_directory: Option<String>,

    /// The command to execute.
    #[serde(alias = "cmd")]
    pub command: String,
}

/// The configuration for a shell command.
#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
#[serde(untagged)]
pub enum ShellCommandConfigVariant {
    /// Encapsulates a [`BashCommandConfig`].
    Bash(BashCommandConfig),
}

/// The configuration for a bash command.
#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct BashCommandConfig {
    /// An optional working directory for the command to be executed in.
    /// If not specified, then the command will be executed in the current directory.
    #[serde(rename = "workdir")]
    #[serde(alias = "wd")]
    pub working_directory: Option<String>,

    /// The command to execute.
    #[serde(rename = "bash")]
    #[serde(alias = "sh")]
    pub command: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::OneOrManyPlatforms::{Many, One};
    use crate::config::Platform::Linux;
    use crate::config::RawCommandConfigVariant::Shorthand;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn bash_exec(command: &str, workdir: Option<String>) -> ExecutionConfigVariant {
        return ExecutionConfigVariant::ShellCommand(ShellCommandConfigVariant::Bash(
            BashCommandConfig {
                working_directory: workdir,
                command: command.to_string(),
            },
        ));
    }

    fn raw_exec(command: &str) -> ExecutionConfigVariant {
        return ExecutionConfigVariant::RawCommand(RawCommandConfigVariant::Shorthand(
            command.to_string(),
        ));
    }

    #[test]
    fn empty_root_variables_allowed() {
        let yaml = "commands:
    demo:
        action: echo \"Hello, World!\"";
        let config = parse_config(&yaml.to_string(), Platform::Linux).unwrap();

        assert!(config.variables.is_empty());
    }

    #[test]
    fn shorthand_literal_variable_parsed() {
        let yaml = "variables:
    my-root-var: My root value
commands:
    demo:
        variables:
            my-command-var: My command value
        action: echo \"Hello, World!\"";
        let config = parse_config(&yaml.to_string(), Platform::Linux).unwrap();

        assert!(!config.variables.is_empty());

        let root_variable = config.variables.get("my-root-var").unwrap();
        assert_eq!(
            root_variable,
            &VariableConfig::ShorthandLiteral("My root value".to_string())
        );

        let demo_command = config.commands.get("demo").unwrap();
        let command_variable = demo_command.variables.get("my-command-var").unwrap();
        assert_eq!(
            command_variable,
            &VariableConfig::ShorthandLiteral("My command value".to_string())
        )
    }

    #[test]
    fn literal_variable_parsed() {
        let yaml = "variables:
    my-root-var:
        value: My root value
commands:
    demo:
        variables:
            my-command-var:
                value: My command value
                description: Command level variable
                arg: command-arg
                env: MY_VAR
        action: echo \"Hello, World!\"";
        let config = parse_config(&yaml.to_string(), Platform::Linux).unwrap();

        assert!(!config.variables.is_empty());

        let root_variable = config.variables.get("my-root-var").unwrap();
        assert_eq!(
            root_variable,
            &VariableConfig::Literal(LiteralVariableConfig {
                value: "My root value".to_string(),
                description: None,
                argument: None,
                environment_variable_name: None,
            })
        );

        let demo_command = config.commands.get("demo").unwrap();
        let command_variable = demo_command.variables.get("my-command-var").unwrap();
        assert_eq!(
            command_variable,
            &VariableConfig::Literal(LiteralVariableConfig {
                value: "My command value".to_string(),
                description: Some("Command level variable".to_string()),
                argument: Some(ArgumentConfigVariant::Shorthand("command-arg".to_string())),
                environment_variable_name: Some("MY_VAR".to_string()),
            })
        )
    }

    #[test]
    fn exec_variable_parsed() {
        let yaml = "variables:
    my-root-var:
        exec:
            sh: echo \"My root value\"
            workdir: ../
commands:
    demo:
        variables:
            my-command-var-1:
                exec:
                    bash: echo \"My command value\"
                description: Command level variable
                arg: command-arg-1
                env: MY_VAR_1
            my-command-var-2:
                exec:
                    bash: echo \"My command value\"
                description: Command level variable
                arg:
                    long: command-arg-2
                    short: c
                env: MY_VAR_2
            my-command-var-3:
                exec:
                    bash: echo \"My command value\"
                description: Command level variable
                arg:
                    position: 1
                env: MY_VAR_3
        action: echo \"Hello, World!\"";
        let config = parse_config(&yaml.to_string(), Platform::Linux).unwrap();

        assert!(!config.variables.is_empty());

        let root_variable = config.variables.get("my-root-var").unwrap();
        assert_eq!(
            root_variable,
            &VariableConfig::Execution(ExecutionVariableConfig {
                execution: bash_exec("echo \"My root value\"", Some("../".to_string())),
                description: None,
                argument: None,
                environment_variable_name: None,
            })
        );

        let demo_command = config.commands.get("demo").unwrap();
        let command_variable_1 = demo_command.variables.get("my-command-var-1").unwrap();
        assert_eq!(
            command_variable_1,
            &VariableConfig::Execution(ExecutionVariableConfig {
                execution: bash_exec("echo \"My command value\"", None),
                description: Some("Command level variable".to_string()),
                argument: Some(ArgumentConfigVariant::Shorthand(
                    "command-arg-1".to_string()
                )),
                environment_variable_name: Some("MY_VAR_1".to_string()),
            })
        );

        let command_variable_2 = demo_command.variables.get("my-command-var-2").unwrap();
        assert_eq!(
            command_variable_2,
            &VariableConfig::Execution(ExecutionVariableConfig {
                execution: bash_exec("echo \"My command value\"", None),
                description: Some("Command level variable".to_string()),
                argument: Some(ArgumentConfigVariant::Named(NamedArgumentConfig {
                    long: "command-arg-2".to_string(),
                    short: Some('c'),
                })),
                environment_variable_name: Some("MY_VAR_2".to_string()),
            })
        );

        let command_variable_3 = demo_command.variables.get("my-command-var-3").unwrap();
        assert_eq!(
            command_variable_3,
            &VariableConfig::Execution(ExecutionVariableConfig {
                execution: bash_exec("echo \"My command value\"", None),
                description: Some("Command level variable".to_string()),
                argument: Some(ArgumentConfigVariant::Positional(PositionalArgumentConfig {
                    position: 1,
                })),
                environment_variable_name: Some("MY_VAR_3".to_string()),
            })
        )
    }

    #[test]
    fn prompt_variable_parsed() {
        let yaml = "variables:
    name:
        prompt:
            message: What's your name?
    food:
        description: Favourite food
        arg: food
        env: FAV_FOOD
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
        let config = parse_config(&yaml.to_string(), Platform::Linux).unwrap();

        assert!(!config.variables.is_empty());

        let name_variable = config.variables.get("name").unwrap();
        assert_eq!(
            name_variable,
            &VariableConfig::Prompt(PromptVariableConfig {
                description: None,
                argument: None,
                environment_variable_name: None,
                prompt: PromptConfig {
                    message: "What's your name?".to_string(),
                    options: PromptOptionsVariant::Text(TextPromptOptions {
                        multi_line: false,
                        sensitive: false,
                    })
                },
            })
        );

        let food_variable = config.variables.get("food").unwrap();
        assert_eq!(
            food_variable,
            &VariableConfig::Prompt(PromptVariableConfig {
                description: Some("Favourite food".to_string()),
                argument: Some(ArgumentConfigVariant::Shorthand("food".to_string())),
                environment_variable_name: Some("FAV_FOOD".to_string()),
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
            })
        );

        let demo_command = config.commands.get("demo").unwrap();
        let password_variable = demo_command.variables.get("password").unwrap();
        assert_eq!(
            password_variable,
            &VariableConfig::Prompt(PromptVariableConfig {
                description: None,
                argument: None,
                environment_variable_name: None,
                prompt: PromptConfig {
                    message: "What's your password?".to_string(),
                    options: PromptOptionsVariant::Text(TextPromptOptions {
                        multi_line: false,
                        sensitive: true
                    })
                },
            })
        );

        let life_story_variable = demo_command.variables.get("life-story").unwrap();
        assert_eq!(
            life_story_variable,
            &VariableConfig::Prompt(PromptVariableConfig {
                description: None,
                argument: None,
                environment_variable_name: None,
                prompt: PromptConfig {
                    message: "What's your life story?".to_string(),
                    options: PromptOptionsVariant::Text(TextPromptOptions {
                        multi_line: true,
                        sensitive: false
                    })
                },
            })
        );

        let fav_line_variable = demo_command.variables.get("favourite-line").unwrap();
        assert_eq!(
            fav_line_variable,
            &VariableConfig::Prompt(PromptVariableConfig {
                description: None,
                argument: None,
                environment_variable_name: None,
                prompt: PromptConfig {
                    message: "What's your favourite line?".to_string(),
                    options: PromptOptionsVariant::Select(SelectPromptOptions {
                        options: SelectOptionsConfig::Execution(ExecutionSelectOptionsConfig {
                            execution: raw_exec("cat example.txt")
                        }),
                    })
                }
            })
        )
    }

    #[test]
    fn variable_order_is_preserved() {
        let yaml = "variables:
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
        let config = parse_config(&yaml.to_string(), Platform::Linux).unwrap();

        assert!(!config.variables.is_empty());

        let root_variable_names: Vec<String> =
            config.variables.iter().map(|kv| kv.0.to_string()).collect();
        assert_eq!(root_variable_names[0], "root-var-3".to_string());
        assert_eq!(root_variable_names[1], "root-var-2".to_string());
        assert_eq!(root_variable_names[2], "root-var-1".to_string());

        let demo_command = config.commands.get("demo").unwrap();
        let command_variable_names: Vec<String> = demo_command
            .variables
            .iter()
            .map(|kv| kv.0.to_string())
            .collect();
        assert_eq!(command_variable_names[0], "command-var-2".to_string());
        assert_eq!(command_variable_names[1], "command-var-1".to_string());
        assert_eq!(command_variable_names[2], "command-var-3".to_string());
    }

    // TODO: Command order is preserved

    #[test]
    fn single_action_command_parses() {
        let yaml = "commands:
    demo:
        action: ls";
        let config = parse_config(&yaml.to_string(), Platform::Linux).unwrap();

        let demo_command = config.commands.get("demo").unwrap();
        assert_eq!(
            demo_command,
            &CommandConfig {
                name: None,
                description: None,
                hidden: false,
                platform: None,
                variables: Default::default(),
                commands: Default::default(),
                action: Some(ActionConfig::SingleStep(SingleActionConfig {
                    action: ExecutionConfigVariant::RawCommand(RawCommandConfigVariant::Shorthand(
                        "ls".to_string()
                    )),
                })),
            }
        );
    }

    #[test]
    fn alias_command_parses() {
        let yaml = "commands:
    deps:
        alias: docker compose -f docker-compose.deps.yml";
        let config = parse_config(&yaml.to_string(), Platform::Linux).unwrap();

        let demo_command = config.commands.get("deps").unwrap();
        assert_eq!(
            demo_command,
            &CommandConfig {
                name: None,
                description: None,
                hidden: false,
                platform: None,
                variables: Default::default(),
                commands: Default::default(),
                action: Some(ActionConfig::Alias(AliasActionConfig {
                    alias: "docker compose -f docker-compose.deps.yml".to_string()
                })),
            }
        );
    }

    #[test]
    fn single_action_command_with_optional_fields_parses() {
        let yaml = "commands:
    demo:
        description: Says hello.
        action: ls";
        let config = parse_config(&yaml.to_string(), Platform::Linux).unwrap();

        let demo_command = config.commands.get("demo").unwrap();
        assert_eq!(
            demo_command,
            &CommandConfig {
                name: None,
                platform: None,
                description: Some("Says hello.".to_string()),
                hidden: false,
                variables: Default::default(),
                commands: Default::default(),
                action: Some(ActionConfig::SingleStep(SingleActionConfig {
                    action: ExecutionConfigVariant::RawCommand(RawCommandConfigVariant::Shorthand(
                        "ls".to_string()
                    )),
                })),
            }
        );
    }

    #[test]
    fn action_with_subcommands_parses() {
        let yaml = "commands:
    demo:
        commands:
            gday:
                action: ls
        action: cat example.txt";
        let config = parse_config(&yaml.to_string(), Platform::Linux).unwrap();

        let demo_command = config.commands.get("demo").unwrap();
        let gday_command = demo_command.commands.get("gday").unwrap();

        assert_eq!(
            gday_command,
            &CommandConfig {
                name: None,
                description: None,
                hidden: false,
                platform: None,
                variables: Default::default(),
                commands: Default::default(),
                action: Some(ActionConfig::SingleStep(SingleActionConfig {
                    action: ExecutionConfigVariant::RawCommand(RawCommandConfigVariant::Shorthand(
                        "ls".to_string()
                    )),
                })),
            }
        );

        let mut map = CommandConfigMap::new();
        map.insert("gday".to_string(), gday_command.clone());

        assert_eq!(
            demo_command,
            &CommandConfig {
                name: None,
                description: None,
                hidden: false,
                platform: None,
                variables: Default::default(),
                commands: map,
                action: Some(ActionConfig::SingleStep(SingleActionConfig {
                    action: ExecutionConfigVariant::RawCommand(RawCommandConfigVariant::Shorthand(
                        "cat example.txt".to_string()
                    )),
                })),
            }
        );
    }

    #[test]
    fn command_with_subcommands_only_parses() {
        let yaml = "commands:
    demo:
        commands:
            gday:
                action: ls";
        let config = parse_config(&yaml.to_string(), Platform::Linux).unwrap();

        let demo_command = config.commands.get("demo").unwrap();
        let gday_command = demo_command.commands.get("gday").unwrap();

        assert_eq!(
            gday_command,
            &CommandConfig {
                name: None,
                description: None,
                hidden: false,
                platform: None,
                variables: Default::default(),
                commands: Default::default(),
                action: Some(ActionConfig::SingleStep(SingleActionConfig {
                    action: ExecutionConfigVariant::RawCommand(RawCommandConfigVariant::Shorthand(
                        "ls".to_string()
                    )),
                })),
            }
        );

        let mut map = CommandConfigMap::new();
        map.insert("gday".to_string(), gday_command.clone());

        assert_eq!(
            demo_command,
            &CommandConfig {
                name: None,
                description: None,
                hidden: false,
                platform: None,
                variables: Default::default(),
                commands: map,
                action: None,
            }
        );
    }

    // TODO: Command with no subcommands or action - Fail

    #[test]
    fn command_with_multiple_actions_parses() {
        let yaml = "commands:
    demo:
        actions:
            - cat example.txt
            - ls";
        let config = parse_config(&yaml.to_string(), Platform::Linux).unwrap();

        let demo_command = config.commands.get("demo").unwrap();
        assert_eq!(
            demo_command,
            &CommandConfig {
                name: None,
                description: None,
                hidden: false,
                platform: None,
                variables: Default::default(),
                commands: Default::default(),
                action: Some(ActionConfig::MultiStep(MultiActionConfig {
                    actions: vec![
                        ExecutionConfigVariant::RawCommand(RawCommandConfigVariant::Shorthand(
                            "cat example.txt".to_string()
                        )),
                        ExecutionConfigVariant::RawCommand(RawCommandConfigVariant::Shorthand(
                            "ls".to_string()
                        )),
                    ],
                })),
            }
        );
    }

    #[test]
    fn commands_with_specific_platforms_parse() {
        let yaml = "commands:
    demo_nix:
        platforms:
            - Linux
            - MacOS
        action: cat example.txt
    demo_win:
        platform: Windows
        action: Get-Content example.txt";
        let config = parse_config(&yaml.to_string(), Platform::Linux).unwrap();

        let demo_command_nix = config.commands.get("demo_nix").unwrap();
        let demo_command_win = config.commands.get("demo_win").unwrap();
        assert_eq!(
            demo_command_nix,
            &CommandConfig {
                name: None,
                description: None,
                hidden: false,
                platform: Some(Many(ManyPlatforms {
                    platforms: vec![Platform::Linux, Platform::MacOS]
                })),
                variables: Default::default(),
                commands: Default::default(),
                action: Some(ActionConfig::SingleStep(SingleActionConfig {
                    action: ExecutionConfigVariant::RawCommand(RawCommandConfigVariant::Shorthand(
                        "cat example.txt".to_string()
                    ))
                })),
            }
        );

        assert_eq!(
            demo_command_win,
            &CommandConfig {
                name: None,
                description: None,
                hidden: false,
                platform: Some(One(OnePlatform {
                    platform: Platform::Windows
                })),
                variables: Default::default(),
                commands: Default::default(),
                action: Some(ActionConfig::SingleStep(SingleActionConfig {
                    action: ExecutionConfigVariant::RawCommand(RawCommandConfigVariant::Shorthand(
                        "Get-Content example.txt".to_string()
                    ))
                })),
            }
        );
    }

    #[test]
    fn commands_with_name_parse() {
        let yaml = "commands:
    demo:
        name: demonstration
        action: cat example.txt";
        let config = parse_config(&yaml.to_string(), Platform::Linux).unwrap();

        let demo_command = config.commands.get("demo").unwrap();
        assert_eq!(
            demo_command,
            &CommandConfig {
                name: Some("demonstration".to_string()),
                description: None,
                hidden: false,
                platform: None,
                variables: Default::default(),
                commands: Default::default(),
                action: Some(ActionConfig::SingleStep(SingleActionConfig {
                    action: ExecutionConfigVariant::RawCommand(RawCommandConfigVariant::Shorthand(
                        "cat example.txt".to_string()
                    ))
                })),
            }
        );
    }

    #[test]
    fn shell_action_parses() {
        let yaml = "commands:
    demo:
        actions:
            - bash: echo \"Hello, World!\"
            - bash: pwd
              workdir: /";
        let config = parse_config(&yaml.to_string(), Platform::Linux).unwrap();

        let demo_command = config.commands.get("demo").unwrap();
        assert_eq!(
            demo_command,
            &CommandConfig {
                name: None,
                description: None,
                hidden: false,
                platform: None,
                variables: Default::default(),
                commands: Default::default(),
                action: Some(ActionConfig::MultiStep(MultiActionConfig {
                    actions: vec![
                        ExecutionConfigVariant::ShellCommand(ShellCommandConfigVariant::Bash(
                            BashCommandConfig {
                                working_directory: None,
                                command: "echo \"Hello, World!\"".to_string(),
                            }
                        )),
                        ExecutionConfigVariant::ShellCommand(ShellCommandConfigVariant::Bash(
                            BashCommandConfig {
                                working_directory: Some("/".to_string()),
                                command: "pwd".to_string(),
                            }
                        )),
                    ]
                })),
            }
        );
    }

    #[test]
    fn import() {
        let yaml3 = "variables:
    age: Forty Two
commands:
    demo:
        action: echo \"You are $age years old.\"";
        let yaml3_file = create_temp_file(yaml3);

        let yaml2 = format!(
            "imports:
    - alias: level-3
      source: {}
      hidden: true
variables:
    last_name: Smith
commands:
    demo:
        action: echo \"Your last name is $last_name!\"",
            yaml3_file.path().to_str().unwrap()
        );
        let yaml2_file = create_temp_file(yaml2.as_str());

        let yaml1 = format!(
            "imports:
    - alias: level-2
      source: {}
      platform: Linux
variables:
    first_name: Dingus
commands:
    demo:
        action: echo \"Your first name is $first_name!\"",
            yaml2_file.path().to_str().unwrap()
        );

        let config = parse_config(&yaml1.to_string(), Platform::Linux).unwrap();

        let root_demo_command = config.commands.get("demo").unwrap();
        assert_eq!(
            root_demo_command.action,
            Some(ActionConfig::SingleStep(SingleActionConfig {
                action: ExecutionConfigVariant::RawCommand(Shorthand(
                    "echo \"Your first name is $first_name!\"".to_string()
                ))
            }))
        );
        assert_eq!(
            config.variables.get("first_name").unwrap(),
            &VariableConfig::ShorthandLiteral("Dingus".to_string())
        );

        let second_level_command = config.commands.get("level-2").unwrap();
        assert_eq!(
            second_level_command.commands.get("demo").unwrap().action,
            Some(ActionConfig::SingleStep(SingleActionConfig {
                action: ExecutionConfigVariant::RawCommand(Shorthand(
                    "echo \"Your last name is $last_name!\"".to_string()
                ))
            }))
        );
        assert_eq!(
            second_level_command.platform,
            Some(One(OnePlatform { platform: Linux }))
        );
        assert_eq!(
            second_level_command.variables.get("last_name").unwrap(),
            &VariableConfig::ShorthandLiteral("Smith".to_string())
        );

        let third_level_command = second_level_command.commands.get("level-3").unwrap();
        assert_eq!(
            third_level_command.commands.get("demo").unwrap().action,
            Some(ActionConfig::SingleStep(SingleActionConfig {
                action: ExecutionConfigVariant::RawCommand(Shorthand(
                    "echo \"You are $age years old.\"".to_string()
                ))
            }))
        );
        assert_eq!(third_level_command.hidden, true);
        assert_eq!(
            third_level_command.variables.get("age").unwrap(),
            &VariableConfig::ShorthandLiteral("Forty Two".to_string())
        );
    }

    #[test]
    fn import_for_other_platform_is_ignored() {
        let yaml2 = "commands:
    demo:
        action: echo \"Your last name is $last_name!\""
            .to_string();
        let yaml2_file = create_temp_file(yaml2.as_str());

        let yaml1 = format!(
            "imports:
    - alias: other
      source: {}
      platform: Windows
variables:
    first_name: Dingus
commands:
    demo:
        action: echo \"Your first name is $first_name!\"",
            yaml2_file.path().to_str().unwrap()
        );

        let config = parse_config(&yaml1.to_string(), Platform::Linux).unwrap();

        let root_demo_command = config.commands.get("demo").unwrap();
        assert_eq!(
            root_demo_command.action,
            Some(ActionConfig::SingleStep(SingleActionConfig {
                action: ExecutionConfigVariant::RawCommand(Shorthand(
                    "echo \"Your first name is $first_name!\"".to_string()
                ))
            }))
        );
        assert_eq!(
            config.variables.get("first_name").unwrap(),
            &VariableConfig::ShorthandLiteral("Dingus".to_string())
        );

        let second_level_command = config.commands.get("other");
        assert_eq!(second_level_command, None);
    }

    fn create_temp_file(content: &str) -> NamedTempFile {
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(content.as_bytes()).unwrap();
        return temp_file;
    }
}
