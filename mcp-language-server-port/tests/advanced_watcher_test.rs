mod mock_lsp_server;
mod common;

use anyhow::Result;
use assert_fs::prelude::*;
use assert_fs::TempDir;
use serial_test::serial;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use test_log::test;
use tokio::time::{sleep, Duration};

use crate::common::{create_test_file, sample_rust_file};
use mcp_language_server_rust::lsp::Client;
use mcp_language_server_rust::watcher::{FileSystemWatcher, WorkspaceWatcher};
use mcp_language_server_rust::watcher::gitignore::GitignoreFilter;

/// Setup test environment for advanced watcher tests
async fn setup_test_env() -> Result<(TempDir, Arc<Client>)> {
    // Create a temporary directory
    let temp_dir = TempDir::new()?;
    
    // Create a .gitignore file with various patterns
    let gitignore_content = r#"# Ignore patterns
*.log
temp/
generated/*.json
**/node_modules/
.DS_Store
thumbs.db
.idea/
.vscode/
*.tmp
"#;
    
    // Create the gitignore file
    let gitignore_path = temp_dir.child(".gitignore");
    gitignore_path.write_str(gitignore_content)?;
    
    // Create some initial files
    temp_dir.child("init.rs").touch()?;
    temp_dir.child("src").create_dir_all()?;
    temp_dir.child("src/main.rs").touch()?;
    temp_dir.child("src/lib.rs").touch()?;
    
    // Create some directories that should be ignored
    temp_dir.child("temp").create_dir_all()?;
    temp_dir.child("temp/ignore_me.rs").touch()?;
    temp_dir.child("generated").create_dir_all()?;
    temp_dir.child("generated/data.json").touch()?;
    temp_dir.child("generated/important.rs").touch()?; // This should NOT be ignored
    
    // Create the LSP client
    let client = Client::new("bash", &["-c".to_string(), "cat".to_string()]).await?;
    
    // Initialize the client
    let workspace_dir = temp_dir.path();
    client.initialize(workspace_dir).await?;
    
    Ok((temp_dir, Arc::new(client)))
}

#[test(tokio::test)]
#[serial]
async fn test_gitignore_filter_complex() -> Result<()> {
    // Setup test environment
    let (temp_dir, _client) = setup_test_env().await?;
    
    // Create the gitignore filter
    let gitignore_filter = GitignoreFilter::new(temp_dir.path().to_path_buf());
    
    // Test paths that should be ignored
    let test_cases = vec![
        ("temp/file.rs", true),
        ("temp/ignore_me.rs", true),
        ("generated/data.json", true),
        ("file.log", true),
        ("logs/error.log", true),
        ("src/node_modules/package.json", true),
        (".vscode/settings.json", true),
        
        // Paths that should NOT be ignored
        ("src/main.rs", false),
        ("generated/important.rs", false),
        ("src/utils/helpers.rs", false),
        ("docs/README.md", false),
    ];
    
    for (relative_path, should_ignore) in test_cases {
        let path = temp_dir.path().join(relative_path);
        let is_ignored = gitignore_filter.is_ignored(&path);
        assert_eq!(
            is_ignored, 
            should_ignore, 
            "Path '{}' should{} be ignored", 
            relative_path, 
            if should_ignore { "" } else { " not" }
        );
    }
    
    Ok(())
}

#[test(tokio::test)]
#[serial]
async fn test_watcher_nested_directories() -> Result<()> {
    // Setup test environment
    let (temp_dir, client) = setup_test_env().await?;
    
    // Create the watcher
    let workspace_watcher = FileSystemWatcher::new(Arc::clone(&client), temp_dir.path().to_path_buf());
    
    // Start watching
    workspace_watcher.watch_workspace(temp_dir.path().to_path_buf()).await?;
    
    // Wait a bit for the watcher to initialize
    sleep(Duration::from_millis(200)).await;
    
    // Create a new nested directory structure
    let nested_dir = temp_dir.child("src/models/users");
    nested_dir.create_dir_all()?;
    
    // Wait a bit for the directory creation to be detected
    sleep(Duration::from_millis(200)).await;
    
    // Create a new file in the nested directory
    let user_model_path = nested_dir.path().join("user.rs");
    let content = r#"
pub struct User {
    id: u64,
    username: String,
    email: String,
}

impl User {
    pub fn new(id: u64, username: &str, email: &str) -> Self {
        Self {
            id,
            username: username.to_string(),
            email: email.to_string(),
        }
    }
}
"#;
    tokio::fs::write(&user_model_path, content).await?;
    
    // Wait for the file to be detected
    sleep(Duration::from_millis(300)).await;
    
    // Modify the file
    let modified_content = content.replace("username: String", "name: String");
    tokio::fs::write(&user_model_path, modified_content).await?;
    
    // Wait for the modification to be detected
    sleep(Duration::from_millis(300)).await;
    
    // Delete the file
    tokio::fs::remove_file(&user_model_path).await?;
    
    // Wait for the deletion to be detected
    sleep(Duration::from_millis(300)).await;
    
    // Remove the directory
    tokio::fs::remove_dir_all(nested_dir.path()).await?;
    
    // Wait for the directory removal to be detected
    sleep(Duration::from_millis(300)).await;
    
    // Stop the watcher
    workspace_watcher.stop().await?;
    
    // Test successful if we got here without errors
    Ok(())
}

#[test(tokio::test)]
#[serial]
async fn test_watcher_with_ignored_files() -> Result<()> {
    // Setup test environment
    let (temp_dir, client) = setup_test_env().await?;
    
    // Create the watcher
    let workspace_watcher = FileSystemWatcher::new(Arc::clone(&client), temp_dir.path().to_path_buf());
    
    // Start watching
    workspace_watcher.watch_workspace(temp_dir.path().to_path_buf()).await?;
    
    // Wait a bit for the watcher to initialize
    sleep(Duration::from_millis(200)).await;
    
    // Create a file that should be watched
    let watched_file_path = temp_dir.child("src/config.rs").path().to_path_buf();
    let content = "pub const VERSION: &str = \"1.0.0\";";
    tokio::fs::write(&watched_file_path, content).await?;
    
    // Wait for the file to be detected
    sleep(Duration::from_millis(200)).await;
    
    // Create a file that should be ignored
    let ignored_file_path = temp_dir.child("temp/ignored.rs").path().to_path_buf();
    tokio::fs::write(&ignored_file_path, "// This file should be ignored").await?;
    
    // Create a log file that should be ignored
    let log_file_path = temp_dir.child("debug.log").path().to_path_buf();
    tokio::fs::write(&log_file_path, "DEBUG: Test log entry").await?;
    
    // Wait a bit
    sleep(Duration::from_millis(200)).await;
    
    // Modify the watched file
    let modified_content = "pub const VERSION: &str = \"1.1.0\";";
    tokio::fs::write(&watched_file_path, modified_content).await?;
    
    // Wait for the modification to be detected
    sleep(Duration::from_millis(200)).await;
    
    // Modify the ignored files (shouldn't trigger watcher)
    tokio::fs::write(&ignored_file_path, "// Modified ignored file").await?;
    tokio::fs::write(&log_file_path, "DEBUG: Another log entry").await?;
    
    // Wait a bit
    sleep(Duration::from_millis(200)).await;
    
    // Delete the files
    tokio::fs::remove_file(&watched_file_path).await?;
    tokio::fs::remove_file(&ignored_file_path).await?;
    tokio::fs::remove_file(&log_file_path).await?;
    
    // Wait for the deletions to be detected
    sleep(Duration::from_millis(300)).await;
    
    // Stop the watcher
    workspace_watcher.stop().await?;
    
    // Test successful if we got here without errors
    Ok(())
}