use crate::platform::{current_platform_provider, is_current_platform};
use linked_hash_map::LinkedHashMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::IsTerminal;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::{env, fs, io};
use thiserror::Error;

const CONFIG_FILE_NAMES: [&str; 4] = ["plz.yaml", "Plz.yaml", "plz.yml", "Plz.yml"];

const DEFAULT_CONFIG_FILE: &str = "description: My plzfile

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
    let base_dir = match &source {
        Source::File(path) => path.parent().map(|p| p.to_path_buf()),
        _ => None,
    };
    let config = parse_config(&config_text, current_platform, base_dir.as_deref())?;
    Ok(FoundConfig { source, config })
}

/// Creates a new config file in the current directory.
pub fn init() -> Result<String, ConfigError> {
    let file_name = CONFIG_FILE_NAMES[0];

    fs::write(file_name, DEFAULT_CONFIG_FILE).map_err(|io_err| ConfigError::WriteFailed(io_err))?;
    Ok(file_name.to_string())
}

fn parse_config_from(path: &Path, current_platform: Platform) -> Result<Config, ConfigError> {
    let config_text = fs::read_to_string(path).map_err(|err| ConfigError::ReadFailed(err))?;
    let base_dir = path.parent();
    parse_config(&config_text, current_platform, base_dir)
}

fn parse_config(
    text: &String,
    current_platform: Platform,
    base_dir: Option<&Path>,
) -> Result<Config, ConfigError> {
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

        let import_path = {
            let raw = PathBuf::from(&import.source);
            if raw.is_relative() {
                if let Some(dir) = base_dir {
                    normalize_path(&dir.join(&raw))
                } else {
                    raw
                }
            } else {
                raw
            }
        };

        let mut child_config =
            parse_config_from(&import_path, current_platform.clone()).map_err(|err| {
                ConfigError::ImportFailed {
                    alias: import.alias.clone(),
                    source: Box::new(err),
                }
            })?;

        // Resolve working directories in the imported config relative to its location
        if let Some(import_dir) = import_path.parent() {
            resolve_variable_working_dirs(&mut child_config.variables, import_dir);
            resolve_command_working_dirs(&mut child_config.commands, import_dir);
        }

        // Create a top-level command for every import
        let command = CommandConfig {
            name: None,
            description: child_config.description,
            hidden: import.hidden,
            platform: import.platform.clone(),
            variables: child_config.variables,
            commands: child_config.commands,
            action: None,
            defer: None,
        };

        base_config.commands.insert(import.alias.clone(), command);
    }

    Ok(base_config)
}

/// Normalizes a path by resolving `.` and `..` components without touching the filesystem.
fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            c => normalized.push(c),
        }
    }
    normalized
}

/// Resolves the working directory for a single execution config relative to `base_dir`.
/// - If the execution has no working directory, sets it to `base_dir`.
/// - If the execution has a relative working directory, resolves it against `base_dir`.
/// - Absolute working directories are left unchanged.
fn resolve_exec_workdir(exec: &mut ExecutionConfigVariant, base_dir: &Path) {
    match exec {
        ExecutionConfigVariant::ShellCommand(ShellCommandConfigVariant::Bash(bash)) => {
            bash.working_directory = Some(resolve_dir(bash.working_directory.as_deref(), base_dir));
        }
        ExecutionConfigVariant::RawCommand(raw) => match raw {
            RawCommandConfigVariant::Shorthand(cmd) => {
                *raw = RawCommandConfigVariant::RawCommandConfig(RawCommandConfig {
                    command: cmd.clone(),
                    working_directory: Some(base_dir.to_string_lossy().to_string()),
                });
            }
            RawCommandConfigVariant::RawCommandConfig(config) => {
                config.working_directory =
                    Some(resolve_dir(config.working_directory.as_deref(), base_dir));
            }
        },
    }
}

/// Returns an absolute working directory.
/// If `workdir` is none, then the `base_dir` is returned.
/// If `workdir` is a relative path, it is joined with `base_dir`.
/// If `workdir` is an absolute path, it is returned as-is.
fn resolve_dir(workdir: Option<&str>, base_dir: &Path) -> String {
    match workdir {
        None => base_dir.to_string_lossy().to_string(),
        Some(wd) => {
            let path = PathBuf::from(wd);
            if path.is_relative() {
                normalize_path(&base_dir.join(path))
                    .to_string_lossy()
                    .to_string()
            } else {
                wd.to_string()
            }
        }
    }
}

/// Resolves working directories in execution-based variables relative to `base_dir`.
fn resolve_variable_working_dirs(variables: &mut VariableConfigMap, base_dir: &Path) {
    for (_, variable) in variables.iter_mut() {
        match variable {
            VariableConfig::Execution(exec_conf) => {
                resolve_exec_workdir(&mut exec_conf.execution, base_dir);
            }
            VariableConfig::Prompt(prompt_conf) => {
                if let PromptOptionsVariant::Select(select_opts) = &mut prompt_conf.prompt.options {
                    if let SelectOptionsConfig::Execution(exec_select_opts) =
                        &mut select_opts.options
                    {
                        resolve_exec_workdir(&mut exec_select_opts.execution, base_dir);
                    }
                }
            }
            _ => {}
        }
    }
}

/// Recursively resolves working directories for all executions in a command map relative to `base_dir`.
fn resolve_command_working_dirs(commands: &mut CommandConfigMap, base_dir: &Path) {
    for (_, command) in commands.iter_mut() {
        resolve_command_working_dirs(&mut command.commands, base_dir);
        resolve_variable_working_dirs(&mut command.variables, base_dir);

        if let Some(action) = &mut command.action {
            match action {
                ActionConfig::SingleStep(single) => {
                    resolve_exec_workdir(&mut single.action, base_dir);
                }
                ActionConfig::MultiStep(multi) => {
                    for exec in &mut multi.actions {
                        resolve_exec_workdir(exec, base_dir);
                    }
                }
                ActionConfig::Alias(_) => {}
            }
        }
    }
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
    pub options: Options,
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
pub struct Options {
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

impl Default for Options {
    fn default() -> Self {
        Options {
            print_commands: default_print_commands(),
            print_variables: default_print_variables(),
            auto_args: default_auto_args(),
        }
    }
}

fn default_print_commands() -> bool {
    match env::var("PLZ_PRINT_COMMANDS") {
        Ok(str) => is_truthy(str),
        Err(_) => false,
    }
}

fn default_print_variables() -> bool {
    match env::var("PLZ_PRINT_VARIABLES") {
        Ok(str) => is_truthy(str),
        Err(_) => false,
    }
}

fn default_auto_args() -> bool {
    match env::var("PLZ_AUTO_ARGS") {
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

    /// Encapsulates a [`ArgumentVariableConfig`].
    Argument(ArgumentVariableConfig),
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
            VariableConfig::Argument(argument_conf) => {
                argument_conf.clone().environment_variable_name
            }
        }
        .unwrap_or(key.to_string())
    }
}

/// Denotes a literal variable where the value is hard-coded.
///
/// Example:
/// ```yaml
/// name:
///     arg: name
///     value: Alice
/// ```
#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct LiteralVariableConfig {
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
///     arg: name
///     exec: cat name.txt
/// ```
#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct ExecutionVariableConfig {
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
///     arg: name
///     prompt:
///         message: What is your name?
/// ```
#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct PromptVariableConfig {
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

/// Denotes a variable whose value is sourced from command-line arguments.
///
/// Example:
/// ```yaml
/// name:
///     arg:
///         long: name
///         short: n
///         description: Your name
/// ```
#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct ArgumentVariableConfig {
    /// An optional argument configuration.
    #[serde(rename(deserialize = "argument"))]
    #[serde(alias = "arg")]
    pub argument: ArgumentConfigVariant,

    /// An optional environment variable name.
    /// If specified, the environment variable for this variable will have the specified name.
    ///
    /// This is **not** the name of the environment variable to source the value from.
    /// If you want to source a variables value from an environment variable,
    /// use an [`ExecutionVariableConfig`].
    #[serde(rename(deserialize = "environment_variable"))]
    #[serde(alias = "env")]
    pub environment_variable_name: Option<String>,
}

/// The kind of argument configuration.
#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
#[serde(untagged)]
pub enum ArgumentConfigVariant {
    Shorthand(String),
    Named(NamedArgumentConfig),
    Positional(PositionalArgumentConfig),
}

/// The configuration for a command-line argument.
#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct NamedArgumentConfig {
    /// An optional description for the variable.
    #[serde(alias = "desc")]
    pub description: Option<String>,

    /// The long version of the argument without the preceding `--`.
    pub long: String,

    /// The short version of the argument without the preceding `-`.
    pub short: Option<char>,
}

/// The configuration for a positional command-line argument.
#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct PositionalArgumentConfig {
    /// An optional description for the variable.
    #[serde(alias = "desc")]
    pub description: Option<String>,

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

    /// The [`DeferConfig`] that this command will perform once all of the actions have been executed.
    /// The commands provided here will always run in the provided order, regardless of any previous errors.
    pub defer: Option<DeferConfig>,
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

/// Encapsulates one or many [`ExecutionConfigVariant`].
#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
#[serde(untagged)]
pub enum DeferConfig {
    MultiStep(Vec<ExecutionConfigVariant>),
    SingleStep(ExecutionConfigVariant),
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
        let config = parse_config(&yaml.to_string(), Platform::Linux, None).unwrap();

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
        let config = parse_config(&yaml.to_string(), Platform::Linux, None).unwrap();

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
                arg: command-arg
                env: MY_VAR
        action: echo \"Hello, World!\"";
        let config = parse_config(&yaml.to_string(), Platform::Linux, None).unwrap();

        assert!(!config.variables.is_empty());

        let root_variable = config.variables.get("my-root-var").unwrap();
        assert_eq!(
            root_variable,
            &VariableConfig::Literal(LiteralVariableConfig {
                value: "My root value".to_string(),
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
                arg: command-arg-1
                env: MY_VAR_1
            my-command-var-2:
                exec:
                    bash: echo \"My command value\"
                arg:
                    description: Command level variable
                    long: command-arg-2
                    short: c
                env: MY_VAR_2
            my-command-var-3:
                exec:
                    bash: echo \"My command value\"
                arg:
                    description: Command level variable
                    position: 1
                env: MY_VAR_3
        action: echo \"Hello, World!\"";
        let config = parse_config(&yaml.to_string(), Platform::Linux, None).unwrap();

        assert!(!config.variables.is_empty());

        let root_variable = config.variables.get("my-root-var").unwrap();
        assert_eq!(
            root_variable,
            &VariableConfig::Execution(ExecutionVariableConfig {
                execution: bash_exec("echo \"My root value\"", Some("../".to_string())),
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
                argument: Some(ArgumentConfigVariant::Named(NamedArgumentConfig {
                    description: Some("Command level variable".to_string()),
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
                argument: Some(ArgumentConfigVariant::Positional(
                    PositionalArgumentConfig {
                        description: Some("Command level variable".to_string()),
                        position: 1,
                    }
                )),
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
        let config = parse_config(&yaml.to_string(), Platform::Linux, None).unwrap();

        assert!(!config.variables.is_empty());

        let name_variable = config.variables.get("name").unwrap();
        assert_eq!(
            name_variable,
            &VariableConfig::Prompt(PromptVariableConfig {
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
    fn argument_variable_parsed() {
        let yaml = "commands:
    demo:
        variables:
            name:
                argument:
                    description: Your name.
                    long: name
                    short: n
            age:
                arg: age
            food:
                arg:
                    description: Your favourite food.
                    position: 1
        action: echo \"Hello, World!\"";
        let config = parse_config(&yaml.to_string(), Platform::Linux, None).unwrap();

        let demo_command = config.commands.get("demo").unwrap();

        let name_variable = demo_command.variables.get("name").unwrap();
        assert_eq!(
            name_variable,
            &VariableConfig::Argument(ArgumentVariableConfig {
                argument: ArgumentConfigVariant::Named(NamedArgumentConfig {
                    description: Some("Your name.".to_string()),
                    long: "name".to_string(),
                    short: Some('n'),
                }),
                environment_variable_name: None,
            })
        );

        let age_variable = demo_command.variables.get("age").unwrap();
        assert_eq!(
            age_variable,
            &VariableConfig::Argument(ArgumentVariableConfig {
                argument: ArgumentConfigVariant::Shorthand("age".to_string()),
                environment_variable_name: None,
            })
        );

        let food_variable = demo_command.variables.get("food").unwrap();
        assert_eq!(
            food_variable,
            &VariableConfig::Argument(ArgumentVariableConfig {
                argument: ArgumentConfigVariant::Positional(PositionalArgumentConfig {
                    description: Some("Your favourite food.".to_string()),
                    position: 1
                }),
                environment_variable_name: None,
            })
        );
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
        let config = parse_config(&yaml.to_string(), Platform::Linux, None).unwrap();

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
        let config = parse_config(&yaml.to_string(), Platform::Linux, None).unwrap();

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
                defer: None,
            }
        );
    }

    #[test]
    fn alias_command_parses() {
        let yaml = "commands:
    deps:
        alias: docker compose -f docker-compose.deps.yml";
        let config = parse_config(&yaml.to_string(), Platform::Linux, None).unwrap();

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
                defer: None,
            }
        );
    }

    #[test]
    fn single_action_command_with_optional_fields_parses() {
        let yaml = "commands:
    demo:
        description: Says hello.
        action: ls";
        let config = parse_config(&yaml.to_string(), Platform::Linux, None).unwrap();

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
                defer: None,
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
        let config = parse_config(&yaml.to_string(), Platform::Linux, None).unwrap();

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
                defer: None,
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
                defer: None,
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
        let config = parse_config(&yaml.to_string(), Platform::Linux, None).unwrap();

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
                defer: None,
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
                defer: None,
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
        let config = parse_config(&yaml.to_string(), Platform::Linux, None).unwrap();

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
                defer: None,
            }
        );
    }

    #[test]
    fn command_with_deferred_action_parses() {
        let yaml = "commands:
    demo:
        action: cat example.txt
        defer: rm example.txt";
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
                        "cat example.txt".to_string()
                    )),
                })),
                defer: Some(DeferConfig::SingleStep(ExecutionConfigVariant::RawCommand(RawCommandConfigVariant::Shorthand(
                        "rm example.txt".to_string()
                    ))
                )),
            }
        );
    }

    #[test]
    fn command_with_deferred_actions_parses() {
        let yaml = "commands:
    demo:
        action: cat example.txt
        defer:
            - rm example.txt
            - echo Done!";
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
                    action: ExecutionConfigVariant::RawCommand(Shorthand(
                            "cat example.txt".to_string()
                        )),
                })),
                defer: Some(DeferConfig::MultiStep(vec![
                        ExecutionConfigVariant::RawCommand(Shorthand(
                            "rm example.txt".to_string()
                        )),
                        ExecutionConfigVariant::RawCommand(Shorthand(
                            "echo Done!".to_string()
                        ))
                    ]
                )),
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
        let config = parse_config(&yaml.to_string(), Platform::Linux, None).unwrap();

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
                defer: None,
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
                defer: None,
            }
        );
    }

    #[test]
    fn commands_with_name_parse() {
        let yaml = "commands:
    demo:
        name: demonstration
        action: cat example.txt";
        let config = parse_config(&yaml.to_string(), Platform::Linux, None).unwrap();

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
                defer: None,
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
        let config = parse_config(&yaml.to_string(), Platform::Linux, None).unwrap();

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
                defer: None,
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
        let yaml3_dir = yaml3_file
            .path()
            .parent()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();

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
        let yaml2_dir = yaml2_file
            .path()
            .parent()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();

        let yaml1 = format!(
            "imports:
    - alias: level-2
      source: {}
      platform: Linux
variables:
    first_name: Alice
commands:
    demo:
        action: echo \"Your first name is $first_name!\"",
            yaml2_file.path().to_str().unwrap()
        );

        let config = parse_config(&yaml1.to_string(), Platform::Linux, None).unwrap();

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
            &VariableConfig::ShorthandLiteral("Alice".to_string())
        );

        let second_level_command = config.commands.get("level-2").unwrap();
        assert_eq!(
            second_level_command.commands.get("demo").unwrap().action,
            Some(ActionConfig::SingleStep(SingleActionConfig {
                action: ExecutionConfigVariant::RawCommand(
                    RawCommandConfigVariant::RawCommandConfig(RawCommandConfig {
                        command: "echo \"Your last name is $last_name!\"".to_string(),
                        working_directory: Some(yaml2_dir),
                    })
                )
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
                action: ExecutionConfigVariant::RawCommand(
                    RawCommandConfigVariant::RawCommandConfig(RawCommandConfig {
                        command: "echo \"You are $age years old.\"".to_string(),
                        working_directory: Some(yaml3_dir),
                    })
                )
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
    first_name: Alice
commands:
    demo:
        action: echo \"Your first name is $first_name!\"",
            yaml2_file.path().to_str().unwrap()
        );

        let config = parse_config(&yaml1.to_string(), Platform::Linux, None).unwrap();

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
            &VariableConfig::ShorthandLiteral("Alice".to_string())
        );

        let second_level_command = config.commands.get("other");
        assert_eq!(second_level_command, None);
    }

    fn create_temp_file(content: &str) -> NamedTempFile {
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(content.as_bytes()).unwrap();
        return temp_file;
    }

    // --- Import path and working directory resolution tests ---

    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn create_temp_dir() -> TempDir {
        TempDir::new().unwrap()
    }

    fn write_file(path: &PathBuf, content: &str) {
        fs::write(path, content).unwrap();
    }

    #[test]
    fn relative_import_source_resolves_from_config_file_location() {
        let dir = create_temp_dir();

        write_file(
            &dir.path().join("child.yaml"),
            "commands:
  demo:
    action: echo hello",
        );

        let parent_path = dir.path().join("parent.yaml");
        write_file(
            &parent_path,
            "imports:
  - alias: child
    source: ./child.yaml
commands: {}",
        );

        let config = parse_config_from(&parent_path, Platform::Linux).unwrap();

        assert!(config.commands.contains_key("child"));
    }

    #[test]
    fn imported_command_shorthand_action_gets_working_dir_from_config_location() {
        let dir = create_temp_dir();
        let dir_str = dir.path().to_str().unwrap().to_string();

        write_file(
            &dir.path().join("child.yaml"),
            "commands:
  demo:
    action: ./run.sh",
        );

        let parent_path = dir.path().join("parent.yaml");
        write_file(
            &parent_path,
            "imports:
  - alias: child
    source: ./child.yaml
commands: {}",
        );

        let config = parse_config_from(&parent_path, Platform::Linux).unwrap();

        let demo = config.commands["child"].commands["demo"].clone();
        assert_eq!(
            demo.action,
            Some(ActionConfig::SingleStep(SingleActionConfig {
                action: ExecutionConfigVariant::RawCommand(
                    RawCommandConfigVariant::RawCommandConfig(RawCommandConfig {
                        command: "./run.sh".to_string(),
                        working_directory: Some(dir_str),
                    })
                )
            }))
        );
    }

    #[test]
    fn imported_command_bash_action_gets_working_dir_from_config_location() {
        let dir = create_temp_dir();
        let dir_str = dir.path().to_str().unwrap().to_string();

        write_file(
            &dir.path().join("child.yaml"),
            "commands:
  demo:
    action:
      bash: echo hello",
        );

        let parent_path = dir.path().join("parent.yaml");
        write_file(
            &parent_path,
            "imports:
  - alias: child
    source: ./child.yaml
commands: {}",
        );

        let config = parse_config_from(&parent_path, Platform::Linux).unwrap();

        let demo = config.commands["child"].commands["demo"].clone();
        assert_eq!(
            demo.action,
            Some(ActionConfig::SingleStep(SingleActionConfig {
                action: ExecutionConfigVariant::ShellCommand(ShellCommandConfigVariant::Bash(
                    BashCommandConfig {
                        command: "echo hello".to_string(),
                        working_directory: Some(dir_str),
                    }
                ))
            }))
        );
    }

    #[test]
    fn imported_command_relative_workdir_resolves_against_config_location() {
        let dir = create_temp_dir();
        let expected_workdir = dir.path().join("scripts").to_str().unwrap().to_string();

        write_file(
            &dir.path().join("child.yaml"),
            "commands:
  demo:
    action:
      command: ./run.sh
      workdir: ./scripts",
        );

        let parent_path = dir.path().join("parent.yaml");
        write_file(
            &parent_path,
            "imports:
  - alias: child
    source: ./child.yaml
commands: {}",
        );

        let config = parse_config_from(&parent_path, Platform::Linux).unwrap();

        let demo = config.commands["child"].commands["demo"].clone();
        assert_eq!(
            demo.action,
            Some(ActionConfig::SingleStep(SingleActionConfig {
                action: ExecutionConfigVariant::RawCommand(
                    RawCommandConfigVariant::RawCommandConfig(RawCommandConfig {
                        command: "./run.sh".to_string(),
                        working_directory: Some(expected_workdir),
                    })
                )
            }))
        );
    }

    #[test]
    fn imported_command_absolute_workdir_is_unchanged() {
        let dir = create_temp_dir();
        #[cfg(windows)]
        let absolute_workdir = "C:\\absolute\\path";
        #[cfg(not(windows))]
        let absolute_workdir = "/absolute/path";

        write_file(
            &dir.path().join("child.yaml"),
            &format!(
                "commands:
  demo:
    action:
      command: ./run.sh
      workdir: {}",
                absolute_workdir
            ),
        );

        let parent_path = dir.path().join("parent.yaml");
        write_file(
            &parent_path,
            "imports:
  - alias: child
    source: ./child.yaml
commands: {}",
        );

        let config = parse_config_from(&parent_path, Platform::Linux).unwrap();

        let demo = config.commands["child"].commands["demo"].clone();
        assert_eq!(
            demo.action,
            Some(ActionConfig::SingleStep(SingleActionConfig {
                action: ExecutionConfigVariant::RawCommand(
                    RawCommandConfigVariant::RawCommandConfig(RawCommandConfig {
                        command: "./run.sh".to_string(),
                        working_directory: Some(absolute_workdir.to_string()),
                    })
                )
            }))
        );
    }

    #[test]
    fn nested_imported_command_gets_working_dir_from_its_config_location() {
        let dir = create_temp_dir();
        let sub_dir = dir.path().join("sub");
        fs::create_dir(&sub_dir).unwrap();
        let sub_dir_str = sub_dir.to_str().unwrap().to_string();

        write_file(
            &sub_dir.join("grandchild.yaml"),
            "commands:
  demo:
    action: ./run.sh",
        );

        write_file(
            &sub_dir.join("child.yaml"),
            "imports:
  - alias: grandchild
    source: ./grandchild.yaml
commands: {}",
        );

        let parent_path = dir.path().join("parent.yaml");
        write_file(
            &parent_path,
            "imports:
  - alias: child
    source: ./sub/child.yaml
commands: {}",
        );

        let config = parse_config_from(&parent_path, Platform::Linux).unwrap();

        let demo = config.commands["child"].commands["grandchild"].commands["demo"].clone();
        assert_eq!(
            demo.action,
            Some(ActionConfig::SingleStep(SingleActionConfig {
                action: ExecutionConfigVariant::RawCommand(
                    RawCommandConfigVariant::RawCommandConfig(RawCommandConfig {
                        command: "./run.sh".to_string(),
                        working_directory: Some(sub_dir_str),
                    })
                )
            }))
        );
    }
}
