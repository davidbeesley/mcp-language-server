# Testing Guide for MCP Language Server

This document describes the testing approach and architecture used for the MCP Language Server project.

## Test Architecture

The test suite for MCP Language Server follows these principles:

1. **Integration Testing**: Most tests are integration tests that verify the functionality of entire components rather than isolated units.
2. **Mocking**: We use mock implementations for LSP servers to avoid dependencies on actual language servers.
3. **Temporary Environments**: Tests create temporary workspaces and files to ensure isolation.
4. **Serial Execution**: Tests are marked with `#[serial]` to prevent race conditions between tests.

## Key Test Components

### Mock LSP Server

`mock_lsp_server.rs` provides a mock implementation of an LSP server that:

- Receives and responds to LSP protocol messages
- Can be configured to return predefined responses
- Records received messages for verification
- Can simulate LSP notifications like diagnostics

### Test Setup

Most tests follow this pattern:

1. Create a temporary workspace directory
2. Add test files to the workspace
3. Start a mock LSP server
4. Initialize an LSP client connected to the mock server
5. Perform operations and verify results
6. Clean up resources

## Test Categories

The test suite includes:

### Client Initialization Tests
- Tests for establishing the LSP client connection
- Verifying proper initialization and shutdown sequences

### File Operation Tests
- Opening, modifying, and closing files
- Verifying LSP notifications are sent correctly

### LSP Feature Tests
- Testing diagnostic reporting
- Testing hover information
- Testing text editing capabilities

### MCP Integration Tests
- Testing the MCP server's tool implementations
- Verifying proper communication with the LSP client

### Watcher Tests
- Testing file system watching functionality
- Verifying gitignore filtering behavior

### Full Integration Tests
- End-to-end tests combining all components
- Verifying the entire system works together

## Running Tests

```bash
# Run all tests
cargo test

# Run with logging
RUST_LOG=debug cargo test

# Run specific test
cargo test test_client_initialization
```

## Adding New Tests

When adding new tests:

1. For component-specific tests, add to the appropriate test file
2. For new feature tests, create a dedicated test file
3. Always use temporary directories for file operations
4. Clean up resources in case of test failures
5. Use the `#[serial]` attribute if tests might interfere with each other
6. Use `test_log::test` to enable logging within tests