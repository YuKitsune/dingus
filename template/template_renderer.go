package template

import (
	"bytes"
	"github.com/yukitsune/shiji/config"
	"github.com/yukitsune/shiji/variables"
	"text/template"
)

type Renderer interface {
	RenderTemplate(templateString config.TemplateString, variables variables.Variables) (RenderedString, error)
}

func NewRenderer() Renderer {
	return &simpleRenderer{}
}

type simpleRenderer struct{}

func (t *simpleRenderer) RenderTemplate(templateString config.TemplateString, variables variables.Variables) (RenderedString, error) {

	tmpl, err := template.New("Shiji Variables").Parse(templateString.String())
	if err != nil {
		return "", err
	}

	buffer := new(bytes.Buffer)
	err = tmpl.Execute(buffer, variables)
	if err != nil {
		return "", err
	}

	return RenderedString(buffer.String()), nil
}
