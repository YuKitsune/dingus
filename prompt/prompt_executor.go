package prompt

import (
	"bytes"
	"fmt"
	"github.com/charmbracelet/huh"
	"github.com/yukitsune/shiji/config"
	"github.com/yukitsune/shiji/execution"
	"os"
	"strings"
)

type PromptExecutor interface {
	Execute(promptDefinition *config.PromptDefinition) (any, error)
}

type executor struct {
	commandExecutor execution.CommandExecutor
}

func NewPromptExecutor(commandExecutor execution.CommandExecutor) PromptExecutor {
	return &executor{commandExecutor}
}

func (pe *executor) Execute(promptDefinition *config.PromptDefinition) (any, error) {

	if err := ensureMutualExclusivity(promptDefinition); err != nil {
		return nil, err
	}

	if promptDefinition.Text != nil {
		return executeTextPrompt(promptDefinition.Text)
	}

	if promptDefinition.Select != nil {
		return pe.executeSelectPrompt(promptDefinition.Select)
	}

	if promptDefinition.MultiSelect != nil {
		return pe.executeMultiSelectPrompt(promptDefinition.MultiSelect)
	}

	if promptDefinition.Confirm != nil {
		return executeConfirmPrompt(promptDefinition.Confirm)
	}

	return nil, fmt.Errorf("no prompts have been specified")
}

func ensureMutualExclusivity(promptDefinition *config.PromptDefinition) error {

	count := 0

	if promptDefinition.Text != nil {
		count++
	}

	if promptDefinition.Select != nil {
		count++
	}

	if promptDefinition.MultiSelect != nil {
		count++
	}

	if promptDefinition.Confirm != nil {
		count++
	}

	if count > 1 {
		return fmt.Errorf("only one prompt type can be specified")
	}

	if count == 0 {
		return fmt.Errorf("no prompts have been specified")
	}

	return nil
}

func executeTextPrompt(definition *config.TextPromptDefinition) (string, error) {
	var value string = definition.Default

	var err error
	if definition.MultiLine {
		err = huh.NewText().
			Title(definition.Description).
			Value(&value).
			Run()
	} else {
		err = huh.NewInput().
			Title(definition.Description).
			Prompt("?").
			Value(&value).
			Run()
	}

	if err != nil {
		return "", err
	}

	return value, nil
}

func (pe *executor) executeSelectPrompt(definition *config.SelectPromptDefinition) (string, error) {
	var value string

	options, err := pe.makeOptions(definition)
	if err != nil {
		return value, err
	}

	err = huh.NewSelect[string]().
		Title(definition.Description).
		Options(options...).
		Value(&value).
		Run()
	if err != nil {
		return value, err
	}

	return value, nil
}

func (pe *executor) executeMultiSelectPrompt(definition *config.SelectPromptDefinition) ([]string, error) {
	var values []string

	options, err := pe.makeOptions(definition)
	if err != nil {
		return nil, err
	}

	err = huh.NewMultiSelect[string]().
		Title(definition.Description).
		Options(options...).
		Value(&values).
		Run()
	if err != nil {
		return nil, err
	}

	return values, nil
}

func executeConfirmPrompt(definition *config.ConfirmPromptDefinition) (bool, error) {
	var value bool

	err := huh.NewConfirm().
		Title(definition.Description).
		Affirmative(definition.Affirmative).
		Negative(definition.Negative).
		Value(&value).
		Run()

	return value, err
}

func (pe *executor) makeOptions(definition *config.SelectPromptDefinition) ([]huh.Option[string], error) {

	var options []huh.Option[string]
	var err error
	if len(definition.Options) > 0 {
		for _, option := range definition.Options {
			options = append(options, huh.NewOption[string](option, option))
		}
	} else if definition.OptionsFrom != nil {
		options, err = pe.getPromptOptionsFromCommand(*definition.OptionsFrom)
	}

	return options, err
}

func (pe *executor) getPromptOptionsFromCommand(optionsCommand config.ExecutableCommand) ([]huh.Option[string], error) {

	stdoutBuffer := &bytes.Buffer{}
	stderrBuffer := &bytes.Buffer{}
	if err := pe.commandExecutor.Execute(optionsCommand, os.Stdin, stdoutBuffer, stderrBuffer); err != nil {
		return nil, err
	}

	errStr := stderrBuffer.String()
	if errStr != "" {
		return nil, fmt.Errorf("%s", errStr)
	}

	result := stdoutBuffer.String()
	trimmedResult := strings.TrimRight(result, "\n ")

	values := strings.Split(trimmedResult, "\n")
	var options []huh.Option[string]
	for _, value := range values {
		options = append(options, huh.NewOption(value, value))
	}

	return options, nil
}
