use std::error::Error;
use std::fmt;
use std::process::{Command};

use crate::variables::Variables;

pub struct CommandExecutor {
}

impl CommandExecutor {

    pub fn execute(&self, command: &str, variables: &Variables) -> Result<(), Box<dyn Error>> {

        let mut binding = Command::new("bash");
        let cmd = binding
            .arg("-c")
            .arg(command)
            .envs(variables);

        // When invoked using spawn, this will inherit stdin, stdout, and stdin from this process
        if let Ok(mut child) = cmd.spawn() {
            child.wait().expect("command wasn't running");
        } else {
            return Err(Box::new(FailedToStart{}))
        }

        return Ok(())
    }

    pub fn get_output(&self, command: String) -> Result<String, Box<dyn Error>> {
        let output = Command::new("bash")
            .arg("-c")
            .arg(command)
            .output()?;
        let str = String::from_utf8(output.stdout)?;
        return Ok(str);
    }
}

#[derive(Debug)]
struct FailedToStart{}
impl Error for FailedToStart {}

impl fmt::Display for FailedToStart {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "process failed to start")
    }
}

