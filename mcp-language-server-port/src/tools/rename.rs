use crate::lsp::Client;
use anyhow::{Context, Result, anyhow};
use log::debug;
use lsp_types::{OneOf, Position, RenameParams, WorkspaceEdit};
use std::path::PathBuf;
use tokio::fs;

use super::utils::{to_path, to_text_document_identifier};

/// Renames a symbol across the workspace
pub async fn rename_symbol(
    client: &Client,
    file_path: PathBuf,
    line: u32,
    column: u32,
    new_name: String,
) -> Result<String> {
    debug!(
        "[TOOL] Renaming symbol at {}:{}:{} to '{}'",
        file_path.display(),
        line,
        column,
        &new_name
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

    // Create rename params (adjust from 1-indexed to 0-indexed)
    let line = line.saturating_sub(1);
    let column = column.saturating_sub(1);

    let rename_params = RenameParams {
        text_document_position: lsp_types::TextDocumentPositionParams {
            text_document: to_text_document_identifier(&file_path)?,
            position: Position {
                line,
                character: column,
            },
        },
        new_name,
        work_done_progress_params: Default::default(),
    };

    // Call the LSP rename request
    let edit: WorkspaceEdit = client.call("textDocument/rename", rename_params).await?;

    // Apply the edits
    let result = apply_workspace_edit(client, edit).await?;

    Ok(result)
}

/// Applies a workspace edit returned by the LSP server
async fn apply_workspace_edit(client: &Client, edit: WorkspaceEdit) -> Result<String> {
    let mut files_changed = 0;
    let mut edits_applied = 0;

    // Process changes
    if let Some(changes) = edit.changes {
        for (uri, edits) in changes {
            let file_path = to_path(&uri)?;

            // Read the file content
            let content = fs::read_to_string(&file_path)
                .await
                .context(format!("Failed to read file: {}", file_path.display()))?;

            // Apply the edits
            let mut new_content = content.clone();

            // Apply edits in reverse to avoid position changes
            for text_edit in edits.iter().rev() {
                // Convert the LSP positions to string indices
                let start_line = text_edit.range.start.line as usize;
                let start_char = text_edit.range.start.character as usize;
                let end_line = text_edit.range.end.line as usize;
                let end_char = text_edit.range.end.character as usize;

                // Split into lines
                let lines: Vec<&str> = new_content.lines().collect();

                // Calculate start and end indices
                let mut start_index = 0;
                for i in 0..start_line {
                    if i < lines.len() {
                        start_index += lines[i].len() + 1; // +1 for the newline
                    }
                }
                start_index += start_char;

                let mut end_index = 0;
                for i in 0..end_line {
                    if i < lines.len() {
                        end_index += lines[i].len() + 1; // +1 for the newline
                    }
                }
                end_index += end_char;

                // Apply the edit
                if start_index <= new_content.len() && end_index <= new_content.len() {
                    new_content = format!(
                        "{}{}{}",
                        &new_content[..start_index],
                        text_edit.new_text,
                        &new_content[end_index..],
                    );
                }

                edits_applied += 1;
            }

            // Write the changes back to the file
            fs::write(&file_path, &new_content)
                .await
                .context(format!("Failed to write file: {}", file_path.display()))?;

            // Notify the LSP server of the change
            client.notify_change(&file_path).await?;

            files_changed += 1;
        }
    }

    // Process document changes
    if let Some(document_changes) = edit.document_changes {
        match document_changes {
            lsp_types::DocumentChanges::Edits(edits) => {
                for text_document_edit in edits {
                    let uri = text_document_edit.text_document.uri;
                    let file_path = to_path(&uri)?;

                    // Read the file content
                    let content = fs::read_to_string(&file_path)
                        .await
                        .context(format!("Failed to read file: {}", file_path.display()))?;

                    // Apply the edits
                    let mut new_content = content.clone();

                    // Apply edits in reverse to avoid position changes
                    for text_edit in text_document_edit.edits.iter().rev() {
                        // Extract range and new_text based on the OneOf variant
                        let (range, new_text) = match text_edit {
                            OneOf::Left(edit) => (&edit.range, &edit.new_text),
                            OneOf::Right(annotated) => {
                                (&annotated.text_edit.range, &annotated.text_edit.new_text)
                            }
                        };

                        // Convert the LSP positions to string indices
                        let start_line = range.start.line as usize;
                        let start_char = range.start.character as usize;
                        let end_line = range.end.line as usize;
                        let end_char = range.end.character as usize;

                        // Split into lines
                        let lines: Vec<&str> = new_content.lines().collect();

                        // Calculate start and end indices
                        let mut start_index = 0;
                        for i in 0..start_line {
                            if i < lines.len() {
                                start_index += lines[i].len() + 1; // +1 for the newline
                            }
                        }
                        start_index += start_char;

                        let mut end_index = 0;
                        for i in 0..end_line {
                            if i < lines.len() {
                                end_index += lines[i].len() + 1; // +1 for the newline
                            }
                        }
                        end_index += end_char;

                        // Apply the edit
                        if start_index <= new_content.len() && end_index <= new_content.len() {
                            new_content = format!(
                                "{}{}{}",
                                &new_content[..start_index],
                                new_text,
                                &new_content[end_index..],
                            );
                        }

                        edits_applied += 1;
                    }

                    // Write the changes back to the file
                    fs::write(&file_path, &new_content)
                        .await
                        .context(format!("Failed to write file: {}", file_path.display()))?;

                    // Notify the LSP server of the change
                    client.notify_change(&file_path).await?;

                    files_changed += 1;
                }
            }
            lsp_types::DocumentChanges::Operations(_) => {
                // We don't support document operations yet
                return Err(anyhow!("Document operations are not supported"));
            }
        }
    }

    Ok(format!(
        "Applied {} edits across {} files",
        edits_applied, files_changed
    ))
}
