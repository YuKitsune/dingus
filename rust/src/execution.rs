use std::error::Error;
use std::process::{Command};

use crate::variables::Variables;

pub struct CommandExecutor {
}

impl CommandExecutor {

    pub fn execute(&self, command: &str, variables: &Variables) -> Result<(), Box<dyn Error>> {

        // When invoked using spawn, this will inherit stdin, stdout, and stdin from this process
        Command::new("bash")
            .arg("-c")
            .arg(command)
            .envs(variables)
            .spawn()?;
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
