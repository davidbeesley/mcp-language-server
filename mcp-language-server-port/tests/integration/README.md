# Integration Tests for MCP Language Server

Integration tests in this directory validate the functionality of the MCP Language Server by testing all components together.

These tests verify:
1. Client initialization and shutdown
2. File system watching
3. File operations (open, change, close)
4. LSP feature integration (diagnostics, hover, definition)
5. MCP API endpoints

## Running the Tests

```bash
# Run all tests
cargo test

# Run a specific test
cargo test --test client_init_test

# Run with logging
RUST_LOG=debug cargo test
```

## Test Structure

- **mock_lsp_server.rs** - A mock LSP server implementation for testing
- **client_init_test.rs** - Tests for client initialization and shutdown
- **file_operations_test.rs** - Tests for file operations
- **lsp_features_test.rs** - Tests for LSP features
- **mcp_integration_test.rs** - Tests for MCP API integration
- **watcher_test.rs** - Tests for file system watcher
- **full_integration_test.rs** - End-to-end integration tests