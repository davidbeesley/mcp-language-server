mod mock_lsp_server;

use anyhow::Result;
use assert_fs::prelude::*;
use assert_fs::TempDir;
use serial_test::serial;
use std::sync::Arc;
use test_log::test;
use tokio::time::{sleep, Duration};

// MockLspServer not used in these tests
use mcp_language_server_rust::lsp::Client;
use mcp_language_server_rust::watcher::{FileSystemWatcher, WorkspaceWatcher};

/// Setup test environment - returns Arc'd client
async fn setup_test_env() -> Result<(TempDir, Arc<Client>)> {
    // Create a temporary directory for the workspace
    let temp_dir = TempDir::new()?;
    
    // Create some initial files
    temp_dir.child("init.rs").touch()?;
    temp_dir.child("init2.rs").touch()?;
    
    // Create a .gitignore file
    temp_dir.child(".gitignore").write_str("ignored.rs\n*.log\ntmp/\n")?;
    
    // Create a directory that should be ignored
    temp_dir.child("tmp").create_dir_all()?;
    temp_dir.child("tmp/ignored.rs").touch()?;

    // Start the LSP client - note that it returns Arc<Client>
    let client = Client::new("bash", &["-c".to_string(), "cat".to_string()]).await?;

    // Initialize the client
    let workspace_dir = temp_dir.path();
    client.initialize(workspace_dir).await?;

    Ok((temp_dir, client))
}

#[test(tokio::test)]
#[serial]
async fn test_watcher_creation() -> Result<()> {
    // Setup test environment
    let (temp_dir, client) = setup_test_env().await?;
    
    // Create the watcher - client is already an Arc<Client>
    let workspace_watcher = FileSystemWatcher::new(Arc::clone(&client), temp_dir.path().to_path_buf());
    
    // Start watching
    workspace_watcher.watch_workspace(temp_dir.path().to_path_buf()).await?;
    
    // Wait a bit for the watcher to initialize
    sleep(Duration::from_millis(100)).await;
    
    // Stop the watcher
    workspace_watcher.stop().await?;
    
    Ok(())
}

#[test(tokio::test)]
#[serial]
async fn test_watcher_file_changes() -> Result<()> {
    // Setup test environment
    let (temp_dir, client) = setup_test_env().await?;
    
    // Create the watcher - client is already an Arc<Client>
    let workspace_watcher = FileSystemWatcher::new(Arc::clone(&client), temp_dir.path().to_path_buf());
    
    // Start watching
    workspace_watcher.watch_workspace(temp_dir.path().to_path_buf()).await?;
    
    // Wait a bit for the watcher to initialize
    sleep(Duration::from_millis(100)).await;
    
    // Create a new file
    let test_file_path = temp_dir.child("new_test.rs").path().to_path_buf();
    tokio::fs::write(&test_file_path, "fn main() {}\n").await?;
    
    // Wait for the file to be detected
    sleep(Duration::from_millis(300)).await;
    
    // Create an ignored file (shouldn't trigger any changes)
    let ignored_file_path = temp_dir.child("ignored.rs").path().to_path_buf();
    tokio::fs::write(&ignored_file_path, "// This should be ignored\n").await?;
    
    // Wait a bit
    sleep(Duration::from_millis(100)).await;
    
    // Modify the test file
    tokio::fs::write(&test_file_path, "fn main() { println!(\"modified\"); }\n").await?;
    
    // Wait for the modification to be detected
    sleep(Duration::from_millis(300)).await;
    
    // Remove the test file
    tokio::fs::remove_file(&test_file_path).await?;
    
    // Wait for the removal to be detected
    sleep(Duration::from_millis(300)).await;
    
    // Stop the watcher
    workspace_watcher.stop().await?;
    
    Ok(())
}

#[test(tokio::test)]
#[serial]
async fn test_gitignore_filter() -> Result<()> {
    // Setup test environment
    let (temp_dir, _client) = setup_test_env().await?;
    
    // Create the gitignore filter
    let gitignore_filter = mcp_language_server_rust::watcher::gitignore::GitignoreFilter::new(
        temp_dir.path().to_path_buf()
    );
    
    // Test paths that should be ignored
    let ignored_path = temp_dir.child("ignored.rs").path().to_path_buf();
    assert!(gitignore_filter.is_ignored(&ignored_path), "ignored.rs should be ignored");
    
    let log_path = temp_dir.child("test.log").path().to_path_buf();
    assert!(gitignore_filter.is_ignored(&log_path), "test.log should be ignored");
    
    let tmp_path = temp_dir.child("tmp/test.rs").path().to_path_buf();
    assert!(gitignore_filter.is_ignored(&tmp_path), "tmp/test.rs should be ignored");
    
    // Test paths that should not be ignored
    let normal_path = temp_dir.child("normal.rs").path().to_path_buf();
    assert!(!gitignore_filter.is_ignored(&normal_path), "normal.rs should not be ignored");
    
    Ok(())
}