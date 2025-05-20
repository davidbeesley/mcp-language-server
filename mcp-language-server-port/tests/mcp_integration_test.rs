mod mock_lsp_server;

use anyhow::Result;
use assert_fs::prelude::*;
use assert_fs::TempDir;
// futures::FutureExt is used within impl blocks
// ServerInfo is used via trait
use serial_test::serial;
use serde_json::json;
use std::sync::Arc;
use test_log::test;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration};

use crate::mock_lsp_server::MockLspServer;
use mcp_language_server_rust::lsp::Client;
use mcp_language_server_rust::mcp::McpLanguageServer;

// Create a mock transport for testing MCP server
struct MockTransport {
    tx: mpsc::Sender<String>,
    rx: mpsc::Receiver<String>,
}

impl MockTransport {
    fn new() -> (Self, mpsc::Receiver<String>, mpsc::Sender<String>) {
        let (in_tx, in_rx) = mpsc::channel(100);
        let (out_tx, out_rx) = mpsc::channel(100);
        
        (Self { tx: out_tx, rx: in_rx }, out_rx, in_tx)
    }
}

impl AsyncRead for MockTransport {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        // Simplified implementation for testing
        let this = self.get_mut();
        
        // We need a pinned future
        use futures::FutureExt;
        
        // Create a new future each time
        let mut future = Box::pin(this.rx.recv());
        
        match future.poll_unpin(cx) {
            std::task::Poll::Ready(opt) => {
                match opt {
                    Some(message) => {
                        let bytes = message.into_bytes();
                        let len = bytes.len().min(buf.remaining());
                        buf.put_slice(&bytes[..len]);
                        std::task::Poll::Ready(Ok(()))
                    }
                    None => std::task::Poll::Ready(Ok(())),
                }
            }
            std::task::Poll::Pending => std::task::Poll::Pending,
        }
    }
}

impl AsyncWrite for MockTransport {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        // Simplified implementation for testing
        let this = self.get_mut();
        let message = std::str::from_utf8(buf).unwrap_or("invalid utf8").to_string();
        let len = buf.len();
        
        // We need a pinned future
        use futures::FutureExt;
        
        // Create a new future each time
        let mut future = Box::pin(this.tx.send(message));
        
        match future.poll_unpin(cx) {
            std::task::Poll::Ready(result) => {
                match result {
                    Ok(_) => std::task::Poll::Ready(Ok(len)),
                    Err(_) => std::task::Poll::Ready(Ok(0)),
                }
            }
            std::task::Poll::Pending => std::task::Poll::Pending,
        }
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::task::Poll::Ready(Ok(()))
    }
}

/// Setup test environment - returns Arc'd client
async fn setup_test_env() -> Result<(TempDir, MockLspServer, Arc<Client>)> {
    // Create a temporary directory for the workspace
    let temp_dir = TempDir::new()?;
    
    // Create a Rust file with some code
    let content = r#"
fn main() {
    println!("Hello, MCP!");
}
"#;
    temp_dir.child("mcp_test.rs").write_str(content)?;

    // Start the mock LSP server
    let mock_server = MockLspServer::start()?;

    // Start the LSP client - note it returns Arc<Client>
    let client = Client::new("bash", &["-c".to_string(), "cat".to_string()]).await?;

    // Initialize the client
    let workspace_dir = temp_dir.path();
    client.initialize(workspace_dir).await?;

    Ok((temp_dir, mock_server, client))
}

#[test(tokio::test)]
#[serial]
async fn test_mcp_server_info() -> Result<()> {
    // Setup test environment
    let (temp_dir, _mock_server, lsp_client) = setup_test_env().await?;

    // Create MCP server - client is already an Arc<Client>
    let mcp_server = McpLanguageServer::new(Arc::clone(&lsp_client), temp_dir.path().to_path_buf());

    // Test the server info
    let info = rmcp::ServerHandler::get_info(&mcp_server);
    
    assert!(!info.instructions.unwrap_or_default().is_empty(), "Server info should have instructions");

    Ok(())
}

#[test(tokio::test)]
#[serial]
async fn test_mcp_diagnostics_tool() -> Result<()> {
    // Setup test environment
    let (temp_dir, mock_server, lsp_client) = setup_test_env().await?;

    // Create test file and open it
    let file_path = temp_dir.child("diagnostics_test.rs").path().to_path_buf();
    let content = "fn main() {\n    println!(\"Hello, MCP diagnostics!\");\n}";
    tokio::fs::write(&file_path, content).await?;
    lsp_client.open_file(&file_path).await?;

    // Send mock diagnostics
    let uri = MockLspServer::path_to_uri(&file_path);
    let diagnostics = MockLspServer::create_mock_diagnostics();
    mock_server.send_diagnostics(uri, diagnostics)?;

    // Create MCP server - client is already an Arc<Client>
    let mcp_server = McpLanguageServer::new(Arc::clone(&lsp_client), temp_dir.path().to_path_buf());

    // Create a mock transport
    let (transport, mut out_rx, in_tx) = MockTransport::new();

    // Send a diagnostics request
    let request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "diagnostics",
        "params": {
            "file_path": file_path.to_string_lossy().to_string(),
            "context_lines": 2,
            "show_line_numbers": true
        }
    });
    in_tx.send(serde_json::to_string(&request)?).await?;

    // Start the MCP server in a separate task
    tokio::spawn(async move {
        let _ = rmcp::serve_server(mcp_server, transport).await;
    });

    // Wait for the response
    sleep(Duration::from_millis(100)).await;

    // Check if we got a response
    match out_rx.try_recv() {
        Ok(response) => {
            assert!(response.contains("\"result\""), "Response should contain a result");
            assert!(response.contains("Mock error diagnostic") || response.contains("Mock warning diagnostic"), 
                    "Response should contain diagnostic messages");
        }
        Err(e) => {
            panic!("Failed to receive response: {:?}", e);
        }
    }

    Ok(())
}

#[test(tokio::test)]
#[serial]
async fn test_mcp_edit_tool() -> Result<()> {
    // Setup test environment
    let (temp_dir, _mock_server, lsp_client) = setup_test_env().await?;

    // Create test file and open it
    let file_path = temp_dir.child("edit_test.rs").path().to_path_buf();
    let content = "fn main() {\n    println!(\"Hello, MCP edit!\");\n}";
    tokio::fs::write(&file_path, content).await?;
    lsp_client.open_file(&file_path).await?;

    // Create MCP server - client is already an Arc<Client>
    let mcp_server = McpLanguageServer::new(Arc::clone(&lsp_client), temp_dir.path().to_path_buf());

    // Create a mock transport
    let (transport, mut out_rx, in_tx) = MockTransport::new();

    // Send an edit request
    let request = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "edit_file",
        "params": {
            "file_path": file_path.to_string_lossy().to_string(),
            "edits": [
                {
                    "start_line": 2,
                    "end_line": 2,
                    "new_text": "    println!(\"Hello, MCP edited!\");\n"
                }
            ]
        }
    });
    in_tx.send(serde_json::to_string(&request)?).await?;

    // Start the MCP server in a separate task
    tokio::spawn(async move {
        let _ = rmcp::serve_server(mcp_server, transport).await;
    });

    // Wait for the response
    sleep(Duration::from_millis(100)).await;

    // Check if we got a response
    match out_rx.try_recv() {
        Ok(response) => {
            assert!(response.contains("\"result\""), "Response should contain a result");
            assert!(response.contains("Successfully applied"), "Response should indicate success");
            
            // Verify the file was actually edited
            let updated_content = tokio::fs::read_to_string(&file_path).await?;
            assert!(updated_content.contains("Hello, MCP edited!"), 
                    "File content should be updated");
        }
        Err(e) => {
            panic!("Failed to receive response: {:?}", e);
        }
    }

    Ok(())
}

// Additional tests can be added for hover, definition, references, and rename tools