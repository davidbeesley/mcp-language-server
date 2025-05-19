use crate::lsp::Client;
use anyhow::{Context, Result, anyhow};
use log::debug;
use lsp_types::{Hover, HoverContents, MarkedString, Position, TextDocumentPositionParams};
use std::path::PathBuf;

use super::utils::to_text_document_identifier;

/// Gets hover information for a position in a file
pub async fn get_hover_info(
    client: &Client,
    file_path: PathBuf,
    line: u32,
    column: u32,
) -> Result<String> {
    debug!(
        "[TOOL] Getting hover info for {}:{}:{}",
        file_path.display(),
        line,
        column
    );

    // Get the file's absolute path
    let file_path = file_path.canonicalize().context(format!(
        "Failed to canonicalize path: {}",
        file_path.display()
    ))?;

    // Ensure the file exists
    if !file_path.exists() {
        return Err(anyhow!("File does not exist: {}", file_path.display()));
    }

    // Ensure the file is open in the LSP server
    client.open_file(&file_path).await?;

    // Create position params (adjust from 1-indexed to 0-indexed)
    let line = line.saturating_sub(1);
    let column = column.saturating_sub(1);

    let position_params = TextDocumentPositionParams {
        text_document: to_text_document_identifier(&file_path)?,
        position: Position {
            line,
            character: column,
        },
    };

    // Call the LSP hover request
    let hover: Option<Hover> = client.call("textDocument/hover", position_params).await?;

    // Format the hover information
    match hover {
        Some(hover) => {
            let contents = match hover.contents {
                HoverContents::Scalar(content) => format_marked_string(&content),
                HoverContents::Array(contents) => {
                    let mut result = String::new();
                    for content in contents {
                        result.push_str(&format_marked_string(&content));
                        result.push_str("\n\n");
                    }
                    result
                }
                HoverContents::Markup(markup) => markup.value,
            };

            if contents.is_empty() {
                Ok("No hover information available at this position.".to_string())
            } else {
                Ok(contents)
            }
        }
        None => Ok("No hover information available at this position.".to_string()),
    }
}

/// Formats a MarkedString for display
fn format_marked_string(marked_string: &MarkedString) -> String {
    match marked_string {
        MarkedString::String(s) => s.clone(),
        MarkedString::LanguageString(ls) => {
            format!("```{}\n{}\n```", ls.language, ls.value)
        }
    }
}
