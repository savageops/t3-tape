pub mod atomic;
pub mod lock;
pub mod paths;
pub mod schema;
pub mod time;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::exit::RedtapeError;
use paths::{ResolveOptions, ResolvedPaths};

#[derive(Debug, Clone)]
pub struct InitRequest {
    pub repo_root: Option<PathBuf>,
    pub state_dir: Option<PathBuf>,
    pub upstream: String,
    pub base_ref: String,
    pub cwd: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct InitReport {
    pub paths: ResolvedPaths,
    pub created_directories: Vec<PathBuf>,
    pub created_files: Vec<PathBuf>,
}

pub fn initialize(request: InitRequest) -> Result<InitReport, RedtapeError> {
    let InitRequest {
        repo_root,
        state_dir,
        upstream,
        base_ref,
        cwd,
    } = request;

    let paths = paths::resolve(&ResolveOptions {
        repo_root_override: repo_root,
        state_dir_override: state_dir,
        cwd,
    })?;

    let _lock = lock::StateLock::acquire(&paths.lock_path)?;

    ensure_existing_directory(&paths.repo_root, "repo root")?;

    let mut report = InitReport {
        paths: paths.clone(),
        created_directories: Vec::new(),
        created_files: Vec::new(),
    };

    ensure_directory(&report.paths.state_dir, &mut report.created_directories)?;
    ensure_directory(&report.paths.plugin_root, &mut report.created_directories)?;
    ensure_directory(&report.paths.patches_dir, &mut report.created_directories)?;
    ensure_directory(&report.paths.sandbox_dir, &mut report.created_directories)?;

    ensure_file(
        &report.paths.config_path,
        schema::render_config(&upstream)?.as_bytes(),
        &mut report.created_files,
    )?;
    if report.paths.patch_md_path.exists() {
        let existing = fs::read_to_string(&report.paths.patch_md_path)?;
        if existing.trim().is_empty() {
            let resolved_base_ref = resolve_base_ref(&report.paths.repo_root, &base_ref)?;
            let header = schema::build_patch_header(&upstream, &resolved_base_ref);
            atomic::write_file_atomic(&report.paths.patch_md_path, header.as_bytes())?;
        }
    } else {
        let resolved_base_ref = resolve_base_ref(&report.paths.repo_root, &base_ref)?;
        ensure_file(
            &report.paths.patch_md_path,
            schema::build_patch_header(&upstream, &resolved_base_ref).as_bytes(),
            &mut report.created_files,
        )?;
    }
    ensure_file(
        &report.paths.migration_log_path,
        schema::empty_migration_log().as_bytes(),
        &mut report.created_files,
    )?;
    ensure_file(
        &report.paths.triage_path,
        schema::empty_triage_summary().as_bytes(),
        &mut report.created_files,
    )?;

    Ok(report)
}

fn resolve_base_ref(repo_root: &Path, base_ref: &str) -> Result<String, RedtapeError> {
    let output = Command::new("git")
        .args(["rev-parse", "--verify", &format!("{base_ref}^{{commit}}")])
        .current_dir(repo_root)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let detail = if stderr.is_empty() {
            format!("base ref `{base_ref}` did not resolve to a commit")
        } else {
            stderr
        };
        return Err(RedtapeError::Usage(detail));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn ensure_existing_directory(path: &Path, label: &str) -> Result<(), RedtapeError> {
    if !path.exists() {
        return Err(RedtapeError::Usage(format!(
            "{label} does not exist: {}",
            path.display()
        )));
    }

    if !path.is_dir() {
        return Err(RedtapeError::Usage(format!(
            "{label} is not a directory: {}",
            path.display()
        )));
    }

    Ok(())
}

fn ensure_directory(path: &Path, created: &mut Vec<PathBuf>) -> Result<(), RedtapeError> {
    if path.exists() {
        if path.is_dir() {
            return Ok(());
        }

        return Err(RedtapeError::Usage(format!(
            "expected directory at {}",
            path.display()
        )));
    }

    fs::create_dir_all(path)?;
    created.push(path.to_path_buf());
    Ok(())
}

fn ensure_file(
    path: &Path,
    contents: &[u8],
    created: &mut Vec<PathBuf>,
) -> Result<(), RedtapeError> {
    if path.exists() {
        if path.is_file() {
            return Ok(());
        }

        return Err(RedtapeError::Usage(format!(
            "expected file at {}",
            path.display()
        )));
    }

    atomic::write_new_file_atomic(path, contents)?;
    created.push(path.to_path_buf());
    Ok(())
}
