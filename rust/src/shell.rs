use std::error::Error;
use std::{fmt, io};
use std::process::{Command, ExitStatus, Output};
use crate::definitions::Shell;

use crate::variables::Variables;

pub type ShellCommand = String;
type ShellExecutionResult = Result<ExitStatus, ShellError>;
type ShellOutputResult = Result<Output, ShellError>;

pub trait ShellExecutor {
    fn execute(&self, command: &ShellCommand, variables: &Variables) -> ShellExecutionResult;
    fn get_output(&self, command: &ShellCommand) -> ShellOutputResult;
}

pub fn create_shell_executor(shell: &Shell) -> impl ShellExecutor {
    match shell {
        Shell::Bash => BashExecutor{}
    }
}

struct BashExecutor { }

impl ShellExecutor for BashExecutor {

    fn execute(&self, command: &ShellCommand, variables: &Variables) -> ShellExecutionResult {

        let mut binding = Command::new("bash");
        let cmd = binding
            .arg("-c")
            .arg(command)
            .envs(variables);

        // When invoked using spawn, this will inherit stdin, stdout, and stdin from this process
        if let Ok(mut child) = cmd.spawn() {
            let result = child.wait();

            return match result {
                Ok(exit_status) => Ok(exit_status),
                Err(io_err) => Err(ShellError::IO(io_err)),
            }
        } else {
            return Err(ShellError::FailedToStart)
        }
    }

    fn get_output(&self, command: &ShellCommand) -> ShellOutputResult {
        let result = Command::new("bash")
            .arg("-c")
            .arg(command)
            .output();

        return match result {
            Ok(output) => Ok(output),
            Err(io_err) => Err(ShellError::IO(io_err)),
        }
    }
}

#[derive(Debug)]
pub enum ShellError {
    FailedToStart,
    IO(io::Error)
}

impl Error for ShellError {}

impl fmt::Display for ShellError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ShellError::FailedToStart => write!(f, "process failed to start"),
            ShellError::IO(io_error) => io_error.fmt(f),
        }
    }
}
