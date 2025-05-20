use anyhow::{Result, anyhow};
use log::{debug, error};
use lsp_types::{
    Diagnostic, DiagnosticSeverity, Hover, HoverContents, InitializeParams, InitializeResult,
    InitializedParams, Location, MarkupContent, MarkupKind, Position, Range, ServerCapabilities,
    TextDocumentPositionParams, Url, WorkspaceEdit,
};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        atomic::{AtomicI32, Ordering},
        Arc, Mutex,
    },
};

/// A simplified mock LSP server for testing
pub struct MockLspServer {
    next_id: AtomicI32,
    received_messages: Arc<Mutex<Vec<String>>>,
}

impl MockLspServer {
    /// Start a new mock LSP server
    pub fn start() -> Result<Self> {
        // Create a simplified mock server that doesn't use actual processes
        let server = Self {
            next_id: AtomicI32::new(1),
            received_messages: Arc::new(Mutex::new(Vec::new())),
        };

        // Add standard initialization response
        let init_msg = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {}
        }).to_string();
        
        let mut messages = server.received_messages.lock().unwrap();
        messages.push(init_msg);
        
        Ok(server)
    }

    /// Send a mock initialize response
    pub fn handle_initialize(&self, id: Value, _params: InitializeParams) -> Result<()> {
        debug!("[MOCK] Handling initialize request");
        Ok(())
    }

    /// Handle initialized notification
    pub fn handle_initialized(&self, _params: InitializedParams) -> Result<()> {
        debug!("[MOCK] Handling initialized notification");
        Ok(())
    }

    /// Handle textDocument/hover request
    pub fn handle_hover(&self, id: Value, params: TextDocumentPositionParams) -> Result<()> {
        debug!("[MOCK] Handling hover request");
        
        // Record the hover response in the received messages
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
        
        // Create a response message
        let response = json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": hover
        });
        
        // Store it as if we received it
        let mut messages = self.received_messages.lock().unwrap();
        messages.push(response.to_string());
        
        Ok(())
    }

    /// Handle textDocument/definition request
    pub fn handle_definition(&self, id: Value, params: TextDocumentPositionParams) -> Result<()> {
        debug!("[MOCK] Handling definition request");
        
        // Create a mock location
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
        
        // Create a response message
        let response = json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": [location]
        });
        
        // Store it as if we received it
        let mut messages = self.received_messages.lock().unwrap();
        messages.push(response.to_string());
        
        Ok(())
    }

    /// Handle textDocument/references request
    pub fn handle_references(
        &self,
        id: Value,
        params: lsp_types::ReferenceParams,
    ) -> Result<()> {
        debug!("[MOCK] Handling references request");
        
        // Create mock locations
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
        
        // Create a response message
        let response = json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": locations
        });
        
        // Store it as if we received it
        let mut messages = self.received_messages.lock().unwrap();
        messages.push(response.to_string());
        
        Ok(())
    }

    /// Handle textDocument/rename request
    pub fn handle_rename(&self, id: Value, params: lsp_types::RenameParams) -> Result<()> {
        debug!("[MOCK] Handling rename request");
        
        // Create a mock workspace edit
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
        
        // Create a response message
        let response = json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": workspace_edit
        });
        
        // Store it as if we received it
        let mut messages = self.received_messages.lock().unwrap();
        messages.push(response.to_string());
        
        Ok(())
    }

    /// Send diagnostics for a file
    pub fn send_diagnostics(&self, uri: Url, diagnostics: Vec<Diagnostic>) -> Result<()> {
        debug!("[MOCK] Sending diagnostics notification");
        
        // Create the notification params
        let params = lsp_types::PublishDiagnosticsParams {
            uri,
            diagnostics,
            version: None,
        };
        
        // Create a notification message
        let notification = json!({
            "jsonrpc": "2.0",
            "method": "textDocument/publishDiagnostics",
            "params": params
        });
        
        // Store it as if we received it
        let mut messages = self.received_messages.lock().unwrap();
        messages.push(notification.to_string());
        
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
        Url::from_file_path(path).unwrap_or_else(|_| panic!("Failed to convert path to URI: {}", path.display()))
    }
}