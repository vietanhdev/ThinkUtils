# AI Integration (MCP Server)

ThinkUtils includes a built-in MCP (Model Context Protocol) server that exposes system controls to AI assistants.

![AI Integration](/screenshots/ai_integration.png)

## What is MCP?

[Model Context Protocol](https://modelcontextprotocol.io) is a standard protocol that lets AI assistants interact with external tools. ThinkUtils implements an MCP server so AI tools can monitor and control your ThinkPad settings.

## Supported AI Tools

- **Claude Code** (CLI)
- **Claude Desktop**
- Other MCP-compatible AI assistants

## Features

The MCP server exposes ThinkUtils functionality as tools that AI assistants can call:

- Read system stats (CPU, memory, temperature)
- Query battery status and health
- Get fan speed and mode
- View and change performance settings

## Setup

Configure your AI tool to connect to the ThinkUtils MCP server. The MCP settings page in the app shows connection details and lets you manage the server.

See the app's MCP page for specific configuration instructions for each supported AI tool.
