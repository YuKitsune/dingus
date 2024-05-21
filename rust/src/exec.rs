use std::error::Error;
use std::{fmt, io};
use std::fmt::{Formatter};
use std::process::{Command};
use crate::config::{ExecutionConfig, RawCommandConfig, ShellCommandConfig};
use crate::exec::ExitStatus::Unknown;
use crate::variables::Variables;

pub type ExecutionResult = Result<(), ShellError>;
pub type ExecutionOutputResult = Result<Output, ShellError>;

#[derive(PartialEq, Debug, Clone)]
pub enum ExitStatus {
    Success,
    Fail(i32),
    Unknown
}

impl ExitStatus {
    pub fn from_std_exitstatus(exit_status: &std::process::ExitStatus) -> ExitStatus {
        return if exit_status.success() {
            ExitStatus::Success
        } else if let Some(code) = exit_status.code() {
            ExitStatus::Fail(code)
        } else {
            Unknown
        };
    }
}

impl fmt::Display for ExitStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            ExitStatus::Success => write!(f, "process exited with code 0"),
            ExitStatus::Fail(code) => write!(f, "process exited with code {}", code),
            Unknown => write!(f, "process exited with unknown exit code")
        }
    }
}

#[derive(Clone)]
pub struct Output {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

impl Output {
    pub fn from_std_output(output: &std::process::Output) -> Output {
        Output {
            status: ExitStatus::from_std_exitstatus(&output.status),
            stdout: output.stdout.clone(),
            stderr: output.stderr.clone(),
        }
    }
}

pub trait CommandExecutor {
    fn execute(&self, execution_config: &ExecutionConfig, variables: &Variables) -> ExecutionResult;
    fn get_output(&self, execution_config: &ExecutionConfig, variables: &Variables) -> ExecutionOutputResult;
}

pub fn create_command_executor() -> Box<dyn CommandExecutor> {
    return Box::new(CommandExecutorImpl { })
}

struct CommandExecutorImpl { }

impl CommandExecutor for CommandExecutorImpl {

    fn execute(&self, execution_config: &ExecutionConfig, variables: &Variables) -> ExecutionResult {
        let mut command = get_command_for(execution_config);
        command.envs(variables)
            .spawn()
            .map_err(|io_err| ShellError::IO(io_err))?;

        return Ok(());
    }

    fn get_output(&self, execution_config: &ExecutionConfig, variables: &Variables) -> ExecutionOutputResult {
        let mut command = get_command_for(execution_config);
        let output = command.envs(variables)
            .output()
            .map_err(|io_err| ShellError::IO(io_err))?;

        return Ok(Output::from_std_output(&output));
    }
}

fn get_command_for(execution_config: &ExecutionConfig) -> Command {
    match execution_config {
        ExecutionConfig::ShellCommand(shell_command_config) => {
            match shell_command_config {
                ShellCommandConfig::Bash(bash_command_config) => {
                    let mut binding = Command::new("bash");
                    binding.arg("-c")
                        .arg(bash_command_config.clone().command);

                    if let Some(wd) = bash_command_config.clone().working_directory {
                        binding.current_dir(wd);
                    }

                    binding
                }
            }
        }

        ExecutionConfig::RawCommand(raw_command_config) => {
            let (command, working_directory) = match raw_command_config {
                RawCommandConfig::Shorthand(command) => (command.clone(), None),
                RawCommandConfig::Extended(extended_config) => (extended_config.clone().command, extended_config.clone().working_directory),
            };

            const DELIMITER: &str = " ";
            let mut cmd = match command.split_once(DELIMITER) {
                Some((program, args)) => {
                    let argv = args.split(DELIMITER);
                    let mut binding = Command::new(program);
                    binding.args(argv);
                    binding
                },
                None => Command::new(command),
            };

            if let Some(wd) = working_directory {
                cmd.current_dir(wd);
            }

            return cmd
        }
    }
}

#[derive(Debug)]
pub enum ShellError {
    IO(io::Error)
}

impl Error for ShellError {}

impl fmt::Display for ShellError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            ShellError::IO(io_error) => io_error.fmt(f),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use crate::config::{BashShellCommandConfig, ExtendedRawCommandConfig};
    use crate::config::RawCommandConfig::{Extended, Shorthand};
    use super::*;

    // Todo: Tests for execute (Inherits Stdio, interactive, variables evaluated, etc)
    // Todo: Macro for various shell types

    #[test]
    fn bash_command_get_output_evaluates_variables() {

        // Arrange
        let variable_name = "name";
        let variable_value = "Dingus";
        let mut variables = HashMap::new();
        variables.insert(variable_name.to_string(), variable_value.to_string());

        let bash_exec_config = ExecutionConfig::ShellCommand(ShellCommandConfig::Bash(BashShellCommandConfig {
            working_directory: None,
            command: format!("echo \"Hello, ${variable_name}!\"")
        }));
        let shell_executor = create_command_executor();

        // Act
        let result = shell_executor.get_output(&bash_exec_config, &variables);
        assert!(!result.is_err());

        // Assert
        let output = result.unwrap();
        assert_eq!(output.status, ExitStatus::Success);
        assert!(output.stderr.is_empty());

        let output_value = String::from_utf8(output.stdout).unwrap();
        assert_eq!(output_value, format!("Hello, {variable_value}!\n"));
    }

    #[test]
    fn bash_command_get_output_returns_stdout() {

        // Arrange
        let bash_exec_config = ExecutionConfig::ShellCommand(ShellCommandConfig::Bash(BashShellCommandConfig {
            working_directory: None,
            command: "echo \"Hello, World!\"".to_string()
        }));
        let shell_executor = create_command_executor();

        // Act
        let result = shell_executor.get_output(&bash_exec_config, &HashMap::new());
        assert!(!result.is_err());

        // Assert
        let output = result.unwrap();
        assert_eq!(output.status, ExitStatus::Success);
        assert!(output.stderr.is_empty());

        let output_value = String::from_utf8(output.stdout).unwrap();
        assert_eq!(output_value, "Hello, World!\n");
    }

    #[test]
    fn bash_command_get_output_returns_stderr() {

        // Arrange
        let bash_exec_config = ExecutionConfig::ShellCommand(ShellCommandConfig::Bash(BashShellCommandConfig {
            working_directory: None,
            command: ">&2 echo \"Error message\"".to_string()
        }));
        let shell_executor = create_command_executor();

        // Act
        let result = shell_executor.get_output(&bash_exec_config, &HashMap::new());
        assert!(!result.is_err());

        // Assert
        let output = result.unwrap();
        assert_eq!(output.status, ExitStatus::Success);
        assert!(output.stdout.is_empty());

        let output_value = String::from_utf8(output.stderr).unwrap();
        assert_eq!(output_value, "Error message\n");
    }

    #[test]
    fn bash_command_get_output_returns_exit_code() {

        // Arrange
        let bash_exec_config = ExecutionConfig::ShellCommand(ShellCommandConfig::Bash(BashShellCommandConfig {
            working_directory: None,
            command: "exit 42".to_string()
        }));
        let shell_executor = create_command_executor();

        // Act
        let result = shell_executor.get_output(&bash_exec_config, &HashMap::new());
        assert!(!result.is_err());

        // Assert
        let output = result.unwrap();
        assert_eq!(output.status, ExitStatus::Fail(42));
        assert!(output.stdout.is_empty());
        assert!(output.stderr.is_empty());
    }

    #[test]
    fn bash_command_honours_workdir() {

        // Arrange
        let bash_exec_config = ExecutionConfig::ShellCommand(ShellCommandConfig::Bash(BashShellCommandConfig {
            working_directory: Some("./src".to_string()),
            command: "pwd".to_string()
        }));
        let shell_executor = create_command_executor();

        // Act
        let result = shell_executor.get_output(&bash_exec_config, &HashMap::new());
        assert!(!result.is_err());

        // Assert
        let output = result.unwrap();
        assert_eq!(output.status, ExitStatus::Success);
        assert!(output.stderr.is_empty());

        let output_value = String::from_utf8(output.stdout).unwrap();
        assert!(output_value.ends_with("/src\n"));
    }

    #[test]
    fn raw_command_get_output_has_variables() {

        // Arrange
        let variable_name = "CARGO_ALIAS_V";
        let variable_value = "version";
        let mut variables = HashMap::new();
        variables.insert(variable_name.to_string(), variable_value.to_string());

        let exec_config = ExecutionConfig::RawCommand(Shorthand("cargo v".to_string()));
        let shell_executor = create_command_executor();

        // Act
        let result = shell_executor.get_output(&exec_config, &variables);
        assert!(!result.is_err());

        // Assert
        let output = result.unwrap();
        assert_eq!(output.status, ExitStatus::Success);
        assert!(output.stderr.is_empty());

        let output_value = String::from_utf8(output.stdout).unwrap();
        assert_eq!(output_value, "cargo 1.75.0\n");
    }

    #[test]
    fn raw_command_get_output_returns_stdout() {

        // Arrange
        let exec_config = ExecutionConfig::RawCommand(Shorthand("cat test.txt".to_string()));
        let shell_executor = create_command_executor();

        // Act
        let result = shell_executor.get_output(&exec_config, &HashMap::new());
        assert!(!result.is_err());

        // Assert
        let output = result.unwrap();
        assert_eq!(output.status, ExitStatus::Success);
        assert!(output.stderr.is_empty());

        let output_value = String::from_utf8(output.stdout).unwrap();
        assert_eq!(output_value, "Hello, World!");
    }

    #[test]
    fn raw_command_get_output_returns_stderr() {

        // Arrange
        let exec_config = ExecutionConfig::RawCommand(Shorthand("cat does_not_exist.txt".to_string()));
        let shell_executor = create_command_executor();

        // Act
        let result = shell_executor.get_output(&exec_config, &HashMap::new());
        assert!(!result.is_err());

        // Assert
        let output = result.unwrap();
        assert_eq!(output.status, ExitStatus::Fail(1));
        assert!(output.stdout.is_empty());

        let output_value = String::from_utf8(output.stderr).unwrap();
        assert!(output_value.contains("No such file or directory"));
    }

    #[test]
    fn raw_command_honours_workdir() {

        // Arrange
        let exec_config = ExecutionConfig::RawCommand(Extended(ExtendedRawCommandConfig {
            working_directory: Some("./src".to_string()),
            command: "pwd".to_string(),
        }));
        let shell_executor = create_command_executor();

        // Act
        let result = shell_executor.get_output(&exec_config, &HashMap::new());
        assert!(!result.is_err());

        // Assert
        let output = result.unwrap();
        assert_eq!(output.status, ExitStatus::Success);
        assert!(output.stderr.is_empty());

        let output_value = String::from_utf8(output.stdout).unwrap();
        assert!(output_value.ends_with("/src\n"));
    }

    #[test]
    fn raw_command_does_not_use_shell() {

        // Arrange
        let exec_config = ExecutionConfig::RawCommand(Extended(ExtendedRawCommandConfig {
            working_directory: None,
            command: "shopt -s expand_aliases".to_string(),
        }));
        let shell_executor = create_command_executor();

        // Act
        let result = shell_executor.get_output(&exec_config, &HashMap::new());

        // Assert
        assert!(result.is_err());
    }
}
