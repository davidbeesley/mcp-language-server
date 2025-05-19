use crate::lsp::Client;
use anyhow::{Context, Result, anyhow};
use log::debug;
use lsp_types::{Location, Position, ReferenceContext, ReferenceParams};
use std::{collections::HashMap, path::PathBuf};
use tokio::fs;

use super::definition::parse_symbol_location;
use super::utils::{to_path, to_text_document_identifier};

/// Finds all references to a symbol
pub async fn find_references(client: &Client, symbol_name: &str) -> Result<String> {
    debug!("[TOOL] Finding references for symbol: {}", symbol_name);

    // Parse the symbol location
    let (file_path, line, column) = parse_symbol_location(symbol_name)?;

    // Ensure the file is open
    client.open_file(&file_path).await?;

    // Create reference params
    let reference_params = ReferenceParams {
        text_document_position: lsp_types::TextDocumentPositionParams {
            text_document: to_text_document_identifier(&file_path)?,
            position: Position {
                line,
                character: column,
            },
        },
        context: ReferenceContext {
            include_declaration: true,
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };

    // Call the LSP references request
    let locations: Vec<Location> = client
        .call("textDocument/references", reference_params)
        .await?;

    if locations.is_empty() {
        return Err(anyhow!("No references found for symbol: {}", symbol_name));
    }

    // Group references by file
    let mut references_by_file: HashMap<PathBuf, Vec<Location>> = HashMap::new();

    for location in locations {
        let file_path = to_path(&location.uri)?;
        references_by_file
            .entry(file_path)
            .or_default()
            .push(location);
    }

    // For each file, get the content and format the references
    let mut result = String::new();

    // Add summary line
    let reference_count = references_by_file
        .values()
        .map(|locs| locs.len())
        .sum::<usize>();
    result.push_str(&format!(
        "Found {} references to '{}' in {} files:\n\n",
        reference_count,
        symbol_name,
        references_by_file.len()
    ));

    for (file_path, locations) in references_by_file {
        result.push_str(&format!("File: {}\n", file_path.display()));

        // Read the file content
        let content = fs::read_to_string(&file_path)
            .await
            .context(format!("Failed to read file: {}", file_path.display()))?;

        let lines: Vec<&str> = content.lines().collect();

        // For each location, extract the line containing the reference
        for location in locations {
            let line_num = location.range.start.line as usize;
            let col_num = location.range.start.character as usize;

            if line_num < lines.len() {
                let line_content = lines[line_num];

                // Format the line with the reference
                result.push_str(&format!("  Line {}: {}\n", line_num + 1, line_content));

                // Add a pointer to the exact position
                let pointer = format!("  {}{}\n", " ".repeat(col_num + 7), "^");
                result.push_str(&pointer);
            }
        }

        result.push('\n');
    }

    Ok(result)
}
