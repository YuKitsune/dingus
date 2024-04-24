package variables

import "github.com/spf13/cobra"

// TODO: This currently only supports string values. Refactor it to support any value

const UnsetFlagSentinel = "SHIJI_UNSET_FLAG"

type FlagProvider interface {
	GetFlagValue(key string) (string, bool)
}

type cobraFlagProvider struct {
	command *cobra.Command
}

func NewFlagProviderFromCommand(cmd *cobra.Command) FlagProvider {
	return &cobraFlagProvider{cmd}
}

func (p *cobraFlagProvider) GetFlagValue(key string) (string, bool) {

	flag := p.command.Flags().Lookup(key)
	if flag == nil {
		return "", false
	}

	value := flag.Value.String()
	if value == UnsetFlagSentinel {
		return "", false
	}

	return value, true
}
