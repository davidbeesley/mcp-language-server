mod mock_lsp_server;

use anyhow::Result;
use assert_fs::prelude::*;
use assert_fs::TempDir;
use serial_test::serial;
use test_log::test;
use tokio::fs;
use tokio::time::{sleep, Duration};

use crate::mock_lsp_server::MockLspServer;
use mcp_language_server_rust::lsp::Client;

/// Setup test environment with a workspace and some files
async fn setup_test_env() -> Result<(TempDir, MockLspServer)> {
    // Create a temporary directory for the workspace
    let temp_dir = TempDir::new()?;
    
    // Create a Rust file
    let content = r#"
fn main() {
    println!("Hello, world!");
}
"#;
    temp_dir.child("main.rs").write_str(content)?;
    
    // Create another file
    let content = r#"
fn add(a: i32, b: i32) -> i32 {
    a + b
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_add() {
        assert_eq!(add(2, 3), 5);
    }
}
"#;
    temp_dir.child("lib.rs").write_str(content)?;

    // Start the mock LSP server
    let mock_server = MockLspServer::start()?;

    Ok((temp_dir, mock_server))
}

#[test(tokio::test)]
#[serial]
async fn test_open_file() -> Result<()> {
    // Setup test environment
    let (temp_dir, mock_server) = setup_test_env().await?;

    // Start the LSP client
    let client = Client::new("bash", &["-c".to_string(), "cat".to_string()]).await?;

    // Initialize the client
    let workspace_dir = temp_dir.path();
    client.initialize(workspace_dir).await?;

    // Open a file
    let file_path = temp_dir.child("main.rs").path().to_path_buf();
    client.open_file(&file_path).await?;

    // Give some time for messages to be processed
    sleep(Duration::from_millis(100)).await;

    // Verify that the didOpen notification was sent
    let messages = mock_server.get_received_messages();
    let has_did_open = messages.iter().any(|m| m.contains("\"method\":\"textDocument/didOpen\""));
    assert!(has_did_open, "No textDocument/didOpen notification found");

    // Check if the file is marked as open
    assert!(client.is_file_open(&file_path), "File should be marked as open");

    // Clean shutdown
    client.shutdown().await?;

    Ok(())
}

#[test(tokio::test)]
#[serial]
async fn test_notify_change() -> Result<()> {
    // Setup test environment
    let (temp_dir, mock_server) = setup_test_env().await?;

    // Start the LSP client
    let client = Client::new("bash", &["-c".to_string(), "cat".to_string()]).await?;

    // Initialize the client
    let workspace_dir = temp_dir.path();
    client.initialize(workspace_dir).await?;

    // Open a file
    let file_path = temp_dir.child("main.rs").path().to_path_buf();
    client.open_file(&file_path).await?;

    // Modify the file
    let new_content = r#"
fn main() {
    println!("Hello, modified world!");
}
"#;
    fs::write(&file_path, new_content).await?;

    // Notify the LSP server of the change
    client.notify_change(&file_path).await?;

    // Give some time for messages to be processed
    sleep(Duration::from_millis(100)).await;

    // Verify that the didChange notification was sent
    let messages = mock_server.get_received_messages();
    let has_did_change = messages.iter().any(|m| m.contains("\"method\":\"textDocument/didChange\""));
    assert!(has_did_change, "No textDocument/didChange notification found");

    // Clean shutdown
    client.shutdown().await?;

    Ok(())
}

#[test(tokio::test)]
#[serial]
async fn test_close_file() -> Result<()> {
    // Setup test environment
    let (temp_dir, mock_server) = setup_test_env().await?;

    // Start the LSP client
    let client = Client::new("bash", &["-c".to_string(), "cat".to_string()]).await?;

    // Initialize the client
    let workspace_dir = temp_dir.path();
    client.initialize(workspace_dir).await?;

    // Open a file
    let file_path = temp_dir.child("main.rs").path().to_path_buf();
    client.open_file(&file_path).await?;

    // Close the file
    client.close_file(&file_path).await?;

    // Give some time for messages to be processed
    sleep(Duration::from_millis(100)).await;

    // Verify that the didClose notification was sent
    let messages = mock_server.get_received_messages();
    let has_did_close = messages.iter().any(|m| m.contains("\"method\":\"textDocument/didClose\""));
    assert!(has_did_close, "No textDocument/didClose notification found");

    // Check if the file is marked as closed
    assert!(!client.is_file_open(&file_path), "File should be marked as closed");

    // Clean shutdown
    client.shutdown().await?;

    Ok(())
}

#[test(tokio::test)]
#[serial]
async fn test_close_all_files() -> Result<()> {
    // Setup test environment
    let (temp_dir, mock_server) = setup_test_env().await?;

    // Start the LSP client
    let client = Client::new("bash", &["-c".to_string(), "cat".to_string()]).await?;

    // Initialize the client
    let workspace_dir = temp_dir.path();
    client.initialize(workspace_dir).await?;

    // Open multiple files
    let file1_path = temp_dir.child("main.rs").path().to_path_buf();
    let file2_path = temp_dir.child("lib.rs").path().to_path_buf();
    
    client.open_file(&file1_path).await?;
    client.open_file(&file2_path).await?;

    // Close all files
    client.close_all_files().await?;

    // Give some time for messages to be processed
    sleep(Duration::from_millis(100)).await;

    // Verify that the didClose notifications were sent
    let messages = mock_server.get_received_messages();
    let did_close_count = messages.iter().filter(|m| m.contains("\"method\":\"textDocument/didClose\"")).count();
    assert!(did_close_count >= 2, "Expected at least 2 textDocument/didClose notifications, got {}", did_close_count);

    // Check if the files are marked as closed
    assert!(!client.is_file_open(&file1_path), "File 1 should be marked as closed");
    assert!(!client.is_file_open(&file2_path), "File 2 should be marked as closed");

    // Clean shutdown
    client.shutdown().await?;

    Ok(())
}