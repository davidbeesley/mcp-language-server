mod mock_lsp_server;
mod common;

use anyhow::Result;
use assert_fs::TempDir;
use lsp_types::{TextEdit, WorkspaceEdit};
use serial_test::serial;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use test_log::test;
use tokio::time::{sleep, Duration};

use crate::common::{create_test_file, read_file_content, complex_rust_file};
use crate::mock_lsp_server::MockLspServer;
use mcp_language_server_rust::lsp::Client;
use mcp_language_server_rust::tools;

/// Setup test environment for rename tests
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
async fn test_rename_symbol() -> Result<()> {
    // Setup test environment
    let (temp_dir, mock_server, client, file_path) = setup_test_env().await?;
    
    // Define the symbol to rename and its new name
    let line = 6; // The 'name' field line
    let column = 9; // The 'n' in 'name'
    let new_name = "fullName"; // New name for the field
    
    // Use the task function to rename the symbol
    let result = tools::rename_symbol(&client, file_path.clone(), line - 1, column - 1, new_name.to_string()).await?;
    
    // Verify the result
    assert!(result.contains("Applied"), "Result should contain 'Applied'");
    assert!(result.contains("edits"), "Result should contain 'edits'");
    
    // Verify the file was actually modified
    let content = read_file_content(&file_path).await?;
    assert!(content.contains("fullName:"), "File should now contain 'fullName:'");
    assert!(!content.contains("name:"), "File should no longer contain 'name:'");
    
    // Clean shutdown
    client.shutdown().await?;
    
    Ok(())
}

#[test(tokio::test)]
#[serial]
async fn test_rename_method() -> Result<()> {
    // Setup test environment
    let (temp_dir, mock_server, client, file_path) = setup_test_env().await?;
    
    // Define the method to rename and its new name
    let line = 28; // The 'add_attribute' method line
    let column = 16; // The 'a' in 'add_attribute'
    let new_name = "setAttribute"; // New name for the method
    
    // Use the task function to rename the symbol
    let result = tools::rename_symbol(&client, file_path.clone(), line - 1, column - 1, new_name.to_string()).await?;
    
    // Verify the result
    assert!(result.contains("Applied"), "Result should contain 'Applied'");
    assert!(result.contains("edits"), "Result should contain 'edits'");
    
    // Verify the file was actually modified
    let content = read_file_content(&file_path).await?;
    assert!(content.contains("setAttribute"), "File should now contain 'setAttribute'");
    assert!(!content.contains("add_attribute"), "File should no longer contain 'add_attribute'");
    
    // Clean shutdown
    client.shutdown().await?;
    
    Ok(())
}