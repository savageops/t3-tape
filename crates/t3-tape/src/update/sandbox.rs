use std::path::PathBuf;

use crate::store::paths::ResolvedPaths;
use crate::store::time;

use super::triage::SandboxSummary;

#[derive(Debug, Clone)]
pub struct SandboxContext {
    pub timestamp: String,
    pub root: PathBuf,
    pub triage_path: PathBuf,
    pub resolved_dir: PathBuf,
    pub preview_dir: PathBuf,
    pub worktree_path: PathBuf,
    pub branch: String,
}

impl SandboxContext {
    pub fn new(paths: &ResolvedPaths) -> Self {
        let pid = std::process::id();
        let timestamp = format!("{}-{pid}", time::current_utc_compact_timestamp_micros());
        let root = paths.sandbox_dir.join(&timestamp);
        let triage_path = root.join("triage.json");
        let resolved_dir = root.join("resolved");
        let preview_dir = root.join("preview");
        let worktree_path = root.join("worktree");
        let branch = format!("t3-tape/migrate/{timestamp}");

        Self {
            timestamp,
            root,
            triage_path,
            resolved_dir,
            preview_dir,
            worktree_path,
            branch,
        }
    }

    pub fn summary(&self) -> SandboxSummary {
        SandboxSummary {
            path: self.root.display().to_string(),
            worktree_branch: self.branch.clone(),
            worktree_path: self.worktree_path.display().to_string(),
        }
    }
}
