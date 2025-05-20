mod mock_lsp_server;

use anyhow::Result;
use assert_fs::prelude::*;
use assert_fs::TempDir;
// No imports needed from lsp_types
use serial_test::serial;
use test_log::test;
use tokio::time::{sleep, Duration};

use crate::mock_lsp_server::MockLspServer;
use mcp_language_server_rust::lsp::Client;

/// Setup test environment with a workspace
async fn setup_test_env() -> Result<(TempDir, MockLspServer)> {
    // Create a temporary directory for the workspace
    let temp_dir = TempDir::new()?;
    temp_dir.child("file.rs").touch()?;

    // Start the mock LSP server
    let mock_server = MockLspServer::start()?;

    Ok((temp_dir, mock_server))
}

#[test(tokio::test)]
#[serial]
async fn test_client_initialization() -> Result<()> {
    // Setup test environment
    let (temp_dir, mock_server) = setup_test_env().await?;

    // Start the LSP client
    let client = Client::new("bash", &["-c".to_string(), "cat".to_string()]).await?;

    // Initialize the client
    let workspace_dir = temp_dir.path();
    client.initialize(workspace_dir).await?;

    // Give some time for messages to be processed
    sleep(Duration::from_millis(100)).await;

    // Verify that messages were sent
    let messages = mock_server.get_received_messages();
    
    // We should have at least two messages: initialize and initialized
    assert!(messages.len() >= 2, "Expected at least 2 messages, got {}", messages.len());
    
    // Check that the first message is an initialize request
    let init_message = &messages[0];
    assert!(init_message.contains("\"method\":\"initialize\""), 
            "First message should be initialize, got: {}", init_message);
    
    // Check that one of the messages is an initialized notification
    let has_initialized = messages.iter().any(|m| m.contains("\"method\":\"initialized\""));
    assert!(has_initialized, "No initialized notification found");

    // Clean shutdown
    client.shutdown().await?;

    Ok(())
}

#[test(tokio::test)]
#[serial]
async fn test_client_shutdown() -> Result<()> {
    // Setup test environment
    let (temp_dir, mock_server) = setup_test_env().await?;

    // Start the LSP client
    let client = Client::new("bash", &["-c".to_string(), "cat".to_string()]).await?;

    // Initialize the client
    let workspace_dir = temp_dir.path();
    client.initialize(workspace_dir).await?;

    // Clean shutdown
    client.shutdown().await?;

    // Give some time for messages to be processed
    sleep(Duration::from_millis(100)).await;

    // Verify that messages were sent
    let messages = mock_server.get_received_messages();
    
    // Check that one of the messages is a shutdown request
    let has_shutdown = messages.iter().any(|m| m.contains("\"method\":\"shutdown\""));
    assert!(has_shutdown, "No shutdown request found");
    
    // Check that one of the messages is an exit notification
    let has_exit = messages.iter().any(|m| m.contains("\"method\":\"exit\""));
    assert!(has_exit, "No exit notification found");

    Ok(())
}