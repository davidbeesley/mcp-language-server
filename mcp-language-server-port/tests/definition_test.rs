mod mock_lsp_server;
mod common;

use anyhow::Result;
use assert_fs::TempDir;
use lsp_types::Url;
use serial_test::serial;
use std::sync::Arc;
use test_log::test;
use tokio::time::{sleep, Duration};

use crate::common::{create_test_file, complex_rust_file};
use crate::mock_lsp_server::MockLspServer;
use mcp_language_server_rust::lsp::Client;
use mcp_language_server_rust::tools;

/// Setup test environment for definition tests
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
async fn test_find_definition() -> Result<()> {
    // Setup test environment
    let (temp_dir, mock_server, client, file_path) = setup_test_env().await?;
    
    // Create the symbol location string in the format "path:line:column"
    // Let's find the definition of the Person struct
    let symbol_location = format!("{}:21:10", file_path); // line 21, column 10 (the 'p' in Person in process_people)
    
    // Set up the mock server to respond to definition requests
    let uri = MockLspServer::path_to_uri(&std::path::PathBuf::from(&file_path));
    let response_line = 5; // Line of the Person struct definition
    
    // Use the task function to find the definition
    let result = tools::find_definition(&client, &symbol_location).await?;
    
    // Verify the result
    assert!(result.contains("struct Person"), "Definition should contain 'struct Person'");
    
    // Clean shutdown
    client.shutdown().await?;
    
    Ok(())
}

#[test(tokio::test)]
#[serial]
async fn test_find_definition_invalid_location() -> Result<()> {
    // Setup test environment
    let (temp_dir, mock_server, client, file_path) = setup_test_env().await?;
    
    // Create an invalid symbol location
    let symbol_location = format!("{}:999:999", file_path); // Non-existent location
    
    // Try to find the definition
    let result = tools::find_definition(&client, &symbol_location).await;
    
    // Verify the result is an error
    assert!(result.is_err(), "Definition lookup at invalid location should fail");
    
    // Clean shutdown
    client.shutdown().await?;
    
    Ok(())
}