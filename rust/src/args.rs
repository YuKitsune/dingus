use std::collections::HashMap;
use clap::ArgMatches;

pub trait ArgumentResolver {
    fn get(&self, key: &String) -> Option<String>;
}

pub struct ClapArgumentResolver {
    args: HashMap<String, String>
}

impl ClapArgumentResolver {
    pub fn from_arg_matches(arg_matches: &ArgMatches) -> ClapArgumentResolver {
        let ids = arg_matches.ids();
        let mut args = HashMap::new();
        for id in ids {
            if let Some(value) = arg_matches.get_one::<String>(id.as_str()) {
                args.insert(id.to_string(), value.clone());
            }
        }

        return ClapArgumentResolver {args}
    }
}

impl ArgumentResolver for ClapArgumentResolver {
    fn get(&self, key: &String) -> Option<String> {
        if let Some(value) = self.args.get(key) {
            return Some(value.clone());
        }

        return None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{Arg, Command};

    #[test]
    fn argresolver_resolves_arg() {

        // Arrange
        let arg = arg_long(&"name".to_string());

        // Act
        let value = "Dingus";
        let matches = Command::new("shiji")
            .arg(arg)
            .get_matches_from(vec!["shiji", "--name", value]);

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

        let root_command = Command::new("shiji")
            .subcommand(greet_command);

        let value = "Dingus";
        let root_matches = root_command.get_matches_from(vec!["shiji", "greet", "--name", value]);
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
        let name_arg = arg_long(&"name".to_string());
        let age_arg = arg_long(&"age".to_string());
        let greet_command = Command::new("greet")
            .arg(name_arg)
            .arg(age_arg);

        let root_command = Command::new("shiji")
            .subcommand(greet_command);

        // Act
        let name_value = "Dingus";
        let age_value = "42";
        let root_matches = root_command.get_matches_from(vec!["shiji", "greet", "--name", name_value, "--age", age_value]);
        let (subcommand_name, subcommand_matches) = root_matches.subcommand().unwrap();
        assert_eq!(subcommand_name, "greet");

        let arg_resolver = ClapArgumentResolver::from_arg_matches(&subcommand_matches);

        // Assert
        let found_name_value = arg_resolver.get(&"name".to_string());
        assert_eq!(found_name_value, Some(name_value.to_string()));

        let found_age_value = arg_resolver.get(&"age".to_string());
        assert_eq!(found_age_value, Some(age_value.to_string()));
    }

    fn arg_long(name: &String) -> Arg {
        return Arg::new(name.clone())
            .long(name.clone());
    }
}