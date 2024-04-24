package template

import "github.com/yukitsune/shiji/config"

type RenderedString string

func (rs RenderedString) String() string {
	return string(rs)
}

func (rs RenderedString) Executable() config.ExecutableCommand {
	return config.ExecutableCommand(rs)
}
