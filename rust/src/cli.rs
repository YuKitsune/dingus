use std::collections::HashMap;
use std::error::Error;
use clap::{Arg, ArgMatches, Command};
use crate::definitions::{CommandDefinition, Config, VariableDefinition};

pub fn create_root_command(config: &Config) -> Command {
    let root_args = create_args(&config.variables);
    let subcommands = create_commands(&config.commands, &config.variables);
    let root_command = Command::new("shiji")
        .about(&config.description)
        .subcommands(subcommands)
        .subcommand_required(true)
        .args(root_args);

    return root_command;
}

fn create_commands(
    command_definitions: &HashMap<String, CommandDefinition>,
    parent_variable_definitions: &HashMap<String, VariableDefinition>) -> Vec<Command> {
    command_definitions.iter()
        .map(|(name, definition)| -> Command {

            // Combine the variable definitions provided by the caller (parent) with the variable
            // definitions from the current command.
            // This lets us inherit variables from the root config/parent commands.
            let mut variables = parent_variable_definitions.clone();
            variables.extend(definition.variables.clone());

            let args = create_args(&variables);

            let subcommands = create_commands(
                &definition.commands,
                &variables);

            // If this command doesn't have any action, then it needs a subcommand
            // Doesn't make sense to have a command that does nothing and has no subcommands to
            // execute either.
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

fn create_args(variable_definitions: &HashMap<String, VariableDefinition>) -> Vec<Arg> {
    variable_definitions.iter()
        .map(|(name, variable_definition)| -> Arg {

            let arg_name = variable_definition.arg_name(name);

            let mut arg = Arg::new(arg_name.clone())
                .long(arg_name.clone());

            if let Some(description) = variable_definition.description() {
                arg = arg.help(description)
            }

            return arg
        })
        .collect()
}



pub fn find_subcommand(
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