use crate::actions::ActionExecutor;
use crate::args::ClapArgumentResolver;
use crate::config::ConfigError;
use crate::exec::create_command_executor;
use crate::platform::current_platform_provider;
use crate::prompt::TerminalPromptExecutor;
use crate::variables::{RealVariableResolver, VariableResolver};
use anyhow::Result;
use std::env;
use thiserror::Error;

mod actions;
mod args;
mod cli;
mod config;
mod exec;
mod platform;
mod prompt;
mod variables;

// Ideas:
// - Preconditions: Specify a list of applications that must be installed, or a custom script that must succeed before running a command
// - Deferred actions: Always executes at the end, even if one of the actions fails.
// - Cached variable results: Allow the results of an execution variable to be cached on disk for future use.
// - Remote commands: Execute commands on a remote machine (Like a mini Ansible)
// - Container actions: Run an action inside a docker container
// - Include other config files with a remote link
// - YAML schema.

fn main() -> Result<()> {
    let config_result = config::load();

    // Offer to create the config file if one doesn't exist
    if let Err(config_err) = config_result {
        return match config_err {
            ConfigError::FileNotFound => {
                let should_init = inquire::Confirm::new(
                    "Couldn't find a config file in this directory. Do you want to create one?",
                )
                .with_default(true)
                .prompt()?;

                if !should_init {
                    return Err(config_err.into());
                }

                let file_name = config::init()?;
                println!("created {file_name}");
                return Ok(());
            }
            _ => Err(config_err.into()),
        };
    }

    let found_config = config_result?;
    let config = found_config.config;

    // Change the current working directory to the directory that the config file came from.
    if let config::Source::File(config_file_path) = found_config.source {
        if let Some(parent_directory) = config_file_path.parent() {
            env::set_current_dir(parent_directory)?;
        }
    }

    let platform_provider = current_platform_provider();

    let root_command = cli::create_root_command(&config, &platform_provider);

    // This will exit on any match failures
    let arg_matches = root_command.clone().get_matches();

    // Otherwise, look for a configured command
    let find_result = cli::find_subcommand(
        &arg_matches,
        &root_command,
        &config.commands,
        &config.variables,
    );

    if let Some((target_command, available_variable_configs, sucbommand_arg_matches)) = find_result
    {
        if let Some(command_action) = target_command.action {
            // Set up the dependencies
            let arg_resolver = ClapArgumentResolver::from_arg_matches(&sucbommand_arg_matches);
            let variable_resolver = RealVariableResolver {
                command_executor: create_command_executor(&config.options),
                prompt_executor: Box::new(TerminalPromptExecutor::new(create_command_executor(
                    &config.options,
                ))),
                argument_resolver: Box::new(arg_resolver),
                dingus_options: config.options.clone(),
            };

            let variables = variable_resolver.resolve_variables(&available_variable_configs)?;

            let action_executor = ActionExecutor {
                command_executor: create_command_executor(&config.options),
                arg_resolver: Box::new(ClapArgumentResolver::from_arg_matches(
                    &sucbommand_arg_matches,
                )),
            };

            action_executor.execute(&command_action, &variables)?;
            return Ok(());
        }
    }

    Err(CommandError::CommandNotFound.into())
}

#[derive(Error, Debug, Clone)]
enum CommandError {
    #[error("could not find a suitable command")]
    CommandNotFound,
}
