package config

type TemplateString string

type ExecutableCommand string

func (ec ExecutableCommand) String() string {
	return string(ec)
}

func (ts TemplateString) String() string {
	return string(ts)
}

type Config struct {
	Description string
	Variables   map[string]VariableDefinition
	Commands    map[string]CommandDefinition
}

type CommandDefinition struct {
	Alias       []string
	Description string
	Execute     TemplateString
	Commands    map[string]CommandDefinition
	Variables   map[string]VariableDefinition
}

type VariableDefinition struct {
	Description string
	Value       any
	ValueFrom   *ExecutableCommand `yaml:"valueFrom"`
	Flag        string
	Prompt      *PromptDefinition
	Required    bool
}

type PromptDefinition struct {
	Text    *TextPromptDefinition
	Select  *SelectPromptDefinition
	Confirm *ConfirmPromptDefinition
}

type TextPromptDefinition struct {
	Description string
	Default     string
	MultiLine   bool
}

type SelectPromptDefinition struct {
	Description string
	Options     []string
	OptionsFrom *ExecutableCommand
	Multiple    bool // Todo: Consider splitting Multi-select into it's own thing
}

type ConfirmPromptDefinition struct {
	Description string
	Affirmative string
	Negative    string
}
