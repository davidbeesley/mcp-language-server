mod mock_lsp_server;

use anyhow::Result;
use assert_fs::prelude::*;
use assert_fs::TempDir;
use lsp_types::{Diagnostic, DiagnosticSeverity, Position, Range, Url};
use serial_test::serial;
use test_log::test;
use tokio::time::{sleep, Duration};

use crate::mock_lsp_server::MockLspServer;
use mcp_language_server_rust::lsp::Client;
use mcp_language_server_rust::tools;

/// Setup test environment with a workspace and some files
async fn setup_test_env() -> Result<(TempDir, MockLspServer)> {
    // Create a temporary directory for the workspace
    let temp_dir = TempDir::new()?;
    
    // Create a Rust file with some code
    let content = r#"
struct Person {
    name: String,
    age: u32,
}

impl Person {
    fn new(name: &str, age: u32) -> Self {
        Self {
            name: name.to_string(),
            age,
        }
    }
    
    fn greet(&self) {
        println!("Hello, my name is {} and I am {} years old", self.name, self.age);
    }
}

fn main() {
    let person = Person::new("Alice", 30);
    person.greet();
}
"#;
    temp_dir.child("person.rs").write_str(content)?;

    // Start the mock LSP server
    let mock_server = MockLspServer::start()?;

    Ok((temp_dir, mock_server))
}

#[test(tokio::test)]
#[serial]
async fn test_diagnostics() -> Result<()> {
    // Setup test environment
    let (temp_dir, mock_server) = setup_test_env().await?;

    // Start the LSP client
    let client = Client::new("bash", &["-c".to_string(), "cat".to_string()]).await?;

    // Initialize the client
    let workspace_dir = temp_dir.path();
    client.initialize(workspace_dir).await?;

    // Open a file
    let file_path = temp_dir.child("person.rs").path().to_path_buf();
    client.open_file(&file_path).await?;

    // Send some mock diagnostics
    let uri = Url::from_file_path(&file_path).unwrap();
    let diagnostics = vec![
        Diagnostic {
            range: Range {
                start: Position { line: 5, character: 4 },
                end: Position { line: 5, character: 7 },
            },
            severity: Some(DiagnosticSeverity::ERROR),
            code: None,
            code_description: None,
            source: Some("test-source".to_string()),
            message: "Test error diagnostic".to_string(),
            related_information: None,
            tags: None,
            data: None,
        },
        Diagnostic {
            range: Range {
                start: Position { line: 12, character: 8 },
                end: Position { line: 12, character: 13 },
            },
            severity: Some(DiagnosticSeverity::WARNING),
            code: None,
            code_description: None,
            source: Some("test-source".to_string()),
            message: "Test warning diagnostic".to_string(),
            related_information: None,
            tags: None,
            data: None,
        },
    ];
    
    mock_server.send_diagnostics(uri.clone(), diagnostics)?;

    // Give some time for the diagnostics to be processed
    sleep(Duration::from_millis(100)).await;

    // Get diagnostics using our tool
    let diagnostics_result = tools::get_diagnostics(&client, file_path.clone(), 2, true).await?;
    
    // Check that we got the expected diagnostics
    assert!(diagnostics_result.contains("Test error diagnostic"), 
            "Diagnostics should contain error message");
    assert!(diagnostics_result.contains("Test warning diagnostic"), 
            "Diagnostics should contain warning message");

    // Clean shutdown
    client.shutdown().await?;

    Ok(())
}

#[test(tokio::test)]
#[serial]
async fn test_hover() -> Result<()> {
    // Setup test environment
    let (temp_dir, mock_server) = setup_test_env().await?;

    // Start the LSP client
    let client = Client::new("bash", &["-c".to_string(), "cat".to_string()]).await?;

    // Initialize the client
    let workspace_dir = temp_dir.path();
    client.initialize(workspace_dir).await?;

    // Open a file
    let file_path = temp_dir.child("person.rs").path().to_path_buf();
    client.open_file(&file_path).await?;

    // Set up the mock server to respond to hover requests
    let _ = tokio::spawn(async move {
        sleep(Duration::from_millis(50)).await;
        let messages = mock_server.get_received_messages();
        for message in messages {
            if message.contains("\"method\":\"textDocument/hover\"") {
                mock_server.handle_hover(lsp_types::TextDocumentPositionParams {
                    text_document: lsp_types::TextDocumentIdentifier {
                        uri: Url::from_file_path(&file_path).unwrap(),
                    },
                    position: Position { line: 7, character: 10 },
                })?;
                break;
            }
        }
        Result::<()>::Ok(())
    });

    // Get hover info using our tool
    let line = 8; // 1-indexed for our tool
    let column = 11; // 1-indexed for our tool
    let hover_result = tools::get_hover_info(&client, file_path.clone(), line, column).await?;
    
    // Verify we got some hover information
    assert!(!hover_result.is_empty(), "Hover result should not be empty");

    // Clean shutdown
    client.shutdown().await?;

    Ok(())
}

#[test(tokio::test)]
#[serial]
async fn test_text_edits() -> Result<()> {
    // Setup test environment
    let (temp_dir, _) = setup_test_env().await?;

    // Start the LSP client
    let client = Client::new("bash", &["-c".to_string(), "cat".to_string()]).await?;

    // Initialize the client
    let workspace_dir = temp_dir.path();
    client.initialize(workspace_dir).await?;

    // Create a new file for testing edits
    let edit_file_path = temp_dir.child("edit_test.rs").path().to_path_buf();
    let initial_content = "fn main() {\n    println!(\"Hello, world!\");\n}";
    tokio::fs::write(&edit_file_path, initial_content).await?;

    // Open the file
    client.open_file(&edit_file_path).await?;

    // Apply text edits using our tool
    let edits = vec![
        tools::edit::TextEditParams {
            start_line: 2, // 1-indexed
            end_line: 2,   // 1-indexed
            new_text: "    println!(\"Hello, edited world!\");\n",
        },
    ];

    let edit_result = tools::apply_text_edits(&client, edit_file_path.clone(), edits).await?;
    
    // Verify the edit was successful
    assert!(edit_result.contains("Successfully applied"), "Edit result should indicate success");

    // Read the file content to verify the edit
    let updated_content = tokio::fs::read_to_string(&edit_file_path).await?;
    assert!(updated_content.contains("Hello, edited world!"), "File content should contain the edited text");

    // Clean shutdown
    client.shutdown().await?;

    Ok(())
}

// This test can be expanded to test more LSP features as needed
// For example, adding tests for definition, references, and rename operations