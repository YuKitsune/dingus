use std::{fmt, fs};
use std::collections::HashMap;
use std::error::Error;

use clap::{Arg, ArgMatches, Command};

use definitions::*;
use variables::{VariableResolver};

use crate::execution::CommandExecutor;
use crate::prompt::{ConfirmExecutor, PromptExecutor, SelectExecutor};

mod definitions;
mod execution;
mod variables;
mod prompt;

fn main() {
    let result = main_with_result();
    if let Err(err) = result {
        panic!("{}", err)
    }
}

fn main_with_result() -> Result<(), Box<dyn Error>> {
    let config = read_config().unwrap();
    let mut variable_definitions = config.variables.clone();

    let root_args = create_args_for_command(&config.variables);
    let sub_commands = create_subcommands(&config.commands, &config.variables);
    let root_command = Command::new("shiji")
        .about(config.description)
        .subcommands(sub_commands)
        .subcommand_required(true)
        .args(root_args);

    let command_executor = &CommandExecutor{};

    let variable_resolver = &VariableResolver {
        command_executor: CommandExecutor{},
        prompt_executor: PromptExecutor{},
        select_executor: SelectExecutor{
            command_executor: CommandExecutor{}
        }
    };

    let confirm_executor = &ConfirmExecutor{};

    let arg_matches = root_command.clone().get_matches();
    if let Some((subcommand_name, subcommand_matches)) = arg_matches.subcommand() {
        if let Some(subcommand) = root_command.clone().find_subcommand(subcommand_name) {
            if let Some(command_definition) = config.commands.get(subcommand_name) {
                return execute_command(command_executor, subcommand, command_definition, &mut variable_definitions, variable_resolver, confirm_executor, &subcommand_matches)
            }
        }
    }

    todo!("didn't match any commands")
}

fn create_subcommands(
    command_definitions: &HashMap<String, CommandDefinition>,
    parent_variable_definitions: &HashMap<String, VariableDefinition>) -> Vec<Command> {
    command_definitions.iter()
        .map(|(name, definition)| -> Command {

            // Combine the variable definitions from the parent with the variable definitions from the current command.
            // This lets us inherit variables from the root config/parent commands.
            let mut variables = parent_variable_definitions.clone();
            variables.extend(definition.variables.clone());

            let subcommands = create_subcommands(
                &definition.commands,
                &variables);

            let args = create_args_for_command(&variables);

            let command = Command::new(name)
                .about(definition.description.clone())
                .subcommands(subcommands)
                .args(args);

            return command;
        })
        .collect()
}

fn create_args_for_command(variable_definitions: &HashMap<String, VariableDefinition>) -> Vec<Arg> {
    variable_definitions.iter()
        .filter(|(_, variable_definition)| -> bool {
            return if matches!(variable_definition, VariableDefinition::Literal(_)) || matches!(variable_definition, VariableDefinition::Invocation(_)) {
                false
            } else {
                true
            }
        })
        .map(|(name, variable_definition)| -> Arg {

            let (flag, description) = match variable_definition {
                VariableDefinition::Prompt(prompt_variable_def) =>
                    (prompt_variable_def.clone().prompt.flag.unwrap_or(name.to_string()), prompt_variable_def.clone().prompt.description),
                VariableDefinition::Select(select_variable_def) =>
                    (select_variable_def.clone().select.flag.unwrap_or(name.to_string()), select_variable_def.clone().select.description),
                _ => {
                    panic!("This shouldn't happen")
                }
            };

            let arg = Arg::new(flag.clone())
                .long(flag.clone())
                .help(description);

            return arg
        })
        .collect()
}

fn read_config() -> Result<Config, Box<dyn Error>> {
    let config_text: String = fs::read_to_string("example.yaml")?;
    let config: Config = serde_yaml::from_str(&config_text)?;
    Ok(config)
}

fn execute_command(
    command_executor: &CommandExecutor,
    command: &Command,
    command_definition: &CommandDefinition,
    variable_definitions: &mut HashMap<String, VariableDefinition>,
    variable_resolver: &VariableResolver,
    confirm_executor: &ConfirmExecutor,
    arg_matches: &ArgMatches) -> Result<(), Box<dyn Error>> {

    // Combine the variables from this command with the parent variables
    variable_definitions.extend(command_definition.variables.clone());

    // Try to find any further matches on a subcommand
    if let Some((subcommand_name, subcommand_matches)) = arg_matches.subcommand() {
        if let Some(subcommand) = command.clone().find_subcommand(subcommand_name) {
            if let Some(command_definition) = command_definition.commands.get(subcommand_name) {
                return execute_command(command_executor, subcommand, command_definition, variable_definitions, variable_resolver, confirm_executor, &subcommand_matches)
            }
        }
    }

    return match &command_definition.action {
        CommandActions::SingleStep(step) => execute_action(&step.action, variable_definitions, command_executor, confirm_executor, variable_resolver, arg_matches),
        CommandActions::MultiStep(steps) => execute_actions(&steps.actions, variable_definitions, command_executor, confirm_executor, variable_resolver, arg_matches)
    }
}

fn execute_actions(
    command_actions: &Vec<CommandAction>,
    variable_definitions: &HashMap<String, VariableDefinition>,
    command_executor: &CommandExecutor,
    confirm_executor: &ConfirmExecutor,
    variable_resolver: &VariableResolver,
    arg_matches: &ArgMatches) -> Result<(), Box<dyn Error>> {

    // TODO: Evaluate variables here

    for command_action in command_actions {
        execute_action(command_action, variable_definitions, command_executor, confirm_executor, variable_resolver, arg_matches)?;
    }

    Ok(())
}

fn execute_action(
    command_action: &CommandAction,
    variable_definitions: &HashMap<String, VariableDefinition>,
    command_executor: &CommandExecutor,
    confirm_executor: &ConfirmExecutor,
    variable_resolver: &VariableResolver,
    arg_matches: &ArgMatches) -> Result<(), Box<dyn Error>> {

    // TODO: Evaluate variables here

    return match command_action {
        CommandAction::Invocation(invocation) => {

            let variables = variable_resolver.resolve_variables(variable_definitions, arg_matches)?;

            command_executor.execute(invocation.as_str(), &variables)
        },
        CommandAction::Confirmation(confirm_definition) => {
            let result = confirm_executor.execute(confirm_definition)?;
            if result == false {
                return Err(Box::new(ConfirmationError))
            }

            Ok(())
        }
    }
}

#[derive(Debug, Clone)]
struct ConfirmationError;

// Generation of an error is completely separate from how it is displayed.
// There's no need to be concerned about cluttering complex logic with the display style.
//
// Note that we don't store any extra info about the errors. This means we can't state
// which string failed to parse without modifying our types to carry that information.
impl fmt::Display for ConfirmationError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "confirmation resulted in a negative result")
    }
}

impl Error for ConfirmationError { }