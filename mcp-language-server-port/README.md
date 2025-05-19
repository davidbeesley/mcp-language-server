# MCP Language Server (Rust)

A Rust implementation of the [MCP Language Server](https://github.com/isaacphi/mcp-language-server) originally created by Phil Isaac.

## Overview

This is an [MCP](https://modelcontextprotocol.io/introduction) server that runs and exposes a [language server](https://microsoft.github.io/language-server-protocol/) to LLMs. Not a language server for MCP, whatever that would be.

The server helps MCP-enabled clients navigate codebases more easily by providing access to semantic tools like definition lookup, references, rename, diagnostics, and more.

## Demo

TODO: ACTUAL DEMO
`mcp-language-server` helps MCP enabled clients navigate codebases more easily by giving them access semantic tools like get definition, references, rename, and diagnostics.

## Tools

- `definition`: Retrieves the complete source code definition of any symbol (function, type, constant, etc.)
- `references`: Locates all usages and references of a symbol throughout the codebase
- `diagnostics`: Provides diagnostic information for a specific file, including warnings and errors
- `hover`: Display documentation, type hints, or other hover information for a given location
- `rename_symbol`: Rename a symbol across a project
- `edit_file`: Allows making multiple text edits to a file based on line numbers

## Setup

1. **Install Rust**: Follow instructions at https://www.rust-lang.org/tools/install
2. **Build this server**: `cargo build --release`
3. **Install a language server**: Install a language server like gopls, rust-analyzer, pyright, typescript-language-server, or clangd
4. **Configure your MCP client**: Add the server to your MCP client configuration, pointing to this binary


## Logging

Setting the `LOG_LEVEL` environment variable to DEBUG enables verbose logging to stderr.

## About

This is a Rust implementation of the [MCP Language Server](https://github.com/isaacphi/mcp-language-server) originally created by Phil Isaac. The original repository and this port are both covered by a permissive BSD-style license.

This is beta software. Please create an issue if you run into any problems or have suggestions.

## Contributing

Contributions are welcome. Please keep PRs small and open issues first for substantial changes.
