use std::{fmt, process};
use std::error::Error;
use crate::args::ClapArgumentResolver;
use crate::cli::MetaCommandResult;
use crate::actions::{ActionExecutor, ActionId};
use crate::config::{CommandActionConfigVariant, ConfigError};
use crate::exec::create_command_executor;
use crate::prompt::{InquireConfirmExecutor, TerminalPromptExecutor};
use crate::variables::{RealVariableResolver};

mod exec;
mod prompt;
mod actions;
mod cli;
mod config;
mod args;
mod variables;

// Todo:
// - [ ] Second pass for tests
// - [ ] Consider that whole action key thing
// - [ ] Config validation
// - [ ] Refine error messages (and have tests for them)
// - [ ] Documentation (in-code and public-facing)
// - [ ] Publish v0.1.0

// Ideas:
// - Named actions: Actions can be named so that they can be skipped selectively (--skip arg vs custom conditional stuff per action)
// - Preconditions: Specify a list of applications that must be installed, or a custom script that must succeed before running a command
// - Command invocation action: Actions can invoke other commands (Or named action ^). Variables can be passed to the command.
// - Deferred actions: Always executes at the end, even if one of the actions fails.
// - Include other gecko files (on disk or with a GitHub link)
// - Pipe config file: example.yaml | gecko do something
// - Aliases: Commands can act as an alias for another command (E.g: gecko deps = docker compose -f deps.yaml). Remaining args are passed to the child command. (Naming issue here since commands can have aliases)
// - Platform-specific commands.
// - Cached variable results: Allow the results of an execution variable to be cached on disk for future use.
// - Remote commands: Execute commands on a remote machine (Like a mini Ansible)
// - GitHub Actions integration: Run gecko commands as part of a GitHub Actions workflow

fn main() {
    let result = run();
    if let Err(err) = result {
        eprintln!("{}", err);
        process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let config_result = config::load();

    // Offer to create the config file if one doesn't exist
    if let Err(config_err) = config_result {
        return match config_err {
            ConfigError::FileNotFound => {
                let should_init = inquire::Confirm::new("Couldn't find a gecko file in this directory. Do you want to create one?")
                    .with_default(true)
                    .prompt()?;

                if !should_init {
                    return Err(Box::new(config_err))
                }

                let file_name = config::init().map_err(|err| Box::new(err))?;
                println!("created {file_name}");
                return Ok(())
            },
            _ => Err(Box::new(config_err))
        }
    }

    let config = config_result.unwrap();
    let root_command = cli::create_root_command(&config);

    // This will exit on any match failures
    let arg_matches = root_command.clone().get_matches();

    // Check if the command was a meta command first
    let meta_command_result = cli::find_meta_command(&arg_matches);
    if matches!(meta_command_result, MetaCommandResult::Executed) {
        return Ok(())
    }

    // Otherwise, look for a configured command
    let find_result = cli::find_subcommand(
        &arg_matches,
        &root_command,
        &config.commands,
        &config.variables);

    if let Some((target_command, available_variable_configs, sucbommand_arg_matches)) = find_result {

        if let Some(command_action) = target_command.action {

            // Coalesce single actions into multistep actions.
            // Makes the execution part easier.
            let actions = match command_action {
                CommandActionConfigVariant::SingleStep(single_command_action) =>
                    vec![single_command_action.action],

                CommandActionConfigVariant::MultiStep(multi_command_action) =>
                    multi_command_action.actions
            };

            // Set up the dependencies
            let arg_resolver = ClapArgumentResolver::from_arg_matches(&sucbommand_arg_matches);
            let variable_resolver = RealVariableResolver {
                command_executor: create_command_executor(),
                prompt_executor: Box::new(TerminalPromptExecutor::new(create_command_executor())),
                argument_resolver: Box::new(arg_resolver),
            };

            let action_executor = ActionExecutor {
                command_executor: create_command_executor(),
                confirm_executor: Box::new(InquireConfirmExecutor{}),
                variable_resolver: Box::new(variable_resolver),
            };

            // Execute the actions
            for (idx, action) in actions.iter().enumerate() {

                let action_id = ActionId {
                    command_name: arg_matches.subcommand_name().unwrap().to_string(),
                    action_index: idx
                };

                action_executor.execute(
                    action_id,
                    &action,
                    &available_variable_configs)?;
            }

            return Ok(());
        }
    }

    return Err(Box::new(CommandNotFound{}));
}

#[derive(Debug, Clone)]
struct CommandNotFound;

impl Error for CommandNotFound { }

impl fmt::Display for CommandNotFound {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "could not find a suitable command")
    }
}
