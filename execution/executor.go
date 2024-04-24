package execution

import (
	"github.com/yukitsune/shiji/config"
	"io"
	"os/exec"
)

type CommandExecutor interface {
	Execute(command config.ExecutableCommand, stdin io.Reader, stdout io.Writer, stderr io.Writer) error
}

type bashExecutor struct{}

func NewBashExecutor() CommandExecutor {
	return &bashExecutor{}
}

func (e *bashExecutor) Execute(command config.ExecutableCommand, stdin io.Reader, stdout io.Writer, stderr io.Writer) error {

	bashCommand := exec.Command("bash", "-c", command.String())
	bashCommand.Stdin = stdin
	bashCommand.Stdout = stdout
	bashCommand.Stderr = stderr

	if err := bashCommand.Run(); err != nil {
		return err
	}

	return nil
}
