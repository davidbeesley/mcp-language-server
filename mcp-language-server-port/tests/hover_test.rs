mod mock_lsp_server;
mod common;

use anyhow::Result;
use assert_fs::TempDir;
use lsp_types::{Hover, HoverContents, MarkupContent, MarkupKind, Position};
use serial_test::serial;
use std::path::PathBuf;
use std::sync::Arc;
use test_log::test;
use tokio::time::{sleep, Duration};

use crate::common::{create_test_file, complex_rust_file};
use crate::mock_lsp_server::MockLspServer;
use mcp_language_server_rust::lsp::Client;
use mcp_language_server_rust::tools;

/// Setup test environment for hover tests
async fn setup_test_env() -> Result<(TempDir, MockLspServer, Arc<Client>, PathBuf)> {
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
    
    Ok((temp_dir, mock_server, Arc::new(client), file_path))
}

#[test(tokio::test)]
#[serial]
async fn test_get_hover_info() -> Result<()> {
    // Setup test environment
    let (temp_dir, mock_server, client, file_path) = setup_test_env().await?;
    
    // Define the position to hover over
    let line = 6; // The 'name' field line
    let column = 9; // The 'n' in 'name'
    
    // Use the task function to get hover information
    let result = tools::get_hover_info(&client, file_path.clone(), line, column).await?;
    
    // Verify the result
    assert!(!result.is_empty(), "Hover result should not be empty");
    assert!(result.contains("Mock hover information"), 
            "Result should contain mock hover information");
    
    // Clean shutdown
    client.shutdown().await?;
    
    Ok(())
}

#[test(tokio::test)]
#[serial]
async fn test_get_hover_info_method() -> Result<()> {
    // Setup test environment
    let (temp_dir, mock_server, client, file_path) = setup_test_env().await?;
    
    // Define the position to hover over a method
    let line = 28; // The 'add_attribute' method line
    let column = 16; // The 'a' in 'add_attribute'
    
    // Use the task function to get hover information
    let result = tools::get_hover_info(&client, file_path.clone(), line, column).await?;
    
    // Verify the result
    assert!(!result.is_empty(), "Hover result should not be empty");
    assert!(result.contains("Mock hover information"), 
            "Result should contain mock hover information");
    
    // Clean shutdown
    client.shutdown().await?;
    
    Ok(())
}

#[test(tokio::test)]
#[serial]
async fn test_get_hover_info_no_info_available() -> Result<()> {
    // Setup test environment
    let (temp_dir, _mock_server, client, file_path) = setup_test_env().await?;
    
    // Define a position where there's likely no hover info
    let line = 1; // A comment line
    let column = 1; // Beginning of line
    
    // Use the task function to get hover information
    let result = tools::get_hover_info(&client, file_path.clone(), line, column).await?;
    
    // Verify the result indicates no information
    assert!(result.contains("No hover information available"), 
            "Result should indicate no hover information is available");
    
    // Clean shutdown
    client.shutdown().await?;
    
    Ok(())
}