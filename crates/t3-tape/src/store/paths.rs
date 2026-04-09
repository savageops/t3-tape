use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::exit::RedtapeError;

#[derive(Debug, Clone, Default)]
pub struct ResolveOptions {
    pub repo_root_override: Option<PathBuf>,
    pub state_dir_override: Option<PathBuf>,
    pub cwd: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedPaths {
    pub repo_root: PathBuf,
    pub state_dir: PathBuf,
    pub patches_dir: PathBuf,
    pub sandbox_dir: PathBuf,
    pub config_path: PathBuf,
    pub patch_md_path: PathBuf,
    pub migration_log_path: PathBuf,
    pub triage_path: PathBuf,
}

impl ResolvedPaths {
    pub fn new(repo_root: PathBuf, state_dir: PathBuf) -> Self {
        let patches_dir = state_dir.join("patches");
        let sandbox_dir = state_dir.join("sandbox");
        let config_path = state_dir.join("config.json");
        let patch_md_path = state_dir.join("patch.md");
        let migration_log_path = state_dir.join("migration.log");
        let triage_path = state_dir.join("triage.json");

        Self {
            repo_root,
            state_dir,
            patches_dir,
            sandbox_dir,
            config_path,
            patch_md_path,
            migration_log_path,
            triage_path,
        }
    }
}

pub fn resolve(options: &ResolveOptions) -> Result<ResolvedPaths, RedtapeError> {
    let cwd = match &options.cwd {
        Some(cwd) => absolute_from(&env::current_dir()?, cwd),
        None => env::current_dir()?,
    };

    let repo_root = match &options.repo_root_override {
        Some(repo_root) => absolute_from(&cwd, repo_root),
        None => discover_git_repo_root(&cwd).unwrap_or_else(|| cwd.clone()),
    };

    let state_dir = match &options.state_dir_override {
        Some(state_dir) => absolute_from(&repo_root, state_dir),
        None => repo_root.join(".t3"),
    };

    Ok(ResolvedPaths::new(repo_root, state_dir))
}

fn discover_git_repo_root(start_dir: &Path) -> Option<PathBuf> {
    let output = Command::new("git")
        .arg("rev-parse")
        .arg("--show-toplevel")
        .current_dir(start_dir)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if path.is_empty() {
        None
    } else {
        Some(PathBuf::from(path))
    }
}

fn absolute_from(base: &Path, candidate: &Path) -> PathBuf {
    if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        base.join(candidate)
    }
}
