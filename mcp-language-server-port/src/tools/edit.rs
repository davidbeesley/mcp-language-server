use crate::lsp::Client;
use anyhow::{Context, Result, anyhow};
use log::debug;
use lsp_types::TextEdit;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::fs;

/// Parameters for a text edit operation
#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
pub struct TextEditParams {
    #[schemars(description = "Start line to replace (1-indexed)")]
    pub start_line: u32,

    #[schemars(description = "End line to replace (1-indexed)")]
    pub end_line: u32,

    #[schemars(description = "New text to insert")]
    pub new_text: String,
}

/// Applies a set of text edits to a file
pub async fn apply_text_edits(
    client: &Client,
    file_path: PathBuf,
    edits: Vec<TextEditParams>,
) -> Result<String> {
    debug!(
        "[TOOL] Applying {} text edits to {}",
        edits.len(),
        file_path.display()
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

    // Read the file content
    let content = fs::read_to_string(&file_path)
        .await
        .context(format!("Failed to read file: {}", file_path.display()))?;

    // Split the content into lines
    let lines: Vec<&str> = content.lines().collect();

    // Ensure the file is open in the LSP server
    client.open_file(&file_path).await?;

    // Convert edits to LSP TextEdit format
    let lsp_edits: Vec<TextEdit> = edits
        .iter()
        .map(|edit| {
            // LSP positions are 0-indexed, but our parameters are 1-indexed
            let start_line = edit.start_line - 1;
            let end_line = edit.end_line - 1;

            // Calculate the start character (start of the line)
            let start_character = 0;

            // Calculate the end character (end of the line)
            let end_character = if end_line < lines.len() as u32 {
                lines[end_line as usize].len() as u32
            } else {
                0
            };

            // Create the LSP TextEdit
            TextEdit {
                range: lsp_types::Range {
                    start: lsp_types::Position {
                        line: start_line,
                        character: start_character,
                    },
                    end: lsp_types::Position {
                        line: end_line,
                        character: end_character,
                    },
                },
                new_text: edit.new_text.clone(),
            }
        })
        .collect();

    // Apply the edits to the in-memory content
    let mut result = content.clone();

    // Apply edits in reverse to avoid position changes
    for edit in lsp_edits.iter().rev() {
        // Convert the LSP positions to string indices
        let start_index = position_to_index(&content, edit.range.start)?;
        let end_index = position_to_index(&content, edit.range.end)?;

        // Apply the edit
        result = format!(
            "{}{}{}",
            &result[..start_index],
            edit.new_text,
            &result[end_index..],
        );
    }

    // Write the result back to the file
    fs::write(&file_path, &result)
        .await
        .context(format!("Failed to write file: {}", file_path.display()))?;

    // Notify the LSP server of the change
    client.notify_change(&file_path).await?;

    debug!(
        "[TOOL] Successfully applied edits to {}",
        file_path.display()
    );

    Ok(format!(
        "Successfully applied {} edits to {}",
        edits.len(),
        file_path.display()
    ))
}

/// Converts an LSP Position to a string index
fn position_to_index(content: &str, position: lsp_types::Position) -> Result<usize> {
    let lines: Vec<&str> = content.lines().collect();

    // Check if the position is valid
    if position.line as usize >= lines.len() {
        return Err(anyhow!("Invalid line number: {}", position.line));
    }

    // Calculate the index
    let mut index = 0;

    // Add the length of all lines before the position
    for line in lines.iter().take(position.line as usize) {
        index += line.len() + 1; // +1 for the newline character
    }

    // Add the character offset
    let line_len = lines[position.line as usize].len();
    let char_offset = std::cmp::min(position.character as usize, line_len);
    index += char_offset;

    Ok(index)
}
