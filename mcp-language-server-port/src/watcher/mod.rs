pub mod gitignore;

use crate::lsp::Client;

use anyhow::{Context, Result, anyhow};
use async_trait::async_trait;
use log::{debug, error, info};
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use path_absolutize::Absolutize;
use std::{path::PathBuf, sync::Arc, time::Duration};
use tokio::sync::{broadcast, mpsc};

use self::gitignore::GitignoreFilter;

/// Interface for a workspace watcher
#[async_trait]
pub trait WorkspaceWatcher: Send + Sync {
    /// Start watching a workspace for changes
    async fn watch_workspace(&self, workspace_path: PathBuf) -> Result<()>;

    /// Stop watching
    async fn stop(&self) -> Result<()>;
}

/// FileSystemWatcher watches a workspace for file changes and notifies the LSP client
pub struct FileSystemWatcher {
    lsp_client: Arc<Client>,
    gitignore_filter: GitignoreFilter,
    watcher_tx: broadcast::Sender<WatcherCommand>,
}

#[derive(Clone)]
enum WatcherCommand {
    Stop,
}

impl FileSystemWatcher {
    /// Create a new FileSystemWatcher
    pub fn new(lsp_client: Arc<Client>, workspace_root: PathBuf) -> Self {
        let gitignore_filter = GitignoreFilter::new(workspace_root);
        let (watcher_tx, _) = broadcast::channel(10);

        Self {
            lsp_client,
            gitignore_filter,
            watcher_tx,
        }
    }

    /// Process a file change event
    async fn process_event(&self, event: Event) -> Result<()> {
        match event.kind {
            EventKind::Create(_) | EventKind::Modify(_) => {
                for path in event.paths {
                    if self.gitignore_filter.is_ignored(&path) {
                        continue;
                    }

                    if path.is_file() {
                        debug!("[WATCHER] File changed: {}", path.display());

                        // If the file is already open, notify the LSP client of the change
                        // Otherwise, just make sure the LSP server knows about it
                        let absolute_path = path.absolutize()?;
                        if self.lsp_client.is_file_open(&absolute_path) {
                            self.lsp_client.notify_change(&absolute_path).await?;
                        }
                    }
                }
            }
            EventKind::Remove(_) => {
                for path in event.paths {
                    if self.gitignore_filter.is_ignored(&path) {
                        continue;
                    }

                    debug!("[WATCHER] File removed: {}", path.display());

                    // If the file is open, close it
                    let absolute_path = path.absolutize()?;
                    if self.lsp_client.is_file_open(&absolute_path) {
                        self.lsp_client.close_file(&absolute_path).await?;
                    }
                }
            }
            _ => {}
        }

        Ok(())
    }
}

#[async_trait]
impl WorkspaceWatcher for FileSystemWatcher {
    async fn watch_workspace(&self, workspace_path: PathBuf) -> Result<()> {
        let workspace_path = workspace_path
            .absolutize()
            .context("Failed to absolutize workspace path")?;
        info!(
            "[WATCHER] Starting file watcher for workspace: {}",
            workspace_path.display()
        );

        // Create the event channel
        let (tx, mut rx) = mpsc::channel(100);

        // Create a new watcher
        let mut watcher = RecommendedWatcher::new(
            move |res| {
                let tx = tx.clone();
                if let Ok(event) = res {
                    let _ = tx.blocking_send(event);
                }
            },
            Config::default().with_poll_interval(Duration::from_secs(2)),
        )
        .context("Failed to create file watcher")?;

        // Start watching the workspace
        watcher
            .watch(&workspace_path, RecursiveMode::Recursive)
            .context("Failed to watch workspace")?;

        // Create clone for the watcher task
        let watcher_tx = self.watcher_tx.clone();
        let self_clone = Arc::new(self.clone());

        // Spawn a task to handle file change events
        tokio::spawn(async move {
            // Create a channel for the watcher commands
            let mut watcher_rx = watcher_tx.subscribe();

            loop {
                tokio::select! {
                    // Process file change events
                    Some(event) = rx.recv() => {
                        if let Err(e) = self_clone.process_event(event).await {
                            error!("[WATCHER] Error processing file event: {}", e);
                        }
                    }

                    // Process watcher commands
                    Ok(cmd) = watcher_rx.recv() => {
                        match cmd {
                            WatcherCommand::Stop => {
                                info!("[WATCHER] Stopping file watcher");
                                break;
                            }
                        }
                    }

                    // Exit if both channels are closed
                    else => break,
                }
            }

            // Drop the watcher to stop watching
            drop(watcher);
            info!("[WATCHER] File watcher stopped");
        });

        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        // Send stop command to the watcher task
        self.watcher_tx
            .send(WatcherCommand::Stop)
            .map_err(|e| anyhow!("Failed to send stop command to watcher: {}", e))?;

        Ok(())
    }
}

// Clone implementation for FileSystemWatcher
impl Clone for FileSystemWatcher {
    fn clone(&self) -> Self {
        Self {
            lsp_client: Arc::clone(&self.lsp_client),
            gitignore_filter: GitignoreFilter::new(self.gitignore_filter.workspace_root().clone()),
            watcher_tx: self.watcher_tx.clone(),
        }
    }
}
