package variables

import (
	"bytes"
	"fmt"
	"github.com/yukitsune/shiji/config"
	"github.com/yukitsune/shiji/execution"
	"github.com/yukitsune/shiji/prompt"
	"os"
	"strings"
)

type Variables map[string]any

type VariableProvider interface {
	GetVariablesFor(commandDefinition *config.CommandDefinition, provider FlagProvider) (Variables, error)
}

type variableProvider struct {
	config          *config.Config
	commandExecutor execution.CommandExecutor
	promptExecutor  prompt.PromptExecutor
}

func NewVariableProvider(config *config.Config, commandExecutor execution.CommandExecutor, promptExecutor prompt.PromptExecutor) VariableProvider {
	return &variableProvider{config, commandExecutor, promptExecutor}
}

func (vp *variableProvider) GetVariablesFor(commandDefinition *config.CommandDefinition, provider FlagProvider) (Variables, error) {

	variables := make(map[string]any)
	for key, variable := range vp.config.Variables {
		result, err := vp.getVariableValue(key, &variable, provider)
		if err != nil {
			return nil, err
		}

		variables[key] = result
	}

	// TODO: Support inherited variables
	for key, variable := range commandDefinition.Variables {
		result, err := vp.getVariableValue(key, &variable, provider)
		if err != nil {
			return nil, err
		}

		variables[key] = result
	}

	return variables, nil
}

func (vp *variableProvider) getVariableValue(name string, variableDefinition *config.VariableDefinition, flagProvider FlagProvider) (any, error) {

	// Command-line flags have the highest priority
	if flagValue, ok := flagProvider.GetFlagValue(name); ok {
		return flagValue, nil
	}

	if variableDefinition.Value != nil {
		return variableDefinition.Value, nil
	}

	if variableDefinition.ValueFrom != nil {
		return getVariableValueFromCommand(*variableDefinition.ValueFrom, vp.commandExecutor)
	}

	if variableDefinition.Prompt != nil {
		return vp.promptExecutor.Execute(variableDefinition.Prompt)
	}

	if !variableDefinition.Required {
		return nil, nil
	}

	return nil, fmt.Errorf("variable %s is required", name)
}

func getVariableValueFromCommand(variableCommand config.ExecutableCommand, executor execution.CommandExecutor) (string, error) {

	stdoutBuffer := &bytes.Buffer{}
	stderrBuffer := &bytes.Buffer{}
	if err := executor.Execute(variableCommand, os.Stdin, stdoutBuffer, stderrBuffer); err != nil {
		return "", err
	}

	errStr := stderrBuffer.String()
	if errStr != "" {
		return "", fmt.Errorf("%s", errStr)
	}

	value := stdoutBuffer.String()
	trimmedValue := strings.TrimRight(value, "\n ")

	return trimmedValue, nil
}
