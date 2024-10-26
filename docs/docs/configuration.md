---
sidebar_position: 3
---

# Configuration

## Dingus file

When executing Dingus, it will look in the current directory a `dingus.yaml` file.
If there is no file in the current directory, it will check all parent directories until it finds one.

Once a config file has been found, Dingus will use that files location as it's working directory.
This allows you to reference files from the config file using relative paths.

```sh
$ ls
docs    license   dingus.yaml

$ cat dingus.yaml
commands:
  print-license:
    action: cat license

$ cat license
MIT

$ cd docs

$ dingus print-license
MIT
```

## Variables

Variables are exposed to [commands](#commands) as environment variables.

Variables can be defined at the root-level, or on a specific command.
Variables defined at the root-level are available to all commands defined in the file.
Variables defined on a specific command are available to that command and any of it's subcommands.

```yaml
# Root-level variables
variables:
    name: Dingus

commands:
    greet:

        # Command variables
        variables:
            age: 42
        action: echo "Hello $name, you are $age years old"
```

### Environment Variables

By default, variables are exposed to commands as environment variables with the same name as the variable, so a variable called `name` can be read using the `$name` environment variable.
The optional `environment_variable` field can be used to change the name of the environment variable exposed to the command.

```yaml
commands:
    deploy:

        # Command variables
        variables:
            host:
                exec: cat infra.yaml | yq '.prod.host'
                environment_variable: DOCKER_HOST
        action: docker compose up -d
```

### Command-Line Arguments

Variable values can be provided using command-line arguments.
Use the `argument` field to control how the command-line argument is generated for a variable.
Here, a long name can be specified, along with an optional short name and description.
The short name can only be one character long.

```yaml
variables:
  name:
    value: Dingus
    argument:
      description: The name of the user to greet
      name: user
      short: n

  age:
    value: 42
    argument:
      desc: The age of the user to greet
      name: age

commands:
  greet:
    description: Greet the user
    action: echo "Hello, $name! You are $age years old."
```

```
$ dingus --help
Usage: dingus [OPTIONS] <COMMAND>

Commands:
  greet    Greet the user
  version  Shows version information
  help     Print this message or the help of the given subcommand(s)

Options:
  -n  --user <user>  The name of the user to greet [default: Dingus]
      --age <age>    The age of the user to greet [default: 42]
  -h, --help         Print help
```

The `argument` field also accepts a string if a short name and description are not necessary.

```yaml
variables:
  name:
    value: Dingus
    argument: user
    
commands:
  greet:
    description: Greet the user
    action: echo "Hello, $name!"
```

```
$ dingus --help
Usage: dingus [OPTIONS] <COMMAND>

Commands:
  greet    Greet the user
  version  Shows version information
  help     Print this message or the help of the given subcommand(s)

Options:
      --user <user>  [default: Dingus]
  -h, --help         Print help
```

Positional arguments can also be configured using the `position` field. This will set the position of the argument starting from `1`.
When the `position` field is used, the `short` field is ignored.

```yaml
commands:
  deploy:
    variables:
      nodes:
        value: "5"
        arg: nodes

      environment:
        value: Production
        arg:
          name: environment
          position: 1

    action: ...
```

```
Usage: dingus deploy [OPTIONS] [environment]

Arguments:
  [environment]  [default: Production]

Options:
      --nodes <nodes>  [default: 5]
  -h, --help           Print help
```

### Literal Variables

Literal variables are ones where the value is hard-coded to a specific value.
This value can still be overridden using its relevant command-line argument.

```yaml
variables:
    name:
        value: dingus
        argument: user
```

#### Shorthand

If the `argument` field is not required, literal variables can be shortened to `key: value`.

```yaml
variables:
    name: dingus
```

### Execution Variables

Execution variables will be assigned a value at runtime based on the output of a command.

In this example, the `secret` variable will execute `cat secret.txt` and use the output of that command (the contents of `secret.txt` in this case) as it's value.

```yaml
variables:
    secret:
        execute: cat secret.txt
```

Execution variables also have access to all of the variable values defined **above** them.

```yaml
variables:
    environment: Development
    environment_config:
        execute: cat ./$environment/config.yaml"
```

:::info
If the command-line argument for the variable has been specified, then the command will not be executed, and the variable will use the value provided via the command line.
:::

### Prompt Variables

Prompt variables will be assigned a value provided by the user at runtime.

In this example, Dingus will prompt the user with `What's your name?` and assign the users input to the `name` variable.

```yaml
variables:
    name:
        prompt:
            message: What's your name?
```

If the `options` field is specified, then a select-style prompt will be shown where the user can select from a list of options.

```yaml
variables:
    environment:
        prompt:
            message: Which environment are you deploying to?
            options:
                - Development
                - Staging
                - Production
```

The list of options can also be sourced from the output of a command.

```yaml
variables:
    user:
        prompt:
            message: Which user do you want to delete?
            options:
                execute: ls /usr/
```

:::info
If the command-line argument for the variable has been specified, then no prompt will be shown, and the variable will use the value provided via the command line.
:::

## Commands

Commands are the things that the user can execute.

```yaml
commands:
    greet:
        action: echo "Hello!"
```

Commands can contain zero or more subcommands.

```yaml
commands:

    build:
        desc: Parent command for grouping build-related commands

        commands:

            backend:
                desc: Builds the backend
                action: ...

            frontend:
                desc: Builds the frontend
                action: ...
```

The configuration above would result in a top-level `build` command with two subcommands.

```sh
$ dingus build --help
Usage: dingus build <COMMAND>

Commands:
  frontend  Builds the frontend
  backend   Builds the backend
  help      Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help
```

Because the `build` command doesn't have it's own action, `dingus build` cannot be executed on it's own.

:::note
If a command does not have any actions, then it **must** have at least one subcommand.
:::

### Actions

Actions are the actual commands that will be executed.

Commands can execute a single action using the `action` field, or multiple actions using the `actions` field.
These fields are mutally exclusive.

```yaml
commands:
    build:
        action: cargo build
    
    test:
        actions:
            - docker compose up -d ./docker-compose.deps.yaml 
            - cargo test
            - docker compose down -d ./docker-compose.deps.yaml
```

### Aliases

Aliases are similar to commands, but behave more like a traditional shell alias.

In this example, executing `dingus deps` is an alias for `docker compose --file ./docker-compose.deps.yaml`.
Anything after `dingus deps` will be appended to the end of the target command, just like a traditional shell alias.

```sh
$ cat dingus.yaml
commands:
    deps:
        alias: docker compose --file ./docker-compose.deps.yaml

$ dingus deps up -d
[+] Running 2/2
 ✔ Container rabbitmq  Started
 ✔ Container postgres  Started
```

### Platform-specific Commands

The `platform` field can be used to restrict a command to specific platforms.
Multiple platforms can be specified using the `platforms` field. These fields are mutually exclusive.
This is useful when the same command needs to execute something different depending on the platform.

```yaml
commands:
    build-win:
        name: build
        platform: Windows

    build-nix:
        name: build
        platforms:
            - MacOS
            - Linux
```

When the `platform` (or `platforms`) field is specified, then the command will only be available on the specified platforms.
If the current platform is not one of the specified platforms, then Dingus will ignore the command.

:::note
By default, Dingus will use the key to determine the command name.
Each key in the `commands` map must be unique.
If you want your command to have the same name across different platforms, use the `name` field to provide an alternative name.
:::

### Running other commands

Commands can run other commands defined in the file.
There is no special syntax for this, just call `dingus` with the desired command the same way you would invoke a command normally.

```yaml
commands:
    clean:
        action: rm -rf /build

    build:
        action: ./build.sh
    
    rebuild:
        actions:
            - dingus clean
            - dingus build
```

If the command you need to call is not intented to be used directly, use the `hidden` field to hide it from the help output.

```yaml
commands:
    pre:
        hidden: true
        action: ./pre-flight-checks.sh

    start:
        actions:
            - dingus pre
            - ./start.sh

    debug:
        actions:
            - dingus pre
            - ./debug.sh
```

:::note
When a command is hidden, it is only removed from the help output, and any completeions. It can still be executed normally.
:::

## Execution

[Execution variables](#execution-variables), [prompt variable](#prompt-variables) options, and [actions](#actions) all provide a field for command text to be specified.
This command is the real command that will be executed.

By default, Dingus will execute these commands directly **without a shell**. This is referred to internally as a "raw execution".
For raw executions, Dingus will perform Bash-like variable substitution against the command text, as well as injecting all of the variables as environment variables so that the process can read them at runtime. This variable substitution is handled by the individual shells for shell executions.

Because raw executions do not rely on a shell, **they do not have access to shell-specific features**.

If you need to use a specific shell, use the `bash` or `sh` field within the execution definition.
Below are some examples of raw executions vs. bash executions.

```yaml
variables:
    raw_name:
        execute: cat example.txt
    
    bash_name:
        execute:
            sh: cat example.json | jq -r '.name'

    raw_option:
        prompt: 
            message: Pick one
            options:
                execute: cat example.txt
    
    bash_option:
        prompt:
            message: Pick one
            options:
                execute:
                    sh: cat example.json | jq -r '.options[]'

commands:
    raw_example:
        desc: Example command using a raw execution
        action: echo Hello $raw_name
    
    raw_example_multi:
        desc: Example command using a raw execution
        actions:
            - echo Hello $raw_name
            - echo Goodbye $raw_name

    bash_example:
        desc: Example command using a bash command
        action:
            sh: echo "Hello, $(cat example.json | jq -r '.name')"

    bash_example_multi:
        desc: Example command using a bash command
        actions:
            - sh: echo "Hello, $(cat example.json | jq -r '.name')"
            - sh: echo "Goodbye, $(cat example.json | jq -r '.name')"
```

:::note
Only support for raw and Bash executions are supported. Other shells will be added at a later date.
:::

### Working Directories

By default, commands are executed in the current working directory.
This can be overridden using the `workdir` field.

```yaml
# Raw execution
execute:
    workdir: ./config/
    command: ...

# Bash execution
execute:
    workdir: ./config/
    bash: ...
```

This also works on Actions.

```yaml
actions:

    # Raw execution
    - workdir: ./config/
      execute: ...

    # Bash execution
    - workdir: ./config/
      bash: ...
```

## Logging

By default, Dingus will only output errors or the output from the commands being executed.

Variables can be printed before execution by setting the `options.print_variables` field to `true`, or by setting the
`DINGUS_PRINT_VARIABLES` environment variable to `true`.

```yaml
options:
  print_variables: true
```

The command text can also be printed before being executed by setting the `options.print_commands` field to `true`, or
by setting the `DINGUS_PRINT_COMMANDS` environment variable to `true`.

```yaml
options:
  print_commands: true
```

## Imports

Additional config files can be imported using the `imports` field. Importing a config file effectively creates a new 
subcommand with the description, variables, and subcommands from the provided file.

Imports require an `alias`, and a `source`. The `alias` is used to set the name of the subcommand, and the `source` is 
the path to the file to import.

For example, the following config will import all commands and variables defined in the `./docs/dingus.yaml` file into a
subcommand called `docs`.

```yaml
imports:
  - alias: docs
    source: ./docs/dingus.yaml
```

Imported files can be hidden from the help output, or restricted to specific platforms just like normal commands.

```yaml
imports:
  - alias: utils
    source: ./utils/dingus.yaml
    hidden: true

  - alias: packages
    source: ./packages/nix.dingus.yaml
    platform:
      - MacOS
      - Linux
    
  - alias: packages
    source: ./packages/windows.dingus.yaml
    platform: Windows
```

The `alias` field does not need to be unique, so long as the other imports using the same alias are restricted to
another platform. 

## Shortenings

Many fields have an alternative, shorter name.
Here is a list of the available shortenings:

| Field Name             | Alias  |
|------------------------|--------|
| `variables`            | `vars` |
| `commands`             | `cmds` |
| `description`          | `desc` |
| `argument`             | `arg`  |
| `environment_variable` | `env`  |
| `options`              | `opts` |
| `execute`              | `exec` |
| `command`              | `cmd`  |
| `bash`                 | `sh`   |
| `workdir`              | `wd`   |