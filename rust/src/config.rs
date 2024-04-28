use std::{fmt, fs, io};
use std::error::Error;
use std::path::Path;
use crate::definitions::Config;

// Todo: Consider combining this with definitions, and renaming them all to configs.

const CONFIG_FILE_NAMES: [&str;2] = ["shiji.yaml", "shiji.yml"];

pub fn load() -> Result<Config, ConfigError> {
    for config_file_name in CONFIG_FILE_NAMES {
        if !Path::new(config_file_name).exists() {
            continue
        }

        let config_text: String = fs::read_to_string(config_file_name).map_err(|err| ConfigError::ReadFailed(err))?;
        let config: Config = serde_yaml::from_str(&config_text).map_err(|err| ConfigError::ParseFailed(err))?;

        return Ok(config);
    }

    return Err(ConfigError::FileNotFound)
}

#[derive(Debug)]
pub enum ConfigError {
    FileNotFound,
    ReadFailed(io::Error),
    ParseFailed(serde_yaml::Error)
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ConfigError::FileNotFound => write!(f, "config file not found"),
            ConfigError::ReadFailed(io_err) => write!(f, "failed to read config file: {}", io_err),
            ConfigError::ParseFailed(parse_err) => write!(f, "failed to parse config file: {}", parse_err)
        }
    }
}

impl Error for ConfigError {}