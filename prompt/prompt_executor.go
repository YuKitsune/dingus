package prompt

import (
	"fmt"
	"github.com/charmbracelet/huh"
	"github.com/yukitsune/shiji/config"
)

type PromptExecutor interface {
	Execute(promptDefinition *config.PromptDefinition) (any, error)
}

type executor struct{}

func NewPromptExecutor() PromptExecutor {
	return &executor{}
}

func (pe *executor) Execute(promptDefinition *config.PromptDefinition) (any, error) {

	if err := ensureMutualExclusivity(promptDefinition); err != nil {
		return nil, err
	}

	if promptDefinition.Text != nil {
		return executeTextPrompt(promptDefinition.Text)
	}

	if promptDefinition.Select != nil {
		return executeSelectPrompt(promptDefinition.Select)
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

func executeSelectPrompt(definition *config.SelectPromptDefinition) ([]string, error) {
	var values []string

	var err error
	if definition.Multiple {
		err = huh.NewMultiSelect[string]().
			Title(definition.Description).
			Options(makeOptions(definition)...).
			Value(&values).
			Run()
	} else {
		var value string
		err = huh.NewSelect[string]().
			Title(definition.Description).
			Options(makeOptions(definition)...).
			Value(&value).
			Run()
		values = append(values, value)
	}

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

func makeOptions(definition *config.SelectPromptDefinition) []huh.Option[string] {
	var options []huh.Option[string]
	for _, option := range definition.Options {
		options = append(options, huh.NewOption[string](option, option))
	}

	return options
}
