use crate::lsp::Client;
use anyhow::{Context, Result, anyhow};
use log::debug;
use lsp_types::DiagnosticSeverity;
use std::path::PathBuf;
use tokio::fs;

use super::utils::to_uri;

/// Gets diagnostic information for a file
pub async fn get_diagnostics(
    client: &Client,
    file_path: PathBuf,
    context_lines: u32,
    show_line_numbers: bool,
) -> Result<String> {
    debug!(
        "[TOOL] Getting diagnostics for file: {}",
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

    // Ensure the file is open in the LSP server
    client.open_file(&file_path).await?;

    // Get the URI of the file
    let uri = to_uri(&file_path);

    // Get diagnostics for the file
    let diagnostics = client.get_diagnostics(&uri);

    if diagnostics.is_empty() {
        return Ok(format!("No diagnostics found for {}", file_path.display()));
    }

    // Read the file content
    let content = fs::read_to_string(&file_path)
        .await
        .context(format!("Failed to read file: {}", file_path.display()))?;

    // Split the content into lines
    let lines: Vec<&str> = content.lines().collect();

    // Format the diagnostics
    let mut result = String::new();

    result.push_str(&format!("Diagnostics for {}:\n\n", file_path.display()));

    for (i, diagnostic) in diagnostics.iter().enumerate() {
        // Add a separator between diagnostics
        if i > 0 {
            result.push_str("\n---\n\n");
        }

        // Get the severity as a string
        let severity_str = match diagnostic.severity {
            Some(DiagnosticSeverity::ERROR) => "Error",
            Some(DiagnosticSeverity::WARNING) => "Warning",
            Some(DiagnosticSeverity::INFORMATION) => "Info",
            Some(DiagnosticSeverity::HINT) => "Hint",
            None => "Unknown",
            _ => "Unknown", // Handle any other values
        };

        // Format the diagnostic
        result.push_str(&format!("{}: {}\n", severity_str, diagnostic.message));

        // Get the range of the diagnostic
        let range = &diagnostic.range;
        let start_line = range.start.line as usize;
        let end_line = range.end.line as usize;

        // Calculate the context range
        let context_start = start_line.saturating_sub(context_lines as usize);
        let context_end = std::cmp::min(end_line + context_lines as usize, lines.len() - 1);

        // Add code context
        result.push_str("\nCode context:\n");

        for line_num in context_start..=context_end {
            if line_num < lines.len() {
                let line_content = lines[line_num];

                // Add line number if requested
                if show_line_numbers {
                    result.push_str(&format!("{:5} | {}\n", line_num + 1, line_content));
                } else {
                    result.push_str(&format!("{}\n", line_content));
                }

                // Add a pointer to the exact position if this is the error line
                if line_num >= start_line && line_num <= end_line {
                    let start_char = if line_num == start_line {
                        range.start.character as usize
                    } else {
                        0
                    };
                    let end_char = if line_num == end_line {
                        range.end.character as usize
                    } else {
                        line_content.len()
                    };

                    // Create the pointer line
                    let prefix = if show_line_numbers { "      | " } else { "" };
                    let pointer = format!(
                        "{}{}{}\n",
                        prefix,
                        " ".repeat(start_char),
                        "^".repeat(end_char.saturating_sub(start_char).max(1))
                    );

                    result.push_str(&pointer);
                }
            }
        }
    }

    Ok(result)
}
