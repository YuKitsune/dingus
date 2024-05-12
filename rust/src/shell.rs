use std::error::Error;
use std::{fmt, io};
use std::fmt::{Formatter};
use std::process::{Command};
use crate::config::{ExecutionConfig, ShellCommandConfig};
use crate::shell::ExitStatus::Unknown;
use crate::variables::Variables;

pub type ShellCommand = String;
pub type ShellExecutionResult = Result<(), ShellError>;
pub type ShellExecutionOutputResult = Result<Output, ShellError>;

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

pub trait ShellExecutor {
    fn execute(&self, execution_config: &ExecutionConfig, variables: &Variables) -> ShellExecutionResult;
    fn get_output(&self, execution_config: &ExecutionConfig, variables: &Variables) -> ShellExecutionOutputResult;
}

pub fn create_shell_executor() -> Box<dyn ShellExecutor> {
    return Box::new(ShellExecutorImpl { })
}

struct ShellExecutorImpl { }

impl ShellExecutor for ShellExecutorImpl {

    fn execute(&self, execution_config: &ExecutionConfig, variables: &Variables) -> ShellExecutionResult {
        let mut command = get_command_for(execution_config);
        command.envs(variables)
            .spawn()
            .map_err(|io_err| ShellError::IO(io_err))?;

        return Ok(());
    }

    fn get_output(&self, execution_config: &ExecutionConfig, variables: &Variables) -> ShellExecutionOutputResult {
        let mut command = get_command_for(execution_config);
        let output = command.envs(variables)
            .output()
            .map_err(|io_err| ShellError::IO(io_err))?;

        return Ok(Output::from_std_output(&output));
    }
}

fn get_command_for(execution_config: &ExecutionConfig) -> Command {
    return match execution_config {
        ExecutionConfig::ShellCommand(shell_command_config) => {
            match shell_command_config {
                ShellCommandConfig::Bash(bash_command_config) => {
                    let mut binding = Command::new("bash");
                    binding.arg("-c")
                        .arg(bash_command_config.clone().command);
                    binding
                }
            }
        }

        ExecutionConfig::RawCommand(raw_command) => {
            const DELIMITER: &str = " ";
            match raw_command.split_once(DELIMITER) {
                Some((program, args)) => {
                    let argv = args.split(DELIMITER);
                    let mut binding = Command::new(program);
                    binding.args(argv);
                    binding
                },
                None => {
                    Command::new(raw_command)
                },
            }
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
    use crate::config::BashShellCommandConfig;
    use super::*;

    // Todo: Tests for execute (Inherits Stdio, interactive, variables evaluated, etc)
    // Todo: Macro for various shell types

    #[test]
    fn bash_command_get_output_has_variables() {

        // Arrange
        let variable_name = "name";
        let variable_value = "Dingus";
        let mut variables = HashMap::new();
        variables.insert(variable_name.to_string(), variable_value.to_string());

        let bash_exec_config = ExecutionConfig::ShellCommand(ShellCommandConfig::Bash(BashShellCommandConfig {
            command: ShellCommand::from(format!("echo \"Hello, ${variable_name}!\""))
        }));
        let shell_executor = create_shell_executor();

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
            command: ShellCommand::from("echo \"Hello, World!\"")
        }));
        let shell_executor = create_shell_executor();

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
            command: ShellCommand::from(">&2 echo \"Error message\"")
        }));
        let shell_executor = create_shell_executor();

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
            command: ShellCommand::from("exit 42")
        }));
        let shell_executor = create_shell_executor();

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
    fn raw_command_get_output_has_variables() {

        // Arrange
        let variable_name = "filename";
        let variable_value = "test";
        let mut variables = HashMap::new();
        variables.insert(variable_name.to_string(), variable_value.to_string());

        let exec_config = ExecutionConfig::RawCommand(format!("cat ${variable_name}.txt"));
        let shell_executor = create_shell_executor();

        // Act
        let result = shell_executor.get_output(&exec_config, &variables);
        assert!(!result.is_err());

        // Assert
        let output = result.unwrap();
        assert_eq!(output.status, ExitStatus::Success);
        assert!(output.stderr.is_empty());

        let output_value = String::from_utf8(output.stdout).unwrap();
        assert_eq!(output_value, format!("Hello, World!"));
    }

    #[test]
    fn raw_command_get_output_returns_stdout() {

        // Arrange
        let exec_config = ExecutionConfig::RawCommand("cat test.txt".to_string());
        let shell_executor = create_shell_executor();

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
        let exec_config = ExecutionConfig::RawCommand(format!("cat does_not_exist.txt"));
        let shell_executor = create_shell_executor();

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
}
