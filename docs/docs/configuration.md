---
sidebar_position: 3
---

# Configuration

## Variables

Variables are made available to [commands](#commands) as environment variables.

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

### Command-Line Arguments

All variable values can be overridden using command-line arguments.
The name of the argument will be the same as the variable name, so a variable called `name` can be overridden using the `--name` argument.

The optional `argument` and `description` fields can be used to control how the command-line argument is generated for a specific variable.
The name of the command-line argument can be overridden using the `argument` field.
The `description` field can be used to provide help text for the help output.
Both of these fields are available on all variable types.

```yaml
variables:
  name:
    value: Dingus
    description: The name of the user to greet
    argument: user
  
  age:
    value: 42
    desc: The age of the user to greet

commands:
  greet:
    description: Greet the user
    action: echo "Hello, $name! You are $age years old."
```

```
$ gecko --help
Usage: gecko [OPTIONS] <COMMAND>

Commands:
  greet    Greet the user
  version  Shows version information
  help     Print this message or the help of the given subcommand(s)

Options:
      --user <user>  The name of the user to greet [default: Dingus]
      --age <age>    The age of the user to greet [default: 42]
  -h, --help         Print help
```

### Literal Variables

Literal variables are ones where the value is hard-coded to a specific value.
This value can still be overridden using it's relevent command-line argument.

```yaml
variables:
    name:
        value: dingus
        description: The name of the user to use
        argument: user
```

#### Shorthand

If a description and argument name are not required, literal variables can be shortened to `key: value`.

```yaml
variables:
    name: dingus
```

### Execution Variables

Execution variabes will be assigned a value at runtime based on the output of a command.

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
$ gecko build --help
Usage: gecko build <COMMAND>

Commands:
  frontend  Builds the frontend
  backend   Builds the backend
  help      Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help
```

Because the `build` command doesn't have it's own action, `gecko build` cannot be executed on it's own.

:::note
If a command does not have any actions, then it **must** have at least one subcommand.
:::

### Actions 

Actions define the actual commands that will be executed.

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

Commands can also serve as an alias for a real command. This is similar to aliases in Bash and ZSH.

In this example, executing `gecko deps` will act as an alias for `docker compose --file ./docker-compose.deps.yaml`.
Anything after `gecko deps` will be appended to the end of the target command.

```yaml
commands:
    deps:
        alias: docker compose --file ./docker-compose.deps.yaml
```

## Execution

[Execution variables](#execution-variables), [prompt variable](#prompt-variables) options, and [actions](#actions) all provide a field for command text to be specified. This command is the real command that will be executed.

By default, Dingus will execute these commands directly **without a shell**. This is referred to internally as a "raw execution".
Dingus will perform Bash-like variable substitution against the command text, as well as injecting all of the variables as environment variables so that the process can read them at runtime. Because raw executions do not rely on a shell, **they do not have access to shell-specific features**.

If you need to use a specific shell, use the `bash` or `sh` field within the execution definition.
Below are some examples of raw executions vs. bash executions.

```yaml
variables:
    raw_name:
        desc: Execution variable using a raw execution
        execute: cat example.txt
    
    bash_name:
        desc: Execution variable using bash
        execute:
            sh: cat example.json | jq -r '.name'

    raw_option:
        desc: Prompt options sourced from a raw execution
        prompt: 
            message: Pick one
            options:
                execute: cat example.txt
    
    bash_option:
        desc: Prompt options sourced using a bash command
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
Only support for raw and Bash execution is supported. Other shells will be added at a later date.
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

## Shortenings

Many fields have an alternative, shorter name.
Here is a list of the available shortenings:

| Field Name | Alias |
| ---------- | ----- |
| `variables` | `vars` |
| `commands` | `cmds` |
| `description` | `desc` |
| `argument` | `arg` |
| `options` | `opts` |
| `argument` | `arg` |
| `execute` | `exec` |
| `command` | `cmd` |
| `bash` | `sh` |
| `workdir` | `wd` |