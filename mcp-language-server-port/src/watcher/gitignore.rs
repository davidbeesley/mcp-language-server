use ignore::gitignore::{Gitignore, GitignoreBuilder};
use log::{debug, error};
use std::path::{Path, PathBuf};

/// GitignoreFilter handles testing whether paths match patterns from gitignore files
pub struct GitignoreFilter {
    gitignore: Option<Gitignore>,
    workspace_root: PathBuf,
}

impl GitignoreFilter {
    /// Get a reference to the workspace root path
    pub fn workspace_root(&self) -> &PathBuf {
        &self.workspace_root
    }
}

impl GitignoreFilter {
    /// Create a new GitignoreFilter for the given workspace
    pub fn new(workspace_root: PathBuf) -> Self {
        let gitignore = Self::build_gitignore(&workspace_root);

        if gitignore.is_none() {
            debug!("[WATCHER] No .gitignore file found in workspace");
        }

        Self {
            gitignore,
            workspace_root,
        }
    }

    /// Check if a path should be ignored
    pub fn is_ignored(&self, path: &Path) -> bool {
        // Some paths should always be ignored
        let always_ignored = Self::is_always_ignored(path);
        if always_ignored {
            return true;
        }

        // Check gitignore rules
        if let Some(gitignore) = &self.gitignore {
            // Convert path to be relative to workspace root
            let rel_path = path.strip_prefix(&self.workspace_root).unwrap_or(path);

            // Check if path matches any gitignore patterns
            return matches!(gitignore.matched(rel_path, false), ignore::Match::Ignore(_));
        }

        // If no gitignore, don't ignore
        false
    }

    /// Build gitignore from .gitignore files in the workspace
    fn build_gitignore(workspace_root: &Path) -> Option<Gitignore> {
        let gitignore_path = workspace_root.join(".gitignore");

        if !gitignore_path.exists() {
            return None;
        }

        let mut builder = GitignoreBuilder::new(workspace_root);

        match builder.add(gitignore_path) {
            None => {}
            Some(e) => {
                error!("[WATCHER] Error parsing .gitignore: {}", e);
                return None;
            }
        }

        match builder.build() {
            Ok(gitignore) => Some(gitignore),
            Err(e) => {
                error!("[WATCHER] Error building gitignore: {}", e);
                None
            }
        }
    }

    /// Check if a path should always be ignored (e.g., .git directory)
    fn is_always_ignored(path: &Path) -> bool {
        let path_str = path.to_string_lossy();

        // Always ignore .git directory
        if path_str.contains("/.git/") || path_str.ends_with("/.git") {
            return true;
        }

        // Always ignore other common directories
        if path_str.contains("/node_modules/")
            || path_str.contains("/.venv/")
            || path_str.contains("/__pycache__/")
        {
            return true;
        }

        // Ignore backup files
        if path_str.ends_with('~') || path_str.contains(".bak") || path_str.contains(".swp") {
            return true;
        }

        false
    }
}
