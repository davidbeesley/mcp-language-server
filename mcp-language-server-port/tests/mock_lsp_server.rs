use anyhow::Result;
use log::debug;
use lsp_types::{
    Diagnostic, DiagnosticSeverity, Hover, HoverContents, InitializeParams, InitializeResult,
    InitializedParams, Location, MarkupContent, MarkupKind, Position, Range, ServerCapabilities,
    TextDocumentPositionParams, Url, WorkspaceEdit,
};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicI32, Ordering},
        Arc, Mutex,
    },
    thread,
};

/// Mock LSP server for testing
pub struct MockLspServer {
    child: Child,
    next_id: AtomicI32,
    received_messages: Arc<Mutex<Vec<String>>>,
}

impl MockLspServer {
    /// Start a new mock LSP server process
    pub fn start() -> Result<Self> {
        // Create a simple echo script
        let script = r#"#!/bin/bash
# Simple script that forwards stdin to stdout
while IFS= read -r line; do
    echo "$line"
done
"#;

        // Create a temporary file for the script
        let temp_dir = tempfile::tempdir()?;
        let script_path = temp_dir.path().join("mock_lsp.sh");
        std::fs::write(&script_path, script)?;
        std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755))?;

        // Start the process
        let mut child = Command::new(script_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let received_messages = Arc::new(Mutex::new(Vec::new()));
        let received_messages_clone = Arc::clone(&received_messages);

        // Start a thread to handle stdout
        let stdout = child.stdout.take().unwrap();
        let mut reader = BufReader::new(stdout);

        thread::spawn(move || {
            let mut line = String::new();
            let mut content_length = 0;
            let mut reading_headers = true;

            loop {
                line.clear();
                if reader.read_line(&mut line).is_err() {
                    break;
                }

                if line.is_empty() {
                    break;
                }

                if reading_headers {
                    if line.starts_with("Content-Length: ") {
                        content_length = line
                            .trim_start_matches("Content-Length: ")
                            .trim()
                            .parse()
                            .unwrap_or(0);
                    } else if line.trim().is_empty() {
                        reading_headers = false;
                    }
                } else {
                    // Read the content
                    let mut content = vec![0; content_length];
                    if reader.read_exact(&mut content).is_err() {
                        break;
                    }

                    let message = String::from_utf8_lossy(&content).to_string();
                    debug!("[MOCK] Received message: {}", message);

                    // Store the message
                    let mut messages = received_messages_clone.lock().unwrap();
                    messages.push(message);

                    // Reset for next message
                    reading_headers = true;
                }
            }
        });

        Ok(Self {
            child,
            next_id: AtomicI32::new(1),
            received_messages,
        })
    }

    /// Send a mock initialize response
    pub fn handle_initialize(&self, params: InitializeParams) -> Result<()> {
        // Create a response with minimal capabilities
        let result = InitializeResult {
            capabilities: ServerCapabilities {
                hover_provider: Some(lsp_types::HoverProviderCapability::Simple(true)),
                definition_provider: Some(lsp_types::OneOf::Left(true)),
                references_provider: Some(lsp_types::OneOf::Left(true)),
                rename_provider: Some(lsp_types::OneOf::Left(true)),
                ..Default::default()
            },
            server_info: Some(lsp_types::ServerInfo {
                name: "mock-lsp-server".to_string(),
                version: Some("0.1.0".to_string()),
            }),
        };

        // Send the response
        self.send_response(1, result)?;

        Ok(())
    }

    /// Handle initialized notification
    pub fn handle_initialized(&self, _params: InitializedParams) -> Result<()> {
        // Nothing to do for this notification
        Ok(())
    }

    /// Handle textDocument/hover request
    pub fn handle_hover(&self, params: TextDocumentPositionParams) -> Result<()> {
        // Create a mock hover response
        let hover = Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: format!(
                    "Mock hover information for position {}:{}",
                    params.position.line, params.position.character
                ),
            }),
            range: None,
        };

        // Send the response
        self.send_response(self.next_id.load(Ordering::SeqCst), hover)?;

        Ok(())
    }

    /// Handle textDocument/definition request
    pub fn handle_definition(&self, params: TextDocumentPositionParams) -> Result<()> {
        // Create a mock location response
        let location = Location {
            uri: params.text_document.uri.clone(),
            range: Range {
                start: Position {
                    line: params.position.line,
                    character: params.position.character,
                },
                end: Position {
                    line: params.position.line,
                    character: params.position.character + 5,
                },
            },
        };

        // Send the response
        self.send_response(self.next_id.load(Ordering::SeqCst), vec![location])?;

        Ok(())
    }

    /// Handle textDocument/references request
    pub fn handle_references(
        &self,
        params: lsp_types::ReferenceParams,
    ) -> Result<()> {
        // Create a mock locations response
        let locations = vec![
            Location {
                uri: params.text_document_position.text_document.uri.clone(),
                range: Range {
                    start: Position {
                        line: params.text_document_position.position.line,
                        character: params.text_document_position.position.character,
                    },
                    end: Position {
                        line: params.text_document_position.position.line,
                        character: params.text_document_position.position.character + 5,
                    },
                },
            },
            Location {
                uri: params.text_document_position.text_document.uri.clone(),
                range: Range {
                    start: Position {
                        line: params.text_document_position.position.line + 10,
                        character: 0,
                    },
                    end: Position {
                        line: params.text_document_position.position.line + 10,
                        character: 5,
                    },
                },
            },
        ];

        // Send the response
        self.send_response(self.next_id.load(Ordering::SeqCst), locations)?;

        Ok(())
    }

    /// Handle textDocument/rename request
    pub fn handle_rename(&self, params: lsp_types::RenameParams) -> Result<()> {
        // Create a mock workspace edit response
        let uri = params.text_document_position.text_document.uri.clone();
        let mut changes = HashMap::new();

        changes.insert(
            uri,
            vec![lsp_types::TextEdit {
                range: Range {
                    start: Position {
                        line: params.text_document_position.position.line,
                        character: params.text_document_position.position.character,
                    },
                    end: Position {
                        line: params.text_document_position.position.line,
                        character: params.text_document_position.position.character + 5,
                    },
                },
                new_text: params.new_name.clone(),
            }],
        );

        let workspace_edit = WorkspaceEdit {
            changes: Some(changes),
            document_changes: None,
            change_annotations: None,
        };

        // Send the response
        self.send_response(self.next_id.load(Ordering::SeqCst), workspace_edit)?;

        Ok(())
    }

    /// Send diagnostics for a file
    pub fn send_diagnostics(&self, uri: Url, diagnostics: Vec<Diagnostic>) -> Result<()> {
        // Create the notification
        let params = lsp_types::PublishDiagnosticsParams {
            uri,
            diagnostics,
            version: None,
        };

        // Send the notification
        self.send_notification("textDocument/publishDiagnostics", params)?;

        Ok(())
    }

    /// Helper method to send a JSON-RPC response
    fn send_response<T: serde::Serialize>(&self, id: i32, result: T) -> Result<()> {
        let response = json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": result
        });

        self.send_message(&response)
    }

    /// Helper method to send a JSON-RPC notification
    fn send_notification<T: serde::Serialize>(&self, method: &str, params: T) -> Result<()> {
        let notification = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params
        });

        self.send_message(&notification)
    }

    /// Helper method to send a JSON-RPC message
    fn send_message(&self, message: &Value) -> Result<()> {
        let message_str = serde_json::to_string(message)?;
        let content_length = message_str.len();
        let header = format!("Content-Length: {}\r\n\r\n", content_length);

        let mut stdin = self.child.stdin.as_ref().unwrap();
        stdin.write_all(header.as_bytes())?;
        stdin.write_all(message_str.as_bytes())?;
        stdin.flush()?;

        debug!("[MOCK] Sent message: {}", message_str);

        Ok(())
    }

    /// Get all received messages
    pub fn get_received_messages(&self) -> Vec<String> {
        let messages = self.received_messages.lock().unwrap();
        messages.clone()
    }

    /// Create mock diagnostics
    pub fn create_mock_diagnostics() -> Vec<Diagnostic> {
        vec![
            Diagnostic {
                range: Range {
                    start: Position { line: 10, character: 5 },
                    end: Position { line: 10, character: 10 },
                },
                severity: Some(DiagnosticSeverity::ERROR),
                code: None,
                code_description: None,
                source: Some("mock-lsp".to_string()),
                message: "Mock error diagnostic".to_string(),
                related_information: None,
                tags: None,
                data: None,
            },
            Diagnostic {
                range: Range {
                    start: Position { line: 15, character: 2 },
                    end: Position { line: 15, character: 8 },
                },
                severity: Some(DiagnosticSeverity::WARNING),
                code: None,
                code_description: None,
                source: Some("mock-lsp".to_string()),
                message: "Mock warning diagnostic".to_string(),
                related_information: None,
                tags: None,
                data: None,
            },
        ]
    }

    /// Helper function to convert a path to a URI
    pub fn path_to_uri(path: &PathBuf) -> Url {
        Url::from_file_path(path).unwrap()
    }
}

impl Drop for MockLspServer {
    fn drop(&mut self) {
        // Try to kill the child process
        let _ = self.child.kill();
    }
}