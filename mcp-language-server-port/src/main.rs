use anyhow::{Context, Result, anyhow};
use clap::Parser;
use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tokio::signal::ctrl_c;
use tokio::sync::mpsc;

mod logging;
mod lsp;
mod mcp;
mod tools;
mod watcher;

use crate::watcher::{FileSystemWatcher, WorkspaceWatcher};
use log::info;

#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = "MCP Language Server: A proxy server for language servers"
)]
struct Config {
    /// Path to workspace directory
    #[arg(long)]
    workspace: PathBuf,

    /// LSP command to run
    #[arg(long)]
    lsp: String,

    /// Additional args to pass to LSP command
    #[arg(last = true)]
    lsp_args: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    logging::debug();

    info!("MCP Language Server starting");

    // Parse command-line arguments
    let config = Config::parse();

    // Validate workspace path
    if !config.workspace.exists() {
        log::error!(
            "Workspace directory does not exist: {}",
            config.workspace.display()
        );
        return Err(anyhow!("Workspace directory does not exist"));
    }

    // Create a shutdown channel
    let shutdown_flag = Arc::new(AtomicBool::new(false));
    let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);

    // Create a clone for the signal handler
    let shutdown_flag_clone = Arc::clone(&shutdown_flag);

    // Handle shutdown signals
    tokio::spawn(async move {
        let _ = ctrl_c().await;
        info!("Received shutdown signal");
        shutdown_flag_clone.store(true, Ordering::SeqCst);
        let _ = shutdown_tx.send(()).await;
    });

    // Create LSP client
    info!(
        "Starting LSP client: {} {}",
        &config.lsp,
        config.lsp_args.join(" ")
    );

    let lsp_client = lsp::Client::new(&config.lsp, &config.lsp_args)
        .await
        .context("Failed to create LSP client")?;

    // Initialize the LSP client
    info!("Initializing LSP client");

    lsp_client
        .initialize(&config.workspace)
        .await
        .context("Failed to initialize LSP client")?;

    // Create file watcher
    let workspace_watcher =
        FileSystemWatcher::new(Arc::clone(&lsp_client), config.workspace.clone());

    // Start watching the workspace
    workspace_watcher
        .watch_workspace(config.workspace.clone())
        .await
        .context("Failed to start workspace watcher")?;

    // Create MCP server handler
    let server_handler =
        mcp::McpLanguageServer::new(Arc::clone(&lsp_client), config.workspace.clone());

    // Create the MCP server with stdin/stdout transport
    let transport = (tokio::io::stdin(), tokio::io::stdout());

    // Start the MCP server
    let server_handle = tokio::spawn(async move {
        match rmcp::serve_server(server_handler, transport).await {
            Ok(server) => {
                info!("MCP server running");
                let _ = server.waiting().await;
            }
            Err(e) => {
                log::error!("Failed to start MCP server: {}", e);
            }
        }
    });

    info!("MCP server initialized and ready");

    // Wait for shutdown signal or server completion
    tokio::select! {
        _ = shutdown_rx.recv() => {
            info!("Received shutdown signal, initiating clean shutdown");
        }
        _ = server_handle => {
            info!("MCP server completed");
        }
    }

    // Clean shutdown
    info!("Shutting down workspace watcher");
    let _ = workspace_watcher.stop().await;

    info!("Shutting down LSP client");
    let _ = lsp_client.shutdown().await;

    info!("Server shutdown complete");
    Ok(())
}
