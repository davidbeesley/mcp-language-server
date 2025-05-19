use crate::lsp::Client;
use anyhow::{Context, Result, anyhow};
use log::{debug, error};
use lsp_types::{Location, Position, TextDocumentPositionParams};
use serde_json::Value;
use std::path::PathBuf;
use tokio::fs;

use super::utils::{format_code, get_language_from_path, to_path, to_text_document_identifier};

/// Finds the definition of a symbol in a file
pub async fn find_definition(client: &Client, symbol_name: &str) -> Result<String> {
    debug!("[TOOL] Finding definition for symbol: {}", symbol_name);

    // We need to first find a file where the symbol is used
    // For now, let's assume the symbol_name is a file path and line/column
    // in the format "path:line:column" or just the symbol name for a global search

    let (file_path, line, column) = parse_symbol_location(symbol_name)?;

    // Ensure the file is open
    client.open_file(&file_path).await?;

    // Create position params
    let position_params = TextDocumentPositionParams {
        text_document: to_text_document_identifier(&file_path)?,
        position: Position {
            line,
            character: column,
        },
    };

    // Call the LSP definition request
    let definition: Value = client
        .call("textDocument/definition", position_params)
        .await?;

    // Parse the result (could be a Location or an array of Locations)
    let locations = parse_definition_result(definition)?;

    if locations.is_empty() {
        return Err(anyhow!("Definition not found for symbol: {}", symbol_name));
    }

    // For each location, get the content
    let mut result = String::new();

    for location in &locations {
        let file_path = to_path(&location.uri)?;

        // Read the file content
        let content = fs::read_to_string(&file_path)
            .await
            .context(format!("Failed to read file: {}", file_path.display()))?;

        // Extract the relevant part using the range
        let lines: Vec<&str> = content.lines().collect();
        let start_line = location.range.start.line as usize;
        let end_line = location.range.end.line as usize;

        // Get the code snippet
        let mut code_snippet = String::new();
        for i in start_line..=end_line {
            if i < lines.len() {
                code_snippet.push_str(lines[i]);
                code_snippet.push('\n');
            }
        }

        // Format the result
        let language = get_language_from_path(&file_path);
        let formatted_code = format_code(&code_snippet, language);

        result.push_str(&format!(
            "Definition found in {}:{}:{}\n\n{}\n\n",
            file_path.display(),
            start_line + 1, // 1-indexed for display
            location.range.start.character + 1,
            formatted_code
        ));
    }

    Ok(result)
}

/// Parse a symbol location string in the format "path:line:column" or just "symbol"
pub fn parse_symbol_location(symbol_location: &str) -> Result<(PathBuf, u32, u32)> {
    // Check if the symbol_location contains line and column information
    let parts: Vec<&str> = symbol_location.split(':').collect();

    if parts.len() >= 3 {
        // Format is path:line:column
        let path = PathBuf::from(parts[0]);
        let line = parts[1]
            .parse::<u32>()
            .context("Failed to parse line number")?;
        let column = parts[2]
            .parse::<u32>()
            .context("Failed to parse column number")?;

        // Convert to 0-indexed
        let line = line.saturating_sub(1);
        let column = column.saturating_sub(1);

        Ok((path, line, column))
    } else {
        // For now, just return an error if we don't have line/column
        Err(anyhow!(
            "Symbol location must be in the format 'path:line:column'"
        ))
    }
}

/// Parse the LSP definition result into a list of Locations
fn parse_definition_result(value: Value) -> Result<Vec<Location>> {
    match value {
        Value::Array(array) => {
            let mut locations = Vec::new();

            for item in array {
                match serde_json::from_value::<Location>(item) {
                    Ok(location) => locations.push(location),
                    Err(e) => error!("[TOOL] Failed to parse location: {}", e),
                }
            }

            Ok(locations)
        }
        Value::Object(_) => {
            // Single location
            match serde_json::from_value::<Location>(value) {
                Ok(location) => Ok(vec![location]),
                Err(e) => Err(anyhow!("Failed to parse location: {}", e)),
            }
        }
        _ => Err(anyhow!("Unexpected definition result format")),
    }
}
