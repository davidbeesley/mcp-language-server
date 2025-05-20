mod mock_lsp_server;
mod common;

use anyhow::Result;
use assert_fs::TempDir;
use lsp_types::{Location, Position, Range, Url};
use serial_test::serial;
use std::sync::Arc;
use test_log::test;
use tokio::time::{sleep, Duration};

use crate::common::{create_test_file, complex_rust_file};
use crate::mock_lsp_server::MockLspServer;
use mcp_language_server_rust::lsp::Client;
use mcp_language_server_rust::tools;

/// Setup test environment for references tests
async fn setup_test_env() -> Result<(TempDir, MockLspServer, Arc<Client>, String)> {
    // Create a temporary directory
    let temp_dir = TempDir::new()?;
    
    // Create a Rust file with a more complex structure
    let content = complex_rust_file();
    let file_path = create_test_file(&temp_dir, "complex.rs", content).await?;
    
    // Start the mock LSP server
    let mock_server = MockLspServer::start()?;
    
    // Start the LSP client
    let client = Client::new("bash", &["-c".to_string(), "cat".to_string()]).await?;
    
    // Initialize the client
    client.initialize(temp_dir.path()).await?;
    
    // Open the file
    client.open_file(&file_path).await?;
    
    Ok((temp_dir, mock_server, Arc::new(client), file_path.to_string_lossy().to_string()))
}

#[test(tokio::test)]
#[serial]
async fn test_find_references() -> Result<()> {
    // Setup test environment
    let (temp_dir, mock_server, client, file_path) = setup_test_env().await?;
    
    // Create the symbol location string in the format "path:line:column"
    // Let's find references to the 'name' field
    let symbol_location = format!("{}:6:9", file_path); // line 6, column 9 (the 'n' in name field)
    
    // Set up the mock server to handle references requests
    // This is handled automatically by our improved mock server
    
    // Use the task function to find references
    let result = tools::find_references(&client, &symbol_location).await?;
    
    // Verify the result
    assert!(result.contains("Found"), "Result should contain 'Found'");
    assert!(result.contains("references"), "Result should contain 'references'");
    
    // Clean shutdown
    client.shutdown().await?;
    
    Ok(())
}

#[test(tokio::test)]
#[serial]
async fn test_find_references_method() -> Result<()> {
    // Setup test environment
    let (temp_dir, mock_server, client, file_path) = setup_test_env().await?;
    
    // Create the symbol location string in the format "path:line:column"
    // Let's find references to the 'add_attribute' method
    let symbol_location = format!("{}:28:16", file_path); // line 28, column 16 (add_attribute method)
    
    // Use the task function to find references
    let result = tools::find_references(&client, &symbol_location).await?;
    
    // Verify the result
    assert!(result.contains("Found"), "Result should contain 'Found'");
    assert!(result.contains("references"), "Result should contain 'references'");
    
    // Clean shutdown
    client.shutdown().await?;
    
    Ok(())
}