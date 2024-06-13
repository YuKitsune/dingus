---
sidebar_position: 1
---

<!-- 
TODO:
- Introduction: What is Gecko? What does it do? How can I benefit from it?
- Installation
- Configuration: Examples of each configuration scenario
- Contributing: Guidelines for making contributions
- Changelog
- Sponsorship
- Feature sections (Front page)
- Artwork
- Privacy (No tracking by gecko, disclaimer about tracking from package managers, Plausible tracking on docs page)
-->

# Introduction

Gecko is a versatile command-line application designed to streamline task automation through a simple YAML configuration file.
With Gecko, users can define top-level variables, create complex command workflows, and execute tasks locally or remotely via SSH.
Perfect for developers, system administrators, and power users, Gecko consolidates your scripts and commands into an easily manageable format, enhancing productivity and simplifying automation.

## Getting Started

Get started by **creating a YAML file**.

Or **try Docusaurus immediately** with **[docusaurus.new](https://docusaurus.new)**.

### What you'll need

- [Node.js](https://nodejs.org/en/download/) version 18.0 or above:
  - When installing Node.js, you are recommended to check all checkboxes related to dependencies.

## Generate a new site

Generate a new Docusaurus site using the **classic template**.

The classic template will automatically be added to your project after you run the command:

```bash
npm init docusaurus@latest my-website classic
```

You can type this command into Command Prompt, Powershell, Terminal, or any other integrated terminal of your code editor.

The command also installs all necessary dependencies you need to run Docusaurus.

## Start your site

Run the development server:

```bash
cd my-website
npm run start
```

The `cd` command changes the directory you're working with. In order to work with your newly created Docusaurus site, you'll need to navigate the terminal there.

The `npm run start` command builds your website locally and serves it through a development server, ready for you to view at http://localhost:3000/.

Open `docs/intro.md` (this page) and edit some lines: the site **reloads automatically** and displays your changes.
