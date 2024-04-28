use std::{fmt, fs};
use std::collections::HashMap;
use std::error::Error;

use clap::{Arg, ArgMatches, Command};

use definitions::*;
use shell::BashExecutor;
use variables::VariableResolver;

use crate::shell::ShellExecutor;
use crate::prompt::{ConfirmExecutor, PromptExecutor, SelectExecutor};

mod definitions;
mod shell;
mod variables;
mod prompt;

fn main() {
    let result = main_with_result();
    if let Err(err) = result {
        panic!("{}", err)
    }
}

fn main_with_result() -> Result<(), Box<dyn Error>> {
    let config = read_config()?;

    // Configure the clap commands
    let root_args = create_args_for_command(&config.variables);
    let sub_commands = create_subcommands(&config.commands, &config.variables);
    let root_command = Command::new("shiji")
        .about(config.description)
        .subcommands(sub_commands)
        .subcommand_required(true)
        .args(root_args);

    let shell_executor = &BashExecutor{};

    let variable_resolver = &VariableResolver {
        shell_executor: Box::new(BashExecutor{}),
        prompt_executor: PromptExecutor{},
        select_executor: SelectExecutor{
            command_executor: Box::new(BashExecutor{})
        }
    };

    let confirm_executor = &ConfirmExecutor{};

    // This will exit on any match failures
    let arg_matches = root_command.clone().get_matches();

    let find_result = find_subcommand(
        &arg_matches,
        &root_command,
        &config.commands,
        &config.variables)?;

    if let Some((target_command, available_variable_definitions, sucbommand_arg_matches)) = find_result {

        if let Some(command_action) = target_command.action {
            let actions = match command_action {
                CommandActions::SingleStep(single_command_action) => {
                    vec![single_command_action.action]
                }
                CommandActions::MultiStep(multi_command_action) => {
                    multi_command_action.actions
                }
            };

            for action in actions {
                return execute_action(
                    &action,
                    &available_variable_definitions,
                    shell_executor,
                    confirm_executor,
                    variable_resolver,
                    &sucbommand_arg_matches)
            }
        }
    }

    return Err(Box::new(CommandNotFound{}));
}

fn find_subcommand(
    arg_matches: &ArgMatches,
    parent_command: &Command,
    available_commands: &HashMap<String, CommandDefinition>,
    parent_variables: &HashMap<String, VariableDefinition>
) -> Result<Option<SubcommandSearchResult>, Box<dyn Error>> {

    // If we've matched on a subcommand, then lookup the subcommand definition
    if let Some((subcommand_name, subcommand_matches)) = arg_matches.subcommand() {
        let subcommand = parent_command.find_subcommand(subcommand_name).unwrap();
        let command_definition = available_commands.get(subcommand_name).unwrap().to_owned();

        // Add the subcommands variables to the variables provided by the parent
        let mut available_variables = parent_variables.clone();
        available_variables.extend(command_definition.variables.clone());

        // If we've matched another subcommand, return that one instead
        let matched_subcommand = find_subcommand(
            &subcommand_matches,
            &subcommand,
            &command_definition.commands,
            &available_variables)?;
        if matched_subcommand.is_some() {
            return Ok(matched_subcommand)
        }

        // If no more subcommand matches exist, then return the current subcommand
        let result: SubcommandSearchResult = (command_definition.clone(), available_variables, subcommand_matches.clone());
        return Ok(Some(result));
    }

    return Ok(None);
}

type SubcommandSearchResult = (CommandDefinition, HashMap<String, VariableDefinition>, ArgMatches);

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

            let has_action = definition.action.is_some();

            let command = Command::new(name)
                .about(definition.description.clone())
                .subcommands(subcommands)
                .subcommand_required(!has_action)
                .args(args);

            return command;
        })
        .collect()
}

fn create_args_for_command(variable_definitions: &HashMap<String, VariableDefinition>) -> Vec<Arg> {
    variable_definitions.iter()
        .map(|(name, variable_definition)| -> Arg {

            let (flag, description) = match variable_definition {
                VariableDefinition::Literal(_) =>
                    (name.clone(), "".to_string()),
                VariableDefinition::LiteralWithFlag(literal_with_flag) =>
                    (literal_with_flag.flag.clone().unwrap_or(name.to_string()), literal_with_flag.description.clone().unwrap_or("".to_string())),
                VariableDefinition::Invocation(execution_def) =>
                    (execution_def.flag.clone().unwrap_or(name.to_string()), execution_def.description.clone().unwrap_or("".to_string())),
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

fn execute_action(
    command_action: &CommandAction,
    variable_definitions: &HashMap<String, VariableDefinition>,
    shell_executor: &impl ShellExecutor,
    confirm_executor: &ConfirmExecutor,
    variable_resolver: &VariableResolver,
    arg_matches: &ArgMatches) -> Result<(), Box<dyn Error>> {

    let variables = variable_resolver.resolve_variables(variable_definitions, arg_matches)?;

    return match command_action {
        CommandAction::Invocation(invocation) => {

            let result = shell_executor.execute(invocation, &variables);

            // Todo: If the command fails to execute, fail the remaining steps, or seek user input (continue or abort)
            if let Err(err) = result {
                return Err(Box::new(err))
            }

            Ok(())
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

#[derive(Debug, Clone)]
struct CommandNotFound;

impl fmt::Display for CommandNotFound {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "could not find a suitable command")
    }
}

impl Error for CommandNotFound { }