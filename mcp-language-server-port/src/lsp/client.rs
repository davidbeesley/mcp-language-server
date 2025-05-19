use anyhow::{Context, Result, anyhow};
use log::{debug, error, info};
use lsp_types::{
    ClientCapabilities, CodeActionKind, InitializeParams, InitializeResult, InitializedParams,
    TextDocumentIdentifier, TextDocumentItem, Url, VersionedTextDocumentIdentifier,
    WorkspaceFolder,
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    io::{BufReader, BufWriter},
    path::Path,
    process::{Child, Command, Stdio},
    sync::{
        Arc, RwLock,
        atomic::{AtomicI32, Ordering},
    },
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, BufReader as TokioBufReader, BufWriter as TokioBufWriter},
    sync::{mpsc, oneshot},
};

use super::{
    protocol::{Message, MessageID},
    transport::write_message,
};

// Use Url as DocumentUri for compatibility with lsp-types
type DocumentUri = Url;

// Type aliases for handler functions
type NotificationHandler = Box<dyn Fn(Value) -> Result<()> + Send + Sync>;
type RequestHandler = Box<dyn Fn(Value) -> Result<Value> + Send + Sync>;

/// Represents an open file managed by the LSP server
#[derive(Debug, Clone)]
struct OpenFileInfo {
    version: i32,
    _uri: DocumentUri,
}

#[derive(Debug)]
enum ClientMessage {
    Request {
        id: MessageID,
        method: String,
        params: Value,
        response_tx: oneshot::Sender<Result<Value>>,
    },
    Notification {
        method: String,
        params: Value,
    },
    Shutdown,
}

/// Client for interacting with an LSP server
pub struct Client {
    // Child process management
    _child: Child,

    // Message routing
    next_id: AtomicI32,
    message_tx: mpsc::Sender<ClientMessage>,

    // State tracking
    open_files: RwLock<HashMap<String, OpenFileInfo>>,
    _diagnostics: RwLock<HashMap<DocumentUri, Vec<lsp_types::Diagnostic>>>,

    // Handlers for server requests and notifications
    notification_handlers: RwLock<HashMap<String, NotificationHandler>>,
    request_handlers: RwLock<HashMap<String, RequestHandler>>,
}

impl Client {
    /// Creates a new LSP client and starts the LSP server process
    pub async fn new(command: &str, args: &[String]) -> Result<Arc<Self>> {
        let mut child = Command::new(command)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context(format!("Failed to start LSP server: {}", command))?;

        // Get pipes to the child process
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow!("Failed to open stdin pipe"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("Failed to open stdout pipe"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| anyhow!("Failed to open stderr pipe"))?;

        // Create buffered readers and writers
        let _stdin_writer = BufWriter::new(stdin);
        let _stdout_reader = BufReader::new(stdout);

        // Create message channel
        let (tx, mut rx) = mpsc::channel::<ClientMessage>(100);

        // Create the client instance
        let client = Arc::new(Self {
            _child: child,
            next_id: AtomicI32::new(1),
            message_tx: tx,
            open_files: RwLock::new(HashMap::new()),
            _diagnostics: RwLock::new(HashMap::new()),
            notification_handlers: RwLock::new(HashMap::new()),
            request_handlers: RwLock::new(HashMap::new()),
        });

        // Handle stderr in a separate task
        let stderr = tokio::process::ChildStderr::from_std(stderr)
            .context("Failed to convert stderr to async")?;

        tokio::spawn(async move {
            let mut reader = tokio::io::BufReader::new(stderr);
            let mut buffer = Vec::new();
            let mut line = [0u8; 1024];

            loop {
                match reader.read(&mut line).await {
                    Ok(0) => break, // EOF
                    Ok(n) => {
                        buffer.extend_from_slice(&line[0..n]);

                        // Process complete lines
                        if let Some(pos) = buffer.iter().position(|&b| b == b'\n') {
                            let line_str = String::from_utf8_lossy(&buffer[0..pos]);
                            debug!("[TRANSPORT] LSP server stderr: {}", line_str);
                            buffer.drain(0..=pos);
                        }
                    }
                    Err(e) => {
                        error!("[TRANSPORT] Error reading from stderr: {}", e);
                        break;
                    }
                }
            }

            // Process any remaining data
            if !buffer.is_empty() {
                let line_str = String::from_utf8_lossy(&buffer);
                debug!("[TRANSPORT] LSP server stderr: {}", line_str);
            }
        });

        // Spawn a task to handle the message loop
        let client_ref = Arc::clone(&client);
        let stdin_writer = TokioBufWriter::new(tokio::io::sink());
        let stdout_reader = TokioBufReader::new(tokio::io::empty());
        tokio::spawn(async move {
            if let Err(e) = Client::message_loop(
                client_ref,
                &mut rx,
                &mut TokioBufReader::new(stdout_reader),
                &mut TokioBufWriter::new(stdin_writer),
            )
            .await
            {
                error!("[LSP] Message loop error: {}", e);
            }
        });

        Ok(client)
    }

    /// Initializes the LSP client with the given workspace directory
    pub async fn initialize(&self, workspace_dir: &Path) -> Result<InitializeResult> {
        let params = InitializeParams {
            process_id: Some(std::process::id()),
            root_uri: Some(to_uri(workspace_dir)),
            initialization_options: Some(json!({
                "codelenses": {
                    "generate": true,
                    "regenerate_cgo": true,
                    "test": true,
                    "tidy": true,
                    "upgrade_dependency": true,
                    "vendor": true,
                    "vulncheck": false,
                }
            })),

            capabilities: ClientCapabilities {
            workspace: Some(lsp_types::WorkspaceClientCapabilities {
                configuration: Some(true),
                did_change_configuration: Some(
                    lsp_types::DidChangeConfigurationClientCapabilities {
                        dynamic_registration: Some(true),
                    },
                ),
                did_change_watched_files: Some(
                    lsp_types::DidChangeWatchedFilesClientCapabilities {
                        dynamic_registration: Some(true),
                        relative_pattern_support: Some(true),
                    },
                ),
                workspace_folders: Some(true),
                ..Default::default()
            }),
            text_document: Some(lsp_types::TextDocumentClientCapabilities {
                synchronization: Some(lsp_types::TextDocumentSyncClientCapabilities {
                    dynamic_registration: Some(true),
                    did_save: Some(true),
                    ..Default::default()
                }),
                completion: Some(lsp_types::CompletionClientCapabilities {
                    dynamic_registration: Some(true),
                    completion_item: Some(lsp_types::CompletionItemCapability {
                        snippet_support: Some(true),
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
                hover: Some(lsp_types::HoverClientCapabilities {
                    dynamic_registration: Some(true),
                    ..Default::default()
                }),
                code_action: Some(lsp_types::CodeActionClientCapabilities {
                    dynamic_registration: Some(true),
                    code_action_literal_support: Some(lsp_types::CodeActionLiteralSupport {
                        code_action_kind: lsp_types::CodeActionKindLiteralSupport {
                            value_set: vec![
                                CodeActionKind::QUICKFIX.as_str().to_string(),
                                CodeActionKind::REFACTOR.as_str().to_string(),
                                CodeActionKind::SOURCE.as_str().to_string(),
                            ],
                        },
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
            },
            trace: Some(lsp_types::TraceValue::Off),
            workspace_folders: Some(vec![WorkspaceFolder {
                uri: to_uri(workspace_dir),
                name: workspace_dir
                    .file_name()
                    .map(|name| name.to_string_lossy().to_string())
                    .unwrap_or_else(|| "workspace".to_string()),
            }]),
            client_info: Some(lsp_types::ClientInfo {
                name: "mcp-language-server-rust".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
            ..Default::default()
        };

        let result: InitializeResult = self.call("initialize", params).await?;

        // Send initialized notification
        self.notify("initialized", InitializedParams {}).await?;

        // TODO: Register handlers for server requests and notifications

        info!("[LSP] LSP server initialized successfully");
        Ok(result)
    }

    /// Cleanly shuts down the LSP server
    pub async fn shutdown(&self) -> Result<()> {
        // First close all open files
        self.close_all_files().await?;

        // Send shutdown request
        let _: Value = self.call("shutdown", Value::Null).await?;

        // Send exit notification
        self.notify("exit", Value::Null).await?;

        // Signal the message loop to shut down
        let _ = self.message_tx.send(ClientMessage::Shutdown).await;

        info!("[LSP] LSP server shut down");
        Ok(())
    }

    /// Opens a file in the LSP server
    pub async fn open_file(&self, file_path: &Path) -> Result<()> {
        let uri = to_uri(file_path);
        let uri_str = uri.to_string();

        // Check if the file is already open
        {
            let open_files = self.open_files.read().unwrap();
            if open_files.contains_key(&uri_str) {
                return Ok(()); // Already open
            }
        }

        // Read the file content
        let content = tokio::fs::read_to_string(file_path)
            .await
            .context(format!("Failed to read file: {}", file_path.display()))?;

        // Send didOpen notification
        let params = lsp_types::DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: detect_language_id(file_path),
                version: 1,
                text: content,
            },
        };

        self.notify("textDocument/didOpen", params).await?;

        // Track the open file
        {
            let mut open_files = self.open_files.write().unwrap();
            open_files.insert(uri_str, OpenFileInfo { version: 1, _uri: uri });
        }

        debug!("[LSP] Opened file: {}", file_path.display());
        Ok(())
    }

    /// Notifies the LSP server of changes to a file
    pub async fn notify_change(&self, file_path: &Path) -> Result<()> {
        let uri = to_uri(file_path);
        let uri_str = uri.to_string();

        // Check if the file is open
        let version = {
            let mut open_files = self.open_files.write().unwrap();
            let file_info = open_files.get_mut(&uri_str).ok_or_else(|| {
                anyhow!(
                    "Cannot notify change for unopened file: {}",
                    file_path.display()
                )
            })?;

            // Increment version
            file_info.version += 1;
            file_info.version
        };

        // Read the file content
        let content = tokio::fs::read_to_string(file_path)
            .await
            .context(format!("Failed to read file: {}", file_path.display()))?;

        // Send didChange notification
        let params = lsp_types::DidChangeTextDocumentParams {
            text_document: VersionedTextDocumentIdentifier {
                uri: uri.clone(),
                version,
            },
            content_changes: vec![lsp_types::TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: content,
            }],
        };

        self.notify("textDocument/didChange", params).await?;

        debug!("[LSP] Notified change for file: {}", file_path.display());
        Ok(())
    }

    /// Closes a file in the LSP server
    pub async fn close_file(&self, file_path: &Path) -> Result<()> {
        let uri = to_uri(file_path);
        let uri_str = uri.to_string();

        // Check if the file is open
        {
            let open_files = self.open_files.read().unwrap();
            if !open_files.contains_key(&uri_str) {
                return Ok(()); // Already closed
            }
        }

        // Send didClose notification
        let params = lsp_types::DidCloseTextDocumentParams {
            text_document: TextDocumentIdentifier { uri },
        };

        self.notify("textDocument/didClose", params).await?;

        // Remove from open files
        {
            let mut open_files = self.open_files.write().unwrap();
            open_files.remove(&uri_str);
        }

        debug!("[LSP] Closed file: {}", file_path.display());
        Ok(())
    }

    /// Closes all open files
    pub async fn close_all_files(&self) -> Result<()> {
        let files_to_close = {
            let open_files = self.open_files.read().unwrap();
            open_files.keys().cloned().collect::<Vec<_>>()
        };

        for uri_str in files_to_close {
            // Convert URI back to file path
            if let Ok(uri) = uri_str.parse::<lsp_types::Url>() {
                if let Ok(file_path) = uri.to_file_path() {
                    if let Err(e) = self.close_file(&file_path).await {
                        error!("[LSP] Error closing file {}: {}", file_path.display(), e);
                    }
                }
            }
        }

        debug!("[LSP] Closed all files");
        Ok(())
    }

    /// Checks if a file is currently open in the LSP server
    pub fn is_file_open(&self, file_path: &Path) -> bool {
        let uri = to_uri(file_path);
        let uri_str = uri.to_string();

        let open_files = self.open_files.read().unwrap();
        open_files.contains_key(&uri_str)
    }

    /// Gets diagnostics for a file
    pub fn get_diagnostics(&self, uri: &DocumentUri) -> Vec<lsp_types::Diagnostic> {
        let diagnostics = self._diagnostics.read().unwrap();
        diagnostics.get(uri).cloned().unwrap_or_default()
    }

    /// Registers a handler for server notifications
    pub fn register_notification_handler<F>(&self, method: &str, handler: F)
    where
        F: Fn(Value) -> Result<()> + Send + Sync + 'static,
    {
        let mut handlers = self.notification_handlers.write().unwrap();
        handlers.insert(method.to_string(), Box::new(handler));
    }

    /// Registers a handler for server requests
    pub fn register_request_handler<F>(&self, method: &str, handler: F)
    where
        F: Fn(Value) -> Result<Value> + Send + Sync + 'static,
    {
        let mut handlers = self.request_handlers.write().unwrap();
        handlers.insert(method.to_string(), Box::new(handler));
    }

    /// Calls an LSP method and returns the result
    pub async fn call<P, R>(&self, method: &str, params: P) -> Result<R>
    where
        P: Serialize + Send + Sync,
        R: DeserializeOwned + Send + Sync,
    {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let id = MessageID::Number(id);

        let params_value = serde_json::to_value(params)?;

        // Create a channel for the response
        let (tx, rx) = oneshot::channel();

        // Send the request
        self.message_tx
            .send(ClientMessage::Request {
                id: id.clone(),
                method: method.to_string(),
                params: params_value,
                response_tx: tx,
            })
            .await?;

        // Wait for the response
        let result = rx.await?;

        // Convert the result
        match result {
            Ok(value) => {
                let result = serde_json::from_value(value)?;
                Ok(result)
            }
            Err(e) => Err(e),
        }
    }

    /// Sends a notification to the LSP server
    pub async fn notify<P>(&self, method: &str, params: P) -> Result<()>
    where
        P: Serialize + Send + Sync,
    {
        let params_value = serde_json::to_value(params)?;

        // Send the notification
        self.message_tx
            .send(ClientMessage::Notification {
                method: method.to_string(),
                params: params_value,
            })
            .await?;

        Ok(())
    }

    // Private methods

    /// Handles messages from the LSP server
    async fn message_loop<R, W>(
        client: Arc<Client>,
        rx: &mut mpsc::Receiver<ClientMessage>,
        _reader: &mut R,
        writer: &mut W,
    ) -> Result<()>
    where
        R: AsyncReadExt + Unpin,
        W: AsyncWriteExt + Unpin,
    {
        // Maps message IDs to response channels
        let mut response_channels: HashMap<String, oneshot::Sender<Result<Value>>> = HashMap::new();

        // Split the processing into two tasks: one for reading from the LSP server,
        // and one for writing to it
        let (_msg_tx, mut msg_rx) = mpsc::channel::<Message>(100);

        // Spawn a task to read messages from the server
        let read_task = tokio::spawn(async move {
            // Implementation of reading from the server will go here
            // It will receive messages, process them, and send responses when needed
            Ok::<_, anyhow::Error>(())
        });

        // Process messages from both channels: the client and the server
        loop {
            tokio::select! {
                // Handle messages from the client
                Some(client_msg) = rx.recv() => {
                    match client_msg {
                        ClientMessage::Request { id, method, params, response_tx } => {
                            // Create an LSP request message
                            let msg = Message {
                                jsonrpc: "2.0".to_string(),
                                id: Some(id.clone()),
                                method: Some(method.clone()),
                                params: Some(params),
                                result: None,
                                error: None,
                            };

                            // Store the response channel
                            response_channels.insert(id.to_string(), response_tx);

                            // Send the message to the server
                            write_message(writer, &msg).await?;
                        }
                        ClientMessage::Notification { method, params } => {
                            // Create an LSP notification message
                            let msg = Message {
                                jsonrpc: "2.0".to_string(),
                                id: None,
                                method: Some(method),
                                params: Some(params),
                                result: None,
                                error: None,
                            };

                            // Send the message to the server
                            write_message(writer, &msg).await?;
                        }
                        ClientMessage::Shutdown => {
                            // Clean shutdown
                            break;
                        }
                    }
                }

                // Handle messages from the server
                Some(server_msg) = msg_rx.recv() => {
                    if let Some(id) = &server_msg.id {
                        // This is a response to one of our requests
                        if let Some(tx) = response_channels.remove(&id.to_string()) {
                            if let Some(error) = server_msg.error {
                                // Send the error to the waiting task
                                let _ = tx.send(Err(anyhow!("LSP error: {} (code: {})", error.message, error.code)));
                            } else if let Some(result) = server_msg.result {
                                // Send the result to the waiting task
                                let _ = tx.send(Ok(result));
                            } else {
                                // No result or error
                                let _ = tx.send(Err(anyhow!("LSP response has neither result nor error")));
                            }
                        }
                    } else if let Some(method) = &server_msg.method {
                        // This is a server-to-client request or notification
                        if server_msg.id.is_some() {
                            // This is a request
                            let method_name = method.clone();
                            let id = server_msg.id.clone().unwrap();
                            let params = server_msg.params.clone().unwrap_or(Value::Null);

                            // Look up handler
                            let handler_result = {
                                let handlers = client.request_handlers.read().unwrap();
                                if let Some(handler) = handlers.get(&method_name) {
                                    handler(params)
                                } else {
                                    Err(anyhow!("No handler for request method: {}", method_name))
                                }
                            };

                            // Create response message
                            let response = match handler_result {
                                Ok(result) => Message {
                                    jsonrpc: "2.0".to_string(),
                                    id: Some(id),
                                    method: None,
                                    params: None,
                                    result: Some(result),
                                    error: None,
                                },
                                Err(e) => Message {
                                    jsonrpc: "2.0".to_string(),
                                    id: Some(id),
                                    method: None,
                                    params: None,
                                    result: None,
                                    error: Some(super::protocol::ResponseError {
                                        code: -32603, // Internal error
                                        message: e.to_string(),
                                    }),
                                },
                            };

                            // Send response back to server
                            write_message(writer, &response).await?;
                        } else {
                            // This is a notification
                            let method_name = method.clone();
                            let params = server_msg.params.clone().unwrap_or(Value::Null);

                            // Look up handler
                            let handlers = client.notification_handlers.read().unwrap();
                            if let Some(handler) = handlers.get(&method_name) {
                                if let Err(e) = handler(params) {
                                    error!("[LSP] Error handling notification {}: {}", method_name, e);
                                }
                            } else {
                                debug!("[LSP] No handler for notification: {}", method_name);
                            }
                        }
                    }
                }

                // If both channels are closed, break the loop
                else => break,
            }
        }

        // Cancel the read task
        read_task.abort();

        info!("[LSP] Message loop terminated");
        Ok(())
    }
}

/// Converts a path to an LSP URI
fn to_uri(path: &Path) -> DocumentUri {
    lsp_types::Url::from_file_path(path)
        .unwrap_or_else(|_| panic!("Failed to convert path to URI: {}", path.display()))
}

/// Detects the language ID for a file based on its extension
fn detect_language_id(path: &Path) -> String {
    match path.extension().and_then(|e| e.to_str()) {
        Some("rs") => "rust",
        Some("go") => "go",
        Some("js") => "javascript",
        Some("ts") => "typescript",
        Some("py") => "python",
        Some("java") => "java",
        Some("c") | Some("h") => "c",
        Some("cpp") | Some("hpp") | Some("cc") => "cpp",
        Some("json") => "json",
        Some("md") => "markdown",
        Some("html") => "html",
        Some("css") => "css",
        _ => "plaintext",
    }
    .to_string()
}
