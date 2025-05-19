use anyhow::{Context, Result, anyhow};
use lsp_types::{Position, Range, TextDocumentIdentifier, TextDocumentPositionParams};
use path_absolutize::Absolutize;
use std::path::{Path, PathBuf};

/// Converts a file path to an LSP URI
pub fn to_uri(path: &Path) -> lsp_types::Url {
    lsp_types::Url::from_file_path(path)
        .unwrap_or_else(|_| panic!("Failed to convert path to URI: {}", path.display()))
}

/// Converts an LSP URI to a file path
pub fn to_path(uri: &lsp_types::Url) -> Result<PathBuf> {
    uri.to_file_path()
        .map_err(|_| anyhow!("Failed to convert URI to path: {}", uri))
}

/// Creates a TextDocumentIdentifier from a file path
pub fn to_text_document_identifier(file_path: &Path) -> Result<TextDocumentIdentifier> {
    let abs_path = file_path
        .absolutize()
        .context("Failed to absolutize path")?;

    Ok(TextDocumentIdentifier {
        uri: to_uri(&abs_path),
    })
}

/// Creates a TextDocumentPositionParams from a file path and position
pub fn to_text_document_position(
    file_path: &Path,
    line: u32,
    character: u32,
) -> Result<TextDocumentPositionParams> {
    Ok(TextDocumentPositionParams {
        text_document: to_text_document_identifier(file_path)?,
        position: Position { line, character },
    })
}

/// Creates a Range from line and character positions
pub fn to_range(start_line: u32, start_char: u32, end_line: u32, end_char: u32) -> Range {
    Range {
        start: Position {
            line: start_line,
            character: start_char,
        },
        end: Position {
            line: end_line,
            character: end_char,
        },
    }
}

/// Formats code with syntax highlighting
pub fn format_code(code: &str, language: &str) -> String {
    // Simple formatting for now
    format!("```{}\n{}\n```", language, code)
}

/// Extracts a language from a file path
pub fn get_language_from_path(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("rs") => "rust",
        Some("go") => "go",
        Some("js") => "javascript",
        Some("ts") => "typescript",
        Some("jsx") => "jsx",
        Some("tsx") => "tsx",
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
}

/// Creates a formatted error message
pub fn format_error(message: &str) -> String {
    format!("Error: {}", message)
}
