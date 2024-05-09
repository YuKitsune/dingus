use std::error::Error;
use std::{fmt, io};
use std::fmt::{Formatter};
use std::process::{Command};
use crate::config::Shell;
use crate::shell::ExitStatus::Unknown;
use crate::variables::Variables;

pub type ShellCommand = String;
pub type ShellExecutionResult = Result<Output, ShellError>;

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

pub trait ShellExecutorFactory {
    fn create(&self, shell: &Shell) -> Box<dyn ShellExecutor>;
    fn create_default(&self) -> Box<dyn ShellExecutor>;
}

struct ShellExecutorFactoryImpl {
    default_shell: Shell
}

impl ShellExecutorFactory for ShellExecutorFactoryImpl {
    fn create(&self, shell: &Shell) -> Box<dyn ShellExecutor> {
        return match shell {
            Shell::Bash => Box::new(BashExecutor{})
        }
    }

    fn create_default(&self) -> Box<dyn ShellExecutor> {
        self.create(&self.default_shell)
    }
}

pub fn create_shell_executor_factory(default_shell: &Shell) -> impl ShellExecutorFactory {
    return ShellExecutorFactoryImpl{
        default_shell: default_shell.clone()
    };
}

pub trait ShellExecutor {
    fn execute(&self, command: &ShellCommand, variables: &Variables) -> ShellExecutionResult;
}

struct BashExecutor { }

impl ShellExecutor for BashExecutor {

    fn execute(&self, command: &ShellCommand, variables: &Variables) -> ShellExecutionResult {

        let mut binding = Command::new("bash");

        let child = binding
            .arg("-c")
            .arg(command)
            .envs(variables)
            .spawn()
            .map_err(|io_err| ShellError::IO(io_err))?;

        let output = child.wait_with_output()
            .map_err(|io_err| ShellError::IO(io_err))?;

        return Ok(Output::from_std_output(&output))
    }
}

#[derive(Debug)]
pub enum ShellError {
    IO(io::Error)
}

impl Error for ShellError {}

impl fmt::Display for ShellError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ShellError::IO(io_error) => io_error.fmt(f),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use super::*;

    // Todo: Macro for various shell types
    // Todo: execute_is_interactive

    #[test]
    fn bash_executor_execute_has_variables() {

        // Arrange
        let variable_name = "name";
        let variable_value = "Dingus";
        let mut variables = HashMap::new();
        variables.insert(variable_name.to_string(), variable_value.to_string());

        let shell_command: ShellCommand = ShellCommand::from(format!("echo \"Hello, ${variable_name}!\""));
        let shell_executor = BashExecutor{};

        // Act
        let result = shell_executor.execute(&shell_command, &variables);
        assert!(!result.is_err());

        // Assert
        let output = result.unwrap();
        assert_eq!(output.status, ExitStatus::Success);
        assert!(output.stderr.is_empty());

        let output_value = String::from_utf8(output.stdout).unwrap();
        assert_eq!(output_value, format!("Hello, {variable_value}!\n"));
    }

    #[test]
    fn bash_executor_execute_returns_stdout() {

        // Arrange
        let shell_command: ShellCommand = ShellCommand::from("echo \"Hello, World!\"");
        let shell_executor = BashExecutor{};

        // Act
        let result = shell_executor.execute(&shell_command, &HashMap::new());
        assert!(!result.is_err());

        // Assert
        let output = result.unwrap();
        assert_eq!(output.status, ExitStatus::Success);
        assert!(output.stderr.is_empty());

        let output_value = String::from_utf8(output.stdout).unwrap();
        assert_eq!(output_value, "Hello, World!\n");
    }

    #[test]
    fn bash_executor_execute_returns_stderr() {

        // Arrange
        let shell_command: ShellCommand = ShellCommand::from(">&2 echo \"Error message\"");
        let shell_executor = BashExecutor{};

        // Act
        let result = shell_executor.execute(&shell_command, &HashMap::new());
        assert!(!result.is_err());

        // Assert
        let output = result.unwrap();
        assert_eq!(output.status, ExitStatus::Success);
        assert!(output.stdout.is_empty());

        let output_value = String::from_utf8(output.stderr).unwrap();
        assert_eq!(output_value, "Error message\n");
    }

    #[test]
    fn bash_executor_execute_returns_exit_code() {

        // Arrange
        let shell_command: ShellCommand = ShellCommand::from("exit 42");
        let shell_executor = BashExecutor{};

        // Act
        let result = shell_executor.execute(&shell_command, &HashMap::new());
        assert!(!result.is_err());

        // Assert
        let output = result.unwrap();
        assert_eq!(output.status, ExitStatus::Fail(42));
        assert!(output.stdout.is_empty());
        assert!(output.stderr.is_empty());
    }
}
