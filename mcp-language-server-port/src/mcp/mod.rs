use anyhow::Result;
use rmcp::model::ServerInfo;
use rmcp::{ServerHandler, tool};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;

use crate::lsp;
use crate::tools;

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct EditFileRequest {
    #[schemars(description = "Path to the file to edit")]
    pub file_path: String,
    #[schemars(description = "List of text edits to apply")]
    pub edits: Vec<tools::edit::TextEditParams>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DefinitionRequest {
    #[schemars(description = "The symbol name to find definition for")]
    pub symbol_name: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ReferencesRequest {
    #[schemars(description = "The symbol name to find references for")]
    pub symbol_name: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DiagnosticsRequest {
    #[schemars(description = "Path to the file to get diagnostics for")]
    pub file_path: String,
    #[schemars(description = "Number of context lines to show around diagnostics")]
    pub context_lines: Option<u32>,
    #[schemars(description = "Show line numbers in the output")]
    pub show_line_numbers: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct HoverRequest {
    #[schemars(description = "Path to the file")]
    pub file_path: String,
    #[schemars(description = "Line number (0-based)")]
    pub line: u32,
    #[schemars(description = "Column number (0-based)")]
    pub column: u32,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RenameRequest {
    #[schemars(description = "Path to the file")]
    pub file_path: String,
    #[schemars(description = "Line number (0-based)")]
    pub line: u32,
    #[schemars(description = "Column number (0-based)")]
    pub column: u32,
    #[schemars(description = "New name for the symbol")]
    pub new_name: String,
}

/// MCP Server implementation with LSP backend
#[derive(Clone)]
pub struct McpLanguageServer {
    lsp_client: Arc<lsp::Client>,
    workspace_dir: std::path::PathBuf,
}

impl std::fmt::Debug for McpLanguageServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpLanguageServer")
            .field("lsp_client", &"<LSP Client>")
            .field("workspace_dir", &self.workspace_dir)
            .finish()
    }
}

impl McpLanguageServer {
    pub fn new(lsp_client: Arc<lsp::Client>, workspace_dir: std::path::PathBuf) -> Self {
        Self {
            lsp_client,
            workspace_dir,
        }
    }
}

// Create a toolbox for our tools
#[tool(tool_box)]
impl McpLanguageServer {
    #[tool(description = "Edit a file by applying text edits")]
    async fn edit_file(&self, #[tool(aggr)] request: EditFileRequest) -> String {
        let path = Path::new(&request.file_path).to_path_buf();
        match tools::apply_text_edits(&self.lsp_client, path, request.edits).await {
            Ok(result) => result,
            Err(e) => format!("Error editing file: {}", e),
        }
    }

    #[tool(description = "Find the definition of a symbol")]
    async fn definition(&self, #[tool(aggr)] request: DefinitionRequest) -> String {
        match tools::find_definition(&self.lsp_client, &request.symbol_name).await {
            Ok(result) => result,
            Err(e) => format!("Error finding definition: {}", e),
        }
    }

    #[tool(description = "Find all references to a symbol")]
    async fn references(&self, #[tool(aggr)] request: ReferencesRequest) -> String {
        match tools::find_references(&self.lsp_client, &request.symbol_name).await {
            Ok(result) => result,
            Err(e) => format!("Error finding references: {}", e),
        }
    }

    #[tool(description = "Get diagnostics for a file")]
    async fn diagnostics(&self, #[tool(aggr)] request: DiagnosticsRequest) -> String {
        let path = Path::new(&request.file_path).to_path_buf();
        let context_lines = request.context_lines.unwrap_or(5);
        let show_line_numbers = request.show_line_numbers.unwrap_or(true);

        match tools::get_diagnostics(&self.lsp_client, path, context_lines, show_line_numbers).await
        {
            Ok(result) => result,
            Err(e) => format!("Error getting diagnostics: {}", e),
        }
    }

    #[tool(description = "Get hover information at a specific position")]
    async fn hover(&self, #[tool(aggr)] request: HoverRequest) -> String {
        let path = Path::new(&request.file_path).to_path_buf();
        match tools::get_hover_info(&self.lsp_client, path, request.line, request.column).await {
            Ok(result) => result,
            Err(e) => format!("Error getting hover info: {}", e),
        }
    }

    #[tool(description = "Rename a symbol at a specific position")]
    async fn rename_symbol(&self, #[tool(aggr)] request: RenameRequest) -> String {
        let path = Path::new(&request.file_path).to_path_buf();
        match tools::rename_symbol(
            &self.lsp_client,
            path,
            request.line,
            request.column,
            request.new_name,
        )
        .await
        {
            Ok(result) => result,
            Err(e) => format!("Error renaming symbol: {}", e),
        }
    }
}

// Implement the ServerHandler trait for MCP
#[tool(tool_box)]
impl ServerHandler for McpLanguageServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some("A Model Context Protocol server that proxies requests to Language Server Protocol servers, providing LLM-friendly access to language server features like code navigation, diagnostics, and refactoring.".to_string()),
            ..Default::default()
        }
    }
}
