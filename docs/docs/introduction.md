---
sidebar_position: 1
sidebar_label: Introduction
---

<!-- 
TODO:
- Introduction: What is Dingus? What does it do? How can I benefit from it?
- Installation
- Configuration: Examples of each configuration scenario
- Contributing: Guidelines for making contributions
- Changelog
- Sponsorship
- Feature sections (Front page)
- Artwork
- Privacy (No tracking by dingus, disclaimer about tracking from package managers, Plausible tracking on docs page)
-->

# Introduction

Dingus is a dead-simple task runner.

## Features

- Designed from the ground-up as a command runner without the constraints of a build tool.
- Supports Windows, macOS, and Linux, and isn't dependant on a specific Shell.
- Provides a POSIX-style command-line interface allowing for variable substitution using command-line arguments.
- Uses a simple YAML file for configuration.
- Additional configuration files can be included from the local file system or from a URL (Coming soon.)
- Allows for commands to be executed on remote machines (Coming soon.)

## Overview

:::warning
Dingus is still in early development and it's configuration syntax and usage are subject to change.
:::

Dingus relies on YAML for its configuration.

In your `Dingus.yaml`, you can define variables at the root level.
These variables are global, so they're available to all commands and subcommands throughout the file.

Example:
```yaml
variables:
  name: Godzilla

commands:
  greet:
    action: echo Hello, $name!

  pet:
    action: echo You have petted $name!
```

```sh
$ dingus greet
Hello, Godzilla!

$ dingus pet
You have petted Godzilla!
```

You can also define variables within commands.
These variables are available to the command and its subcommands.

```yaml
description: Example Dingus configuration

commands:
  greet:
    variables:
      name: Godzilla
    action: echo Hello, $name!

  pet:
    variables:
      name: Maxwell
    action: echo You have petted $name!
```

```sh
$ dingus greet
Hello, Godzilla!

$ dingus pet
You have petted Maxwell!
```

Actions represent the actual commands that get executed.
When you call a command, its actions are run in sequence.

```yaml
description: Example Dingus configuration

commands:
  greet:
    variables:
      name: Godzilla

    # Single action
    action: echo Hello, $name!

  pet:
    variables:
      name: Maxwell

    # Multiple actions
    actions:
      - echo Petting $name...
      - sleep 5
      - echo You have petted $name!
```

```sh
$ dingus greet
Hello, Godzilla!

$ dingus pet
Petting...
You have petted Maxwell!
```

## Use Cases

Dingus can be used in a variety of ways, but it's primary use is for centralising project-specific scripts, and making them more discoverable.
