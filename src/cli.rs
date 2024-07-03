use clap::{Arg, ArgMatches, Command, ValueHint};
use crate::args::ALIAS_ARGS_NAME;
use crate::config::{ActionConfig, CommandConfig, CommandConfigMap, Config, ExecutionConfigVariant, RawCommandConfigVariant, ShellCommandConfigVariant, VariableConfig, VariableConfigMap};

/// Creates a root-level [`Command`] for the provided [`Config`].
pub fn create_root_command(config: &Config) -> Command {
    let root_args = create_args(&config.variables);
    let subcommands = create_commands(&config.commands, &config.variables);

    let mut root_command = Command::new("dingus")
        .version(env!("CARGO_PKG_VERSION"))
        .subcommands(subcommands)
        .subcommand_required(true)
        .arg_required_else_help(true)
        .args(root_args);

    if let Some(description) = &config.description {
        root_command = root_command.about(description)
    }

    return root_command;
}

fn create_commands(
    commands: &CommandConfigMap,
    parent_variables: &VariableConfigMap) -> Vec<Command> {
    commands.iter()
        .map(|(key, command_config)| -> Command {

            // Combine the variable configs provided by the caller (parent) with the variable
            // configs from the current command.
            // This lets us inherit variables from the root config/parent commands.
            let mut variables = parent_variables.clone();
            variables.extend(command_config.variables.clone());

            let args = create_args(&variables);

            let subcommands = create_commands(
                &command_config.commands,
                &variables);

            // If this command doesn't have any action, then it needs a subcommand
            // Doesn't make sense to have a command that does nothing and has no subcommands to
            // execute either.
            let has_action = command_config.action.is_some();

            let mut command = Command::new(key)
                .subcommands(subcommands)
                .subcommand_required(!has_action)
                .args(args);

            // If the action is an alias, then we use a special argument for the arguments to pass through to the alias
            if let Some(ActionConfig::Alias(_)) = command_config.action.clone() {
                let raw_args = Arg::new(ALIAS_ARGS_NAME)
                    .num_args(1..)
                    .allow_hyphen_values(true)
                    .trailing_var_arg(true)
                    .value_hint(ValueHint::CommandWithArguments)
                    .help("Arguments and options for the aliased command.");

                command = command.arg(raw_args)
            }

            if let Some(description) = command_config.description.clone() {
                command = command.about(description)
            }

            return command;
        })
        .collect()
}

fn create_args(variable_config_map: &VariableConfigMap) -> Vec<Arg> {
    variable_config_map.iter()
        .map(|(key, var_config)| -> Arg {

            // TODO: Try to convert the variable name to an arg name
            // E.g: `--my-variable` instead of `--my_variable`
            // Should also consider whether or not it's a good idea to have all variables available as args by default
            let arg_name = var_config.arg_name(key);

            let mut arg = Arg::new(arg_name.clone())
                .long(arg_name.clone());

            if let Some(description) = var_config.description() {
                arg = arg.help(description)
            }

            match var_config {
                VariableConfig::ShorthandLiteral(literal) => {
                    arg = arg.default_value(literal)
                }
                VariableConfig::Literal(literal) =>  {
                    arg = arg.default_value(&literal.value)
                }
                VariableConfig::Execution(exec) => {

                    let command = match exec.execution.clone() {
                        ExecutionConfigVariant::RawCommand(command) => {
                            match command {
                                RawCommandConfigVariant::Shorthand(command_text) => command_text,
                                RawCommandConfigVariant::RawCommandConfig(raw_command_config) => raw_command_config.command
                            }
                        }
                        ExecutionConfigVariant::ShellCommand(shell_command) =>
                            match shell_command {
                                ShellCommandConfigVariant::Bash(bash_command) => bash_command.command
                            },
                    };

                    arg = arg.hide_default_value(true)
                        .help(format!("Defaults to the result of executing {command}"));
                }
                VariableConfig::Prompt(_) => {
                    arg = arg.hide_default_value(true)
                        .help("Prompts the user for a value if not specified.");
                }
            }

            return arg
        })
        .collect()
}

/// Finds the [`CommandConfig`], [`VariableConfigMap`], and [`ArgMatches`], matching the provided `arg_matches`.
/// This essentially returns the command to invoke (and it's relevent [`ArgMatches`]), all the variables available to the command.
pub fn find_subcommand(
    arg_matches: &ArgMatches,
    parent_command: &Command,
    available_commands: &CommandConfigMap,
    parent_variables: &VariableConfigMap
) -> Option<SubcommandSearchResult> {
    if let Some((subcommand_name, subcommand_matches)) = arg_matches.subcommand() {

        // Safe to unwrap: we wouldn't have matched on anything if the command didn't exist
        let subcommand = parent_command.find_subcommand(subcommand_name).unwrap();
        let command_config = available_commands.get(subcommand_name).unwrap().to_owned();

        // Add the subcommands variables to the variables provided by the parent
        let mut available_variables = parent_variables.clone();
        available_variables.extend(command_config.variables.clone());

        // If we've matched another subcommand, return that one instead
        let matched_subcommand = find_subcommand(
            &subcommand_matches,
            &subcommand,
            &command_config.commands,
            &available_variables);
        if matched_subcommand.is_some() {
            return matched_subcommand
        }

        // If no more subcommand matches exist, then return the current subcommand
        let result: SubcommandSearchResult = (command_config.clone(), available_variables, subcommand_matches.clone());
        return Some(result);
    }

    return None;
}

type SubcommandSearchResult = (CommandConfig, VariableConfigMap, ArgMatches);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ActionConfig, AliasActionConfig, CommandConfig, ExecutionVariableConfig, LiteralVariableConfig, PromptConfig, PromptVariableConfig, SingleActionConfig, VariableConfig};
    use crate::config::RawCommandConfigVariant::Shorthand;

    #[test]
    fn create_commands_creates_subcommands() {

        // Arrange
        let mut subcommands = CommandConfigMap::new();
        subcommands.insert("sub-1".to_string(), CommandConfig {
            description: Some("Sub 1 description".to_string()),
            variables: Default::default(),
            commands: Default::default(),
            action: Some(ActionConfig::SingleStep(SingleActionConfig{
                action: ExecutionConfigVariant::RawCommand(Shorthand("echo \"Hello, World!\"".to_string()))
            })),
        });

        let mut subcommand_variables = VariableConfigMap::new();
        subcommand_variables.insert("sub-var".to_string(), VariableConfig::ShorthandLiteral("bar".to_string()));

        subcommands.insert("sub-2".to_string(), CommandConfig {
            description: Some("Sub 2 description".to_string()),
            variables: subcommand_variables,
            commands: Default::default(),
            action: Some(ActionConfig::SingleStep(SingleActionConfig{
                action: ExecutionConfigVariant::RawCommand(Shorthand("echo \"Hello, World!\"".to_string()))
            })),
        });

        let mut parent_variables = VariableConfigMap::new();
        parent_variables.insert("parent-var".to_string(), VariableConfig::ShorthandLiteral("foo".to_string()));

        // Act
        let created_subcommands = create_commands(&subcommands, &parent_variables);
        assert_eq!(created_subcommands.len(), 2);

        let subcommand_1 = created_subcommands.iter().find(|cmd| cmd.get_name() == "sub-1").unwrap();
        assert_eq!(subcommand_1.get_about().unwrap().to_string(), "Sub 1 description");

        let subcommand_2 = created_subcommands.iter().find(|cmd| cmd.get_name() == "sub-2").unwrap();
        assert_eq!(subcommand_2.get_about().unwrap().to_string(), "Sub 2 description");
    }

    #[test]
    fn create_commands_creates_correct_args() {

        // Arrange
        let mut subcommand_variables = VariableConfigMap::new();
        subcommand_variables.insert("sub-var-1".to_string(), VariableConfig::Execution(ExecutionVariableConfig {
            execution: ExecutionConfigVariant::RawCommand(RawCommandConfigVariant::Shorthand("echo \"Hello, World!\"".to_string())),
            description: None,
            argument_name: None,
        }));
        subcommand_variables.insert("sub-var-2".to_string(), VariableConfig::Prompt(PromptVariableConfig {
            description: None,
            argument_name: None,
            prompt: PromptConfig { message: "What's your name?".to_string(), options: Default::default() },
        }));

        let mut subcommands = CommandConfigMap::new();
        subcommands.insert("sub".to_string(), CommandConfig {
            description: None,
            variables: subcommand_variables,
            commands: Default::default(),
            action: Some(ActionConfig::SingleStep(SingleActionConfig{
                action: ExecutionConfigVariant::RawCommand(Shorthand("echo \"Hello, World!\"".to_string()))
            })),
        });

        let mut parent_variables = VariableConfigMap::new();
        parent_variables.insert("parent-var-1".to_string(), VariableConfig::ShorthandLiteral("foo".to_string()));
        parent_variables.insert("parent-var-2".to_string(), VariableConfig::Literal(LiteralVariableConfig {
            value: "bar".to_string(),
            description: None,
            argument_name: None,
        }));

        // Act
        let created_subcommands = create_commands(&subcommands, &parent_variables);

        // Assert
        let command = created_subcommands.get(0).unwrap();
        let command_args: Vec<&Arg> = command.get_arguments().collect();
        assert_eq!(command_args.len(), 4);

        let parent_arg_1 = command_args.iter().find(|arg| arg.get_id() == "parent-var-1").unwrap();
        assert_eq!(parent_arg_1.get_id().as_str(), "parent-var-1");
        assert_eq!(parent_arg_1.get_default_values(), ["foo"]);

        let parent_arg_2 = command_args.iter().find(|arg| arg.get_id() == "parent-var-2").unwrap();
        assert_eq!(parent_arg_2.get_id().as_str(), "parent-var-2");
        assert_eq!(parent_arg_2.get_default_values(), ["bar"]);

        let sub_arg_1 = command_args.iter().find(|arg| arg.get_id() == "sub-var-1").unwrap();
        assert_eq!(sub_arg_1.get_id().as_str(), "sub-var-1");
        assert_eq!(sub_arg_1.get_help().unwrap().to_string(), "Defaults to the result of executing echo \"Hello, World!\"".to_string());

        let sub_arg_2 = command_args.iter().find(|arg| arg.get_id() == "sub-var-2").unwrap();
        assert_eq!(sub_arg_2.get_id().as_str(), "sub-var-2");
        assert_eq!(sub_arg_2.get_help().unwrap().to_string(), "Prompts the user for a value if not specified.");
    }

    #[test]
    fn create_commands_inherits_args_from_parent_commands() {

        // Arrange
        let mut subsubcommand_variables = VariableConfigMap::new();
        subsubcommand_variables.insert("sub-var-2".to_string(), VariableConfig::Prompt(PromptVariableConfig {
            description: None,
            argument_name: None,
            prompt: PromptConfig { message: "What's your name?".to_string(), options: Default::default() },
        }));

        let mut subsubcommands = CommandConfigMap::new();
        subsubcommands.insert("sub-again".to_string(), CommandConfig {
            description: None,
            variables: subsubcommand_variables,
            commands: Default::default(),
            action: Some(ActionConfig::SingleStep(SingleActionConfig{
                action: ExecutionConfigVariant::RawCommand(Shorthand("echo \"Hello, World!\"".to_string()))
            })),
        });

        let mut subcommand_variables = VariableConfigMap::new();
        subcommand_variables.insert("sub-var-1".to_string(), VariableConfig::Execution(ExecutionVariableConfig {
            execution: ExecutionConfigVariant::RawCommand(RawCommandConfigVariant::Shorthand("echo \"Hello, World!\"".to_string())),
            description: None,
            argument_name: None,
        }));

        let mut subcommands = CommandConfigMap::new();
        subcommands.insert("sub".to_string(), CommandConfig {
            description: None,
            variables: subcommand_variables,
            commands: subsubcommands,
            action: Some(ActionConfig::SingleStep(SingleActionConfig{
                action: ExecutionConfigVariant::RawCommand(Shorthand("echo \"Hello, World!\"".to_string()))
            })),
        });

        // Act
        let created_subcommands = create_commands(&subcommands, &VariableConfigMap::new());

        // Assert
        let command = created_subcommands.get(0).unwrap();
        let subcommands: Vec<&Command> = command.get_subcommands().collect();
        let subcommand = subcommands.get(0).unwrap();
        let subcommand_args: Vec<&Arg> = subcommand.get_arguments().collect();
        assert_eq!(subcommand_args.len(), 2);

        let parent_arg = subcommand_args.iter().find(|arg| arg.get_id() == "sub-var-1").unwrap();
        assert_eq!(parent_arg.get_id().as_str(), "sub-var-1");
        assert_eq!(parent_arg.get_help().unwrap().to_string(), "Defaults to the result of executing echo \"Hello, World!\"".to_string());

        let subcommand_arg = subcommand_args.iter().find(|arg| arg.get_id() == "sub-var-2").unwrap();
        assert_eq!(subcommand_arg.get_id().as_str(), "sub-var-2");
        assert_eq!(subcommand_arg.get_help().unwrap().to_string(), "Prompts the user for a value if not specified.");
    }

    #[test]
    fn create_commands_marks_command_as_required() {

        // Arrange
        let mut subsubcommands = CommandConfigMap::new();
        subsubcommands.insert("sub-again".to_string(), CommandConfig {
            description: None,
            variables: Default::default(),
            commands: Default::default(),
            action: Some(ActionConfig::SingleStep(SingleActionConfig{
                action: ExecutionConfigVariant::RawCommand(Shorthand("echo \"Hello, World!\"".to_string()))
            })),
        });

        let mut subcommands = CommandConfigMap::new();
        subcommands.insert("sub".to_string(), CommandConfig {
            description: None,
            variables: Default::default(),
            commands: subsubcommands,
            action: None,
        });

        // Act
        let created_subcommands = create_commands(&subcommands, &VariableConfigMap::new());

        // Assert
        let parent_command = created_subcommands.get(0).unwrap();
        assert!(parent_command.is_subcommand_required_set());

        let subcommands: Vec<&Command> = parent_command.get_subcommands().collect();
        let subcommand = subcommands.get(0).unwrap();
        assert_eq!(subcommand.is_subcommand_required_set(), false);
    }

    #[test]
    fn create_commands_creates_correct_command_for_alias_command() {

        // Arrange
        let mut subcommands = CommandConfigMap::new();
        subcommands.insert("alias".to_string(), CommandConfig {
            description: None,
            variables: Default::default(),
            commands: Default::default(),
            action: Some(ActionConfig::Alias(AliasActionConfig{ alias: "docker compose".to_string() })),
        });

        // Act
        let created_subcommands = create_commands(&subcommands, &VariableConfigMap::new());

        // Assert
        let command = created_subcommands.get(0).unwrap();
        let command_args: Vec<&Arg> = command.get_arguments().collect();
        assert_eq!(command_args.len(), 1);

        let alias_arg = command_args.iter().find(|arg| arg.get_id() == "ARGS").unwrap();
        assert_eq!(alias_arg.get_help().unwrap().to_string(), "Arguments and options for the aliased command.".to_string());
        assert_eq!(alias_arg.is_allow_hyphen_values_set(), true);
        assert_eq!(alias_arg.is_trailing_var_arg_set(), true);
    }

    #[test]
    fn create_args_creates_correct_args() {

        // Arrange
        let mut variables = VariableConfigMap::new();
        variables.insert("var-1".to_string(), VariableConfig::ShorthandLiteral("foo".to_string()));
        variables.insert("var-2".to_string(), VariableConfig::Literal(LiteralVariableConfig {
            value: "bar".to_string(),
            description: None,
            argument_name: None,
        }));
        variables.insert("var-3".to_string(), VariableConfig::Execution(ExecutionVariableConfig {
            execution: ExecutionConfigVariant::RawCommand(RawCommandConfigVariant::Shorthand("echo \"Hello, World!\"".to_string())),
            description: None,
            argument_name: None,
        }));
        variables.insert("var-4".to_string(), VariableConfig::Prompt(PromptVariableConfig {
            description: None,
            argument_name: None,
            prompt: PromptConfig { message: "What's your name?".to_string(), options: Default::default() },
        }));

        // Act
        let args = create_args(&variables);

        // Assert
        let var1 = args.iter().find(|v| v.get_id() == "var-1").unwrap();
        assert_eq!(var1.get_id().as_str(), "var-1");
        assert_eq!(var1.get_default_values(), ["foo"]);

        let var2 = args.iter().find(|v| v.get_id() == "var-2").unwrap();
        assert_eq!(var2.get_id().as_str(), "var-2");
        assert_eq!(var2.get_default_values(), ["bar"]);

        let var3 = args.iter().find(|v| v.get_id() == "var-3").unwrap();
        assert_eq!(var3.get_id().as_str(), "var-3");
        assert_eq!(var3.get_help().unwrap().to_string(), "Defaults to the result of executing echo \"Hello, World!\"".to_string());

        let var4 = args.iter().find(|v| v.get_id() == "var-4").unwrap();
        assert_eq!(var4.get_id().as_str(), "var-4");
        assert_eq!(var4.get_help().unwrap().to_string(), "Prompts the user for a value if not specified.");
    }

    #[test]
    fn find_subcommand_finds_top_level_command() {

        // Arrange
        let mut root_variables = VariableConfigMap::new();
        root_variables.insert("root-var-1".to_string(), VariableConfig::ShorthandLiteral("root value".to_string()));

        let mut subcommand_variables = VariableConfigMap::new();
        subcommand_variables.insert("sub-var-1".to_string(), VariableConfig::ShorthandLiteral("subcommand value".to_string()));

        let mut commands = CommandConfigMap::new();
        commands.insert("cmd".to_string(), CommandConfig {
            description: Some("Top-level command".to_string()),
            variables: subcommand_variables,
            commands: Default::default(),
            action: Some(ActionConfig::SingleStep(SingleActionConfig{
                action: ExecutionConfigVariant::RawCommand(Shorthand("echo \"Hello, World!\"".to_string()))
            })),
        });

        let config = Config {
            description: None,
            variables: root_variables,
            commands: commands
        };

        let root_command = create_root_command(&config);

        // Act
        let matches = root_command.clone().get_matches_from(vec!["dingus", "cmd"]);
        let (found_command, found_variables, _) = find_subcommand(&matches, &root_command, &config.commands, &config.variables).unwrap();

        // Assert
        assert_eq!(found_command.description, Some("Top-level command".to_string()));
        assert!(found_variables.contains_key("root-var-1"));
        assert!(found_variables.contains_key("sub-var-1"));
    }

    #[test]
    fn find_subcommand_finds_mid_level_command() {

        // Arrange
        let mut root_variables = VariableConfigMap::new();
        root_variables.insert("root-var-1".to_string(), VariableConfig::ShorthandLiteral("root value".to_string()));

        let mut parent_command_variables = VariableConfigMap::new();
        parent_command_variables.insert("parent-var-1".to_string(), VariableConfig::ShorthandLiteral("parent command value".to_string()));

        let mut command_variables = VariableConfigMap::new();
        command_variables.insert("target-var-1".to_string(), VariableConfig::ShorthandLiteral("command value".to_string()));

        let mut subcommand_variables = VariableConfigMap::new();
        subcommand_variables.insert("sub-var-1".to_string(), VariableConfig::ShorthandLiteral("subcommand value".to_string()));

        let mut subcommands = CommandConfigMap::new();
        subcommands.insert("sub".to_string(), CommandConfig {
            description: Some("Subcommand".to_string()),
            variables: subcommand_variables,
            commands: CommandConfigMap::default(),
            action: Some(ActionConfig::SingleStep(SingleActionConfig{
                action: ExecutionConfigVariant::RawCommand(Shorthand("echo \"Hello, World!\"".to_string()))
            })),
        });

        let mut target_commands = CommandConfigMap::new();
        target_commands.insert("target".to_string(), CommandConfig {
            description: Some("Mid-level command".to_string()),
            variables: command_variables,
            commands: subcommands,
            action: Some(ActionConfig::SingleStep(SingleActionConfig{
                action: ExecutionConfigVariant::RawCommand(Shorthand("echo \"Hello, World!\"".to_string()))
            })),
        });

        let mut parent_commands = CommandConfigMap::new();
        parent_commands.insert("parent".to_string(), CommandConfig {
            description: Some("Top-level command".to_string()),
            variables: parent_command_variables,
            commands: target_commands,
            action: Some(ActionConfig::SingleStep(SingleActionConfig{
                action: ExecutionConfigVariant::RawCommand(Shorthand("echo \"Hello, World!\"".to_string()))
            })),
        });

        let config = Config {
            description: None,
            variables: root_variables,
            commands: parent_commands
        };

        let root_command = create_root_command(&config);

        // Act
        let matches = root_command.clone().get_matches_from(vec!["dingus", "parent", "target"]);
        let (found_command, found_variables, _) = find_subcommand(&matches, &root_command, &config.commands, &config.variables).unwrap();

        // Assert
        assert_eq!(found_command.description, Some("Mid-level command".to_string()));
        assert!(found_variables.contains_key("root-var-1"));
        assert!(found_variables.contains_key("parent-var-1"));
        assert!(found_variables.contains_key("target-var-1"));
        assert_eq!(found_variables.contains_key("sub-var-1"), false);
    }

    #[test]
    fn find_subcommand_finds_bottom_level_command() {

        // Arrange
        let mut root_variables = VariableConfigMap::new();
        root_variables.insert("root-var-1".to_string(), VariableConfig::ShorthandLiteral("root value".to_string()));

        let mut parent_command_variables = VariableConfigMap::new();
        parent_command_variables.insert("parent-var-1".to_string(), VariableConfig::ShorthandLiteral("parent command value".to_string()));

        let mut command_variables = VariableConfigMap::new();
        command_variables.insert("sub-var-1".to_string(), VariableConfig::ShorthandLiteral("command value".to_string()));

        let mut target_commands = CommandConfigMap::new();
        target_commands.insert("subcommand".to_string(), CommandConfig {
            description: Some("Bottom-level command".to_string()),
            variables: command_variables,
            commands: CommandConfigMap::new(),
            action: Some(ActionConfig::SingleStep(SingleActionConfig{
                action: ExecutionConfigVariant::RawCommand(Shorthand("echo \"Hello, World!\"".to_string()))
            })),
        });

        let mut parent_commands = CommandConfigMap::new();
        parent_commands.insert("parent".to_string(), CommandConfig {
            description: Some("Top-level command".to_string()),
            variables: parent_command_variables,
            commands: target_commands,
            action: Some(ActionConfig::SingleStep(SingleActionConfig{
                action: ExecutionConfigVariant::RawCommand(Shorthand("echo \"Hello, World!\"".to_string()))
            })),
        });

        let config = Config {
            description: None,
            variables: root_variables,
            commands: parent_commands
        };

        let root_command = create_root_command(&config);

        // Act
        let matches = root_command.clone().get_matches_from(vec!["dingus", "parent", "subcommand"]);
        let (found_command, found_variables, _) = find_subcommand(&matches, &root_command, &config.commands, &config.variables).unwrap();

        // Assert
        assert_eq!(found_command.description, Some("Bottom-level command".to_string()));
        assert!(found_variables.contains_key("root-var-1"));
        assert!(found_variables.contains_key("parent-var-1"));
        assert!(found_variables.contains_key("sub-var-1"));
    }
}