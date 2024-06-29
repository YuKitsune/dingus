use clap::ArgMatches;

pub const ALIAS_ARGS_NAME: &str = "ARGS";

/// Capable of resolving command-line argument values.
pub trait ArgumentResolver {

    /// For a given `key`, this will return `Some(String)` with the argument value matching the
    /// key, otherwise `None` is returned.
    fn get(&self, key: &String) -> Option<String>;

    /// For a given `key`, this will return `Some(Vec<String>)` with the argument values matching
    /// the key, otherwise `None` is returned.
    fn get_many(&self, key: &String) -> Option<Vec<String>>;
}

pub struct ClapArgumentResolver {
    arg_matches: ArgMatches
}

impl ClapArgumentResolver {
    pub fn from_arg_matches(arg_matches: &ArgMatches) -> ClapArgumentResolver {
        return ClapArgumentResolver {
            arg_matches: arg_matches.clone()
        }
    }
}

impl ArgumentResolver for ClapArgumentResolver {
    fn get(&self, key: &String) -> Option<String> {
        if let Some(found_value) = self.arg_matches.get_one::<String>(key) {
            return Some(found_value.clone())
        }

        return None;
    }

    fn get_many(&self, key: &String) -> Option<Vec<String>> {
        if let Some(found_values) = self.arg_matches.get_many::<String>(key) {
            let mut values: Vec<String> = Vec::new();

            for found_value in found_values {
                values.push(found_value.clone());
            }

            return Some(values)
        }

        return None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{Arg, ArgAction, Command};

    #[test]
    fn argresolver_resolves_arg() {

        // Arrange
        let arg = arg_long(&"name".to_string());

        // Act
        let value = "Dingus";
        let matches = Command::new("dingus")
            .arg(arg)
            .get_matches_from(vec!["dingus", "--name", value]);

        let arg_resolver = ClapArgumentResolver::from_arg_matches(&matches);

        // Assert
        let found_value = arg_resolver.get(&"name".to_string());
        assert_eq!(found_value, Some(value.to_string()));
    }

    #[test]
    fn argresolver_resolves_arg_from_subcommand() {

        // Arrange
        let arg = arg_long(&"name".to_string());
        let greet_command = Command::new("greet")
            .arg(arg);

        let root_command = Command::new("gecko")
            .subcommand(greet_command);

        let value = "Dingus";
        let root_matches = root_command.get_matches_from(vec!["gecko", "greet", "--name", value]);
        let (subcommand_name, subcommand_matches) = root_matches.subcommand().unwrap();
        assert_eq!(subcommand_name, "greet");

        let arg_resolver = ClapArgumentResolver::from_arg_matches(&subcommand_matches);

        // Assert
        let found_value = arg_resolver.get(&"name".to_string());
        assert_eq!(found_value, Some(value.to_string()));
    }

    #[test]
    fn argresolver_resolves_multiple_args() {

        // Arrange
        let file_arg = many_arg(&"file".to_string());
        let print_command = Command::new("print")
            .arg(file_arg);

        let root_command = Command::new("gecko")
            .subcommand(print_command);

        // Act
        let file_name_1 = "first.txt";
        let file_name_2 = "second.txt";
        let root_matches = root_command.get_matches_from(vec!["gecko", "print", "--file", file_name_1, file_name_2]);
        let (subcommand_name, subcommand_matches) = root_matches.subcommand().unwrap();
        assert_eq!(subcommand_name, "print");

        let arg_resolver = ClapArgumentResolver::from_arg_matches(&subcommand_matches);

        // Assert
        let found_file_names = arg_resolver.get_many(&"file".to_string());
        assert_eq!(found_file_names, Some(vec!["first.txt".to_string(), "second.txt".to_string()]));
    }

    fn arg_long(name: &String) -> Arg {
        return Arg::new(name.clone())
            .long(name.clone())
            .action(ArgAction::Append);
    }

    fn many_arg(name: &String) -> Arg {
        return Arg::new(name.clone())
            .long(name.clone())
            .allow_hyphen_values(true)
            .action(ArgAction::Append)
            .num_args(0..)
    }
}