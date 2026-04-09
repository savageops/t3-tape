pub mod atomic;
pub mod paths;
pub mod schema;
pub mod time;

use std::fs;
use std::path::{Path, PathBuf};

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

    ensure_existing_directory(&paths.repo_root, "repo root")?;

    let mut report = InitReport {
        paths: paths.clone(),
        created_directories: Vec::new(),
        created_files: Vec::new(),
    };

    ensure_directory(&report.paths.state_dir, &mut report.created_directories)?;
    ensure_directory(&report.paths.patches_dir, &mut report.created_directories)?;
    ensure_directory(&report.paths.sandbox_dir, &mut report.created_directories)?;

    ensure_file(
        &report.paths.config_path,
        schema::render_config(&upstream)?.as_bytes(),
        &mut report.created_files,
    )?;
    ensure_file(
        &report.paths.patch_md_path,
        schema::build_patch_header(&upstream, &base_ref).as_bytes(),
        &mut report.created_files,
    )?;
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
