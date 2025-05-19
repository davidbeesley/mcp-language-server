mod mock_lsp_server;

use anyhow::Result;
use assert_fs::prelude::*;
use assert_fs::TempDir;
use serial_test::serial;
use std::sync::Arc;
use test_log::test;
use tokio::time::{sleep, Duration};

use mcp_language_server_rust::lsp::Client;
use mcp_language_server_rust::mcp::McpLanguageServer;
use mcp_language_server_rust::tools;
use mcp_language_server_rust::watcher::{FileSystemWatcher, WorkspaceWatcher};

/// Integration test that combines all components
#[test(tokio::test)]
#[serial]
async fn test_full_integration() -> Result<()> {
    // Create a temporary workspace
    let temp_dir = TempDir::new()?;
    
    // Create a Rust file
    let code = r#"
// A sample Rust file for testing
struct User {
    name: String,
    email: String,
}

impl User {
    fn new(name: &str, email: &str) -> Self {
        Self {
            name: name.to_string(),
            email: email.to_string(),
        }
    }
    
    fn greet(&self) -> String {
        format!("Hello, {}!", self.name)
    }
}

fn main() {
    let user = User::new("Alice", "alice@example.com");
    println!("{}", user.greet());
}
"#;
    let file_path = temp_dir.child("main.rs").path().to_path_buf();
    tokio::fs::write(&file_path, code).await?;
    
    // Start an LSP client using a simple echo server
    let client = Client::new("bash", &["-c".to_string(), "cat".to_string()]).await?;
    
    // Initialize the client
    let workspace_dir = temp_dir.path();
    client.initialize(workspace_dir).await?;
    
    // Create a file watcher
    let client_arc = Arc::new(client);
    let workspace_watcher = FileSystemWatcher::new(Arc::clone(&client_arc), workspace_dir.to_path_buf());
    
    // Start watching the workspace
    workspace_watcher.watch_workspace(workspace_dir.to_path_buf()).await?;
    
    // Open the file
    client_arc.open_file(&file_path).await?;
    
    // Create an MCP server
    let mcp_server = McpLanguageServer::new(Arc::clone(&client_arc), workspace_dir.to_path_buf());
    
    // Test file modifications via tools
    let edits = vec![
        tools::edit::TextEditParams {
            start_line: 4,
            end_line: 4,
            new_text: "    name: String,\n    email: String,\n    age: u32,\n",
        }
    ];
    
    // Apply the edits
    let edit_result = tools::apply_text_edits(&client_arc, file_path.clone(), edits).await?;
    assert!(edit_result.contains("Successfully"), "Edit should be successful");
    
    // Read the updated file
    let updated_content = tokio::fs::read_to_string(&file_path).await?;
    assert!(updated_content.contains("age: u32"), "File should contain the new field");
    
    // Test modifying the file again
    let edits = vec![
        tools::edit::TextEditParams {
            start_line: 12,
            end_line: 12,
            new_text: "            name: name.to_string(),\n            email: email.to_string(),\n            age: 0,\n",
        }
    ];
    
    // Apply the edits
    let edit_result = tools::apply_text_edits(&client_arc, file_path.clone(), edits).await?;
    assert!(edit_result.contains("Successfully"), "Edit should be successful");
    
    // Read the updated file again
    let updated_content = tokio::fs::read_to_string(&file_path).await?;
    assert!(updated_content.contains("age: 0"), "File should contain the new field initialization");
    
    // Clean up
    workspace_watcher.stop().await?;
    client_arc.shutdown().await?;
    
    Ok(())
}