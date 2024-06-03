use std::{fmt, process};
use std::error::Error;
use crate::args::ClapArgumentResolver;

use crate::commands::{ActionExecutor, ActionId, ActionKey};
use crate::config::CommandActionConfigVariant;
use crate::exec::create_command_executor;
use crate::prompt::{ConfirmExecutor, TerminalPromptExecutor};
use crate::variables::VariableResolver;

mod exec;
mod prompt;
mod commands;
mod cli;
mod config;
mod args;
mod variables;

// Todo:
// - [ ] Consider naming (Variables, Commands, Actions, all confusing)
// - [ ] Offer to create a gecko file if none exists
// - [ ] Integration tests
// - [ ] Meta commands
// - [ ] Documentation (in-code and public-facing)
// - [ ] Publish v0.1.0

// Ideas:
// - Preconditions: Specify a list of applications that must be installed before running a command, or a custom script that must succeed
// - Named actions: Actions can be named so that they can be skipped selectively
// - Command invocation action: Have an action that invokes another command (Or named action ^), can specify the args
// - Deferred actions: Always executes at the end, even if one of the actions fails
// - Include other gecko files (on disk or with a GitHub link)
// - Pipe config file: example.yaml | gecko do something
// - Aliases: Have a command alias another command (E.g: gecko deps = docker compose -f deps.yaml). remaining args are passed to the child command
// - Platform-specific commands.
// - Cached variable results: Allow the results of an execution variable to be cached for future use.
// - Remote commands: Execute commands on a remote machine (Like a mini Ansible)

fn main() {
    let result = run();
    if let Err(err) = result {
        eprintln!("{}", err);
        process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let config = config::load()?;

    let root_command = cli::create_root_command(&config);

    // This will exit on any match failures
    let arg_matches = root_command.clone().get_matches();

    let find_result = cli::find_subcommand(
        &arg_matches,
        &root_command,
        &config.commands,
        &config.variables)?;

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
            let variable_resolver = VariableResolver {
                command_executor: create_command_executor(),
                prompt_executor: Box::new(TerminalPromptExecutor::new(create_command_executor())),
                argument_resolver: Box::new(arg_resolver),
            };

            let action_executor = ActionExecutor {
                command_executor: create_command_executor(),
                confirm_executor: ConfirmExecutor{},
                variable_resolver,
            };

            // Execute the actions
            for (idx, action) in actions.iter().enumerate() {

                let action_id = ActionId {
                    command_name: arg_matches.subcommand_name().unwrap().to_string(),
                    action: ActionKey::Unnamed(idx)
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
