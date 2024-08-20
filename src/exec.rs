use colored::Colorize;
use mockall::automock;
use std::fmt::Formatter;
use std::process::Command;
use std::{fmt, io};
use thiserror::Error;

use crate::config::{
    DingusOptions, ExecutionConfigVariant, RawCommandConfigVariant, ShellCommandConfigVariant,
};
use crate::exec::ExitStatus::Unknown;
use crate::variables;
use crate::variables::VariableMap;

pub type ExecutionResult = Result<ExitStatus, ExecutionError>;
pub type ExecutionOutputResult = Result<Output, ExecutionError>;

#[derive(PartialEq, Debug, Clone)]
pub enum ExitStatus {
    Success,
    Fail(i32),
    Unknown,
}

impl ExitStatus {
    fn from_std_exitstatus(exit_status: &std::process::ExitStatus) -> ExitStatus {
        if exit_status.success() {
            ExitStatus::Success
        } else if let Some(code) = exit_status.code() {
            ExitStatus::Fail(code)
        } else {
            Unknown
        }
    }
}

impl fmt::Display for ExitStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            ExitStatus::Success => write!(f, "process exited with code 0"),
            ExitStatus::Fail(code) => write!(f, "process exited with code {}", code),
            Unknown => write!(f, "process exited with unknown exit code"),
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
    fn from_std_output(output: &std::process::Output) -> Output {
        Output {
            status: ExitStatus::from_std_exitstatus(&output.status),
            stdout: output.stdout.clone(),
            stderr: output.stderr.clone(),
        }
    }
}

// TODO: Consider refactoring these to take stdio as args so we can test with stdin.

/// Capable of executing an [`ExecutionConfigVariant`].
#[automock]
pub trait CommandExecutor {
    /// Executes the provided [`ExecutionConfigVariant`] with the provided [`VariableMap`]
    /// inheriting stdin, stdout, and stderr from the current process.
    fn execute(
        &self,
        execution_config: &ExecutionConfigVariant,
        variables: &VariableMap,
    ) -> ExecutionResult;

    /// Executes the provided [`ExecutionConfigVariant`] with the provided [`VariableMap`]
    /// and returns the output from stdout and stderr.
    fn get_output(
        &self,
        execution_config: &ExecutionConfigVariant,
        variables: &VariableMap,
    ) -> ExecutionOutputResult;
}

pub fn create_command_executor(options: &DingusOptions) -> Box<dyn CommandExecutor> {
    Box::new(CommandExecutorImpl {
        options: options.clone(),
    })
}

struct CommandExecutorImpl {
    options: DingusOptions,
}

impl CommandExecutor for CommandExecutorImpl {
    fn execute(
        &self,
        execution_config: &ExecutionConfigVariant,
        variables: &VariableMap,
    ) -> ExecutionResult {
        let mut command = get_command_for(execution_config, variables);

        self.log(&command);

        let exit_status = command
            .spawn()
            .map_err(|io_err| ExecutionError::IO(io_err))?
            .wait()
            .map_err(|io_err| ExecutionError::IO(io_err))?;

        Ok(ExitStatus::from_std_exitstatus(&exit_status))
    }

    fn get_output(
        &self,
        execution_config: &ExecutionConfigVariant,
        variables: &VariableMap,
    ) -> ExecutionOutputResult {
        let mut command = get_command_for(execution_config, variables);

        self.log(&command);

        let output = command
            .output()
            .map_err(|io_err| ExecutionError::IO(io_err))?;

        Ok(Output::from_std_output(&output))
    }
}

impl CommandExecutorImpl {
    fn log(&self, command: &Command) {
        if self.options.print_commands {
            let command_text = get_command_text(&command);
            println!("Executing: {}", command_text.green())
        }
    }
}

fn get_command_for(execution_config: &ExecutionConfigVariant, variables: &VariableMap) -> Command {
    match execution_config {
        ExecutionConfigVariant::ShellCommand(shell_command_config) => match shell_command_config {
            ShellCommandConfigVariant::Bash(bash_command_config) => {
                let mut binding = Command::new("bash");
                binding
                    .arg("-c")
                    .envs(variables)
                    .arg(bash_command_config.clone().command);

                if let Some(wd) = bash_command_config.clone().working_directory {
                    binding.current_dir(wd);
                }

                binding
            }
        },

        ExecutionConfigVariant::RawCommand(raw_command_config) => {
            let (command_template, working_directory) = match raw_command_config {
                RawCommandConfigVariant::Shorthand(command) => (command.clone(), None),
                RawCommandConfigVariant::RawCommandConfig(raw_command_config) => (
                    raw_command_config.clone().command,
                    raw_command_config.clone().working_directory,
                ),
            };

            // Substitute any variables in the command invocation
            let command = variables::substitute_variables(&command_template, variables);

            const DELIMITER: &str = " ";
            let mut cmd = match command.split_once(DELIMITER) {
                Some((program, args)) => {
                    let argv = args.split(DELIMITER);
                    let mut binding = Command::new(program);
                    binding.args(argv).envs(variables);
                    binding
                }
                None => Command::new(command),
            };

            if let Some(wd) = working_directory {
                cmd.current_dir(wd);
            }

            return cmd;
        }
    }
}

fn get_command_text(command: &Command) -> String {
    let program_string = command.get_program().to_str().unwrap();
    let args_string = command
        .get_args()
        .map(|str| str.to_str().unwrap())
        .collect::<Vec<&str>>()
        .join(" ");
    format!("{} {}", program_string, args_string)
}

/// The error type for any errors that have occurred during the execution of a command.
/// Note that non-zero exit codes are not considered to be errors.
#[derive(Error, Debug)]
pub enum ExecutionError {
    #[error(transparent)]
    IO(io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{BashCommandConfig, RawCommandConfig};
    use std::collections::HashMap;
    use std::fs;
    use std::io::Write;
    use std::path::Path;
    use tempfile::{NamedTempFile, TempDir};

    // TODO: Testing with stdin?

    #[test]
    #[cfg(not(windows))]
    fn bash_command_execute_executes_command() {
        // Arrange
        let temp_file = create_empty_temp_file();
        let temp_file_path = get_path(&temp_file.path());

        let bash_exec_config = ExecutionConfigVariant::ShellCommand(
            ShellCommandConfigVariant::Bash(BashCommandConfig {
                working_directory: None,
                command: format!("echo \"Hello, World!\" > {temp_file_path}"),
            }),
        );
        let command_executor = create_command_executor(&DingusOptions::default());

        // Act
        let result = command_executor.execute(&bash_exec_config, &Default::default());
        assert!(!result.is_err());

        // Assert
        let file_content = fs::read_to_string(temp_file_path).unwrap();
        assert_eq!(file_content, format!("Hello, World!\n"));
    }

    #[test]
    #[cfg(not(windows))]
    fn bash_command_execute_evaluates_variables() {
        // Arrange
        let variable_name = "name";
        let variable_value = "Dingus";
        let mut variables = HashMap::new();
        variables.insert(variable_name.to_string(), variable_value.to_string());

        let temp_file = create_empty_temp_file();
        let temp_file_path = get_path(&temp_file.path());

        let bash_exec_config = ExecutionConfigVariant::ShellCommand(
            ShellCommandConfigVariant::Bash(BashCommandConfig {
                working_directory: None,
                command: format!("echo \"Hello, ${variable_name}!\" > {temp_file_path}"),
            }),
        );
        let command_executor = create_command_executor(&DingusOptions::default());

        // Act
        let result = command_executor.execute(&bash_exec_config, &variables);
        assert!(!result.is_err());

        // Assert
        let file_content = fs::read_to_string(temp_file_path).unwrap();
        assert_eq!(file_content, format!("Hello, {variable_value}!\n"));
    }

    #[test]
    #[cfg(not(windows))]
    fn bash_command_execute_returns_exit_code() {
        // Arrange
        let bash_exec_config = ExecutionConfigVariant::ShellCommand(
            ShellCommandConfigVariant::Bash(BashCommandConfig {
                working_directory: None,
                command: "exit 42".to_string(),
            }),
        );
        let command_executor = create_command_executor(&DingusOptions::default());

        // Act
        let result = command_executor.execute(&bash_exec_config, &Default::default());
        assert!(!result.is_err());

        // Assert
        let exit_status = result.unwrap();
        assert!(matches!(exit_status, ExitStatus::Fail(42)));
    }

    #[test]
    #[cfg(not(windows))]
    fn bash_command_get_output_evaluates_variables() {
        // Arrange
        let variable_name = "name";
        let variable_value = "Dingus";
        let mut variables = HashMap::new();
        variables.insert(variable_name.to_string(), variable_value.to_string());

        let bash_exec_config = ExecutionConfigVariant::ShellCommand(
            ShellCommandConfigVariant::Bash(BashCommandConfig {
                working_directory: None,
                command: format!("echo \"Hello, ${variable_name}!\""),
            }),
        );
        let command_executor = create_command_executor(&DingusOptions::default());

        // Act
        let result = command_executor.get_output(&bash_exec_config, &variables);
        assert!(!result.is_err());

        // Assert
        let output = result.unwrap();
        assert_eq!(output.status, ExitStatus::Success);
        assert!(output.stderr.is_empty());

        let output_value = String::from_utf8(output.stdout).unwrap();
        assert_eq!(output_value, format!("Hello, {variable_value}!\n"));
    }

    #[test]
    #[cfg(not(windows))]
    fn bash_command_get_output_returns_stdout() {
        // Arrange
        let bash_exec_config = ExecutionConfigVariant::ShellCommand(
            ShellCommandConfigVariant::Bash(BashCommandConfig {
                working_directory: None,
                command: "echo \"Hello, World!\"".to_string(),
            }),
        );
        let command_executor = create_command_executor(&DingusOptions::default());

        // Act
        let result = command_executor.get_output(&bash_exec_config, &HashMap::new());
        assert!(!result.is_err());

        // Assert
        let output = result.unwrap();
        assert_eq!(output.status, ExitStatus::Success);
        assert!(output.stderr.is_empty());

        let output_value = String::from_utf8(output.stdout).unwrap();
        assert_eq!(output_value, "Hello, World!\n");
    }

    #[test]
    #[cfg(not(windows))]
    fn bash_command_get_output_returns_stderr() {
        // Arrange
        let bash_exec_config = ExecutionConfigVariant::ShellCommand(
            ShellCommandConfigVariant::Bash(BashCommandConfig {
                working_directory: None,
                command: ">&2 echo \"Error message\"".to_string(),
            }),
        );
        let command_executor = create_command_executor(&DingusOptions::default());

        // Act
        let result = command_executor.get_output(&bash_exec_config, &HashMap::new());
        assert!(!result.is_err());

        // Assert
        let output = result.unwrap();
        assert_eq!(output.status, ExitStatus::Success);
        assert!(output.stdout.is_empty());

        let output_value = String::from_utf8(output.stderr).unwrap();
        assert_eq!(output_value, "Error message\n");
    }

    #[test]
    #[cfg(not(windows))]
    fn bash_command_get_output_returns_exit_code() {
        // Arrange
        let bash_exec_config = ExecutionConfigVariant::ShellCommand(
            ShellCommandConfigVariant::Bash(BashCommandConfig {
                working_directory: None,
                command: "exit 42".to_string(),
            }),
        );
        let command_executor = create_command_executor(&DingusOptions::default());

        // Act
        let result = command_executor.get_output(&bash_exec_config, &HashMap::new());
        assert!(!result.is_err());

        // Assert
        let output = result.unwrap();
        assert_eq!(output.status, ExitStatus::Fail(42));
        assert!(output.stdout.is_empty());
        assert!(output.stderr.is_empty());
    }

    #[test]
    #[cfg(not(windows))]
    fn bash_command_honours_workdir() {
        // Arrange
        let bash_exec_config = ExecutionConfigVariant::ShellCommand(
            ShellCommandConfigVariant::Bash(BashCommandConfig {
                working_directory: Some("./src".to_string()),
                command: "pwd".to_string(),
            }),
        );
        let command_executor = create_command_executor(&DingusOptions::default());

        // Act
        let result = command_executor.get_output(&bash_exec_config, &HashMap::new());
        assert!(!result.is_err());

        // Assert
        let output = result.unwrap();
        assert_eq!(output.status, ExitStatus::Success);
        assert!(output.stderr.is_empty());

        let output_value = String::from_utf8(output.stdout).unwrap();
        assert!(output_value.ends_with("/src\n"));
    }

    #[test]
    fn raw_command_execute_executes_command() {
        // Arrange
        let temp_dir = create_temp_dir();
        let file_name = "dingus.txt";
        let test_file_path = temp_dir.path().join(file_name);

        // Sanity check
        assert_eq!(test_file_path.exists(), false);

        let bash_exec_config = ExecutionConfigVariant::RawCommand(
            RawCommandConfigVariant::Shorthand(format!("touch {}", get_path(&test_file_path))),
        );
        let command_executor = create_command_executor(&DingusOptions::default());

        // Act
        let result = command_executor.execute(&bash_exec_config, &Default::default());
        assert!(!result.is_err());

        // Assert
        let exit_status = result.unwrap();
        assert!(matches!(exit_status, ExitStatus::Success));
        assert_eq!(test_file_path.exists(), true);
    }

    #[test]
    fn raw_command_execute_substitutes_variables_in_invocation() {
        // Arrange
        let temp_dir = create_temp_dir();
        let file_name = "dingus.txt";
        let test_file_path = temp_dir.path().join(file_name);

        // Sanity check
        assert_eq!(Path::new(&test_file_path).exists(), false);

        let variable_name = "file_name";
        let mut variables = HashMap::new();
        variables.insert(variable_name.to_string(), get_path(&test_file_path));

        let exec_config = ExecutionConfigVariant::RawCommand(RawCommandConfigVariant::Shorthand(
            "touch $file_name".to_string(),
        ));
        let command_executor = create_command_executor(&DingusOptions::default());

        // Act
        let result = command_executor.execute(&exec_config, &variables);
        assert!(!result.is_err());

        // Assert
        let exit_status = result.unwrap();
        assert!(matches!(exit_status, ExitStatus::Success));
        assert_eq!(test_file_path.exists(), true);
    }

    #[test]
    fn raw_command_execute_returns_exit_code() {
        // Arrange
        let exec_config = ExecutionConfigVariant::RawCommand(RawCommandConfigVariant::Shorthand(
            "cargo silly".to_string(),
        ));
        let command_executor = create_command_executor(&DingusOptions::default());

        // Act
        let result = command_executor.execute(&exec_config, &Default::default());
        assert!(!result.is_err());

        // Assert
        let exit_status = result.unwrap();
        assert!(matches!(exit_status, ExitStatus::Fail(101)));
    }

    #[test]
    fn raw_command_get_output_substitutes_variables_in_invocation() {
        // Arrange
        let content = "Hello, World!";
        let variable_name = "file_name";
        let temp_file = create_temp_file(content);
        let mut variables = HashMap::new();
        variables.insert(
            variable_name.to_string(),
            temp_file.path().to_str().unwrap().to_string(),
        );

        let exec_config = ExecutionConfigVariant::RawCommand(RawCommandConfigVariant::Shorthand(
            "cat $file_name".to_string(),
        ));
        let command_executor = create_command_executor(&DingusOptions::default());

        // Act
        let result = command_executor.get_output(&exec_config, &variables);
        assert!(!result.is_err());

        // Assert
        let output = result.unwrap();
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert_eq!(stderr, "");

        assert_eq!(output.status, ExitStatus::Success);

        let output_value = String::from_utf8(output.stdout).unwrap();
        assert_eq!(output_value, content);
    }

    // TODO: Re-implement. This is flaky.
    #[test]
    #[ignore]
    fn raw_command_get_output_has_variables() {
        // Arrange
        let variable_name = "CARGO_ALIAS_V";
        let variable_value = "version";
        let mut variables = HashMap::new();
        variables.insert(variable_name.to_string(), variable_value.to_string());

        let exec_config = ExecutionConfigVariant::RawCommand(RawCommandConfigVariant::Shorthand(
            "cargo v".to_string(),
        ));
        let command_executor = create_command_executor(&DingusOptions::default());

        // Act
        let result = command_executor.get_output(&exec_config, &variables);
        assert!(!result.is_err());

        // Assert
        let output = result.unwrap();
        assert_eq!(output.status, ExitStatus::Success);
        assert!(output.stderr.is_empty());

        let output_value = String::from_utf8(output.stdout).unwrap();
        assert!(output_value.contains("cargo 1.78.0"));
    }

    #[test]
    fn raw_command_get_output_returns_stdout() {
        // Arrange
        let content = "Hello, World!";
        let temp_file = create_temp_file(content);
        let temp_file_path = temp_file.path().to_str().unwrap().to_string();

        let exec_config = ExecutionConfigVariant::RawCommand(RawCommandConfigVariant::Shorthand(
            format!("cat {temp_file_path}").to_string(),
        ));
        let command_executor = create_command_executor(&DingusOptions::default());

        // Act
        let result = command_executor.get_output(&exec_config, &HashMap::new());
        assert!(!result.is_err());

        // Assert
        let output = result.unwrap();
        assert_eq!(output.status, ExitStatus::Success);
        assert!(output.stderr.is_empty());

        let output_value = String::from_utf8(output.stdout).unwrap();
        assert_eq!(output_value, content);
    }

    #[test]
    fn raw_command_get_output_returns_stderr() {
        // Arrange
        let exec_config = ExecutionConfigVariant::RawCommand(RawCommandConfigVariant::Shorthand(
            "cat does_not_exist.txt".to_string(),
        ));
        let command_executor = create_command_executor(&DingusOptions::default());

        // Act
        let result = command_executor.get_output(&exec_config, &HashMap::new());
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
        let exec_config = ExecutionConfigVariant::RawCommand(
            RawCommandConfigVariant::RawCommandConfig(RawCommandConfig {
                working_directory: Some("./src".to_string()),
                command: "pwd".to_string(),
            }),
        );
        let command_executor = create_command_executor(&DingusOptions::default());

        // Act
        let result = command_executor.get_output(&exec_config, &HashMap::new());
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
        let exec_config = ExecutionConfigVariant::RawCommand(
            RawCommandConfigVariant::RawCommandConfig(RawCommandConfig {
                working_directory: None,
                command: "shopt -s expand_aliases".to_string(),
            }),
        );
        let command_executor = create_command_executor(&DingusOptions::default());

        // Act
        let result = command_executor.get_output(&exec_config, &HashMap::new());

        // Assert
        assert!(result.is_err());
    }

    fn create_temp_dir() -> TempDir {
        let temp_dir = TempDir::new().unwrap();
        return temp_dir;
    }

    fn create_empty_temp_file() -> NamedTempFile {
        let temp_file = NamedTempFile::new().unwrap();
        return temp_file;
    }

    fn create_temp_file(content: &str) -> NamedTempFile {
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(content.as_bytes()).unwrap();
        return temp_file;
    }

    fn get_path(path: &Path) -> String {
        return path.to_str().unwrap().to_string();
    }
}
