use std::{fmt, process};
use std::error::Error;

use variables::VariableResolver;

use crate::commands::ActionExecutor;
use crate::config::CommandActionConfigVariant;
use crate::prompt::{ConfirmExecutor, PromptExecutor, SelectExecutor};
use crate::shell::{create_shell_executor_factory};
use crate::variables::ArgumentResolver;

mod shell;
mod variables;
mod prompt;
mod commands;
mod cli;
mod config;

// Todo:
// - [ ] Dry-run support
// - [ ] Address todos
// - [ ] Unit tests
// - [ ] Integration tests?
// - [ ] Documentation (in-code and public-facing)
// - [ ] Publish

// Ideas:
// - Named actions: Actions can be named so that they can be skipped selectively
// - Command invocation action: Have an action that invokes another command (Or named action ^)
// - Include other config files
// - Pipe config file: example.yaml | shiji do something
// - Aliases: Have a command alias another command (E.g: shiji deps = docker compose -f deps.yaml). remaining args are passed to the child command
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

    if let Some((target_command, available_variable_definitions, sucbommand_arg_matches)) = find_result {

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
            let arg_resolver = ArgumentResolver::from_arg_matches(&sucbommand_arg_matches);
            let variable_resolver = VariableResolver {
                shell_executor_factory: Box::new(create_shell_executor_factory(&config.default_shell)),
                prompt_executor: PromptExecutor{},
                select_executor: SelectExecutor{
                    shell_executor_factory: Box::new(create_shell_executor_factory(&config.default_shell))
                },
                argument_resolver: arg_resolver
            };

            let action_executor = ActionExecutor {
                shell_executor_factory: Box::new(create_shell_executor_factory(&config.default_shell)),
                confirm_executor: ConfirmExecutor{},
                variable_resolver,
            };

            // Execute the actions
            for action in actions {
                return action_executor.execute(
                    &action,
                    &available_variable_definitions);
            }
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
