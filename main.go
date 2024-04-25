package main

import (
	"fmt"
	"github.com/spf13/cobra"
	"github.com/yukitsune/shiji/config"
	"github.com/yukitsune/shiji/execution"
	"github.com/yukitsune/shiji/prompt"
	"github.com/yukitsune/shiji/template"
	"github.com/yukitsune/shiji/variables"
	"gopkg.in/yaml.v3"
	"os"
)

func main() {

	var cfg *config.Config
	var err error
	if cfg, err = getConfig(); err != nil {
		panic(fmt.Errorf("failed to get config: %v", err))
	}

	commandExecutor := execution.NewBashExecutor() // TODO: Support other shells
	promptExecutor := prompt.NewPromptExecutor(commandExecutor)
	variableProvider := variables.NewVariableProvider(cfg, commandExecutor, promptExecutor)
	templateRenderer := template.NewRenderer()

	rootCmd := &cobra.Command{
		Use:   "shiji",
		Short: cfg.Description,
	}

	for key, commandDefinition := range cfg.Commands {
		rootCmd.AddCommand(createCobraCommand(key, &commandDefinition, variableProvider, templateRenderer, commandExecutor))
	}

	bindVariablesToCommand(cfg.Variables, rootCmd, true)

	if err = rootCmd.Execute(); err != nil {
		panic(err)
	}
}

func getConfig() (*config.Config, error) {

	yamlFile, err := os.ReadFile("example.yaml")
	if err != nil {
		return nil, err
	}

	var cfg *config.Config
	err = yaml.Unmarshal(yamlFile, &cfg)
	if err != nil {
		return nil, err
	}

	return cfg, nil
}

func createCobraCommand(name string, commandDefinition *config.CommandDefinition, variableProvider variables.VariableProvider, templateRenderer template.Renderer, executor execution.CommandExecutor) *cobra.Command {

	cobraCommand := &cobra.Command{
		Use:     name,
		Short:   commandDefinition.Description,
		Aliases: commandDefinition.Alias,
		RunE: func(cmd *cobra.Command, args []string) error {

			// TODO: Extract this function

			flagProvider := variables.NewFlagProviderFromCommand(cmd)
			commandVariables, err := variableProvider.GetVariablesFor(commandDefinition, flagProvider)
			if err != nil {
				return err
			}

			renderedTemplate, err := templateRenderer.RenderTemplate(commandDefinition.Execute, commandVariables)
			if err != nil {
				return err
			}

			return executor.Execute(renderedTemplate.Executable(), os.Stdin, os.Stdout, os.Stderr)
		},
	}

	for key, subCommand := range commandDefinition.Commands {
		cobraCommand.AddCommand(createCobraCommand(key, &subCommand, variableProvider, templateRenderer, executor))
	}

	bindVariablesToCommand(commandDefinition.Variables, cobraCommand, false)

	return cobraCommand
}

func bindVariablesToCommand(variableDefinitions map[string]config.VariableDefinition, command *cobra.Command, persistent bool) {

	// Bind the variables to flags on the cobra command
	for key, variable := range variableDefinitions {

		// Use the key if a custom flag has not been specified
		flagName := key
		if len(variable.Flag) > 0 {
			flagName = variable.Flag
		}

		// TODO: Other data types
		if persistent {
			command.PersistentFlags().String(flagName, variables.UnsetFlagSentinel, variable.Description)
		} else {
			command.Flags().String(flagName, variables.UnsetFlagSentinel, variable.Description)
		}
	}
}
