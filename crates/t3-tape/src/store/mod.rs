pub mod atomic;
pub mod lock;
pub mod paths;
pub mod schema;
pub mod time;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::exit::RedtapeError;
use crate::patch::patch_md;
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

    ensure_config_file(
        &report.paths.config_path,
        &upstream,
        &mut report.created_files,
    )?;
    ensure_patch_registry(
        &report.paths.patch_md_path,
        &report.paths.repo_root,
        &upstream,
        &base_ref,
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

fn ensure_config_file(
    path: &Path,
    upstream: &str,
    created: &mut Vec<PathBuf>,
) -> Result<(), RedtapeError> {
    if path.exists() {
        if !path.is_file() {
            return Err(RedtapeError::Usage(format!(
                "expected file at {}",
                path.display()
            )));
        }

        let config = schema::read_config(path)?;
        if config.protocol != schema::PROTOCOL_VERSION {
            return Err(RedtapeError::Validation(format!(
                "existing config.json protocol mismatch at {}: expected {} but found {}",
                path.display(),
                schema::PROTOCOL_VERSION,
                config.protocol
            )));
        }
        return Ok(());
    }

    atomic::write_new_file_atomic(path, schema::render_config(upstream)?.as_bytes())?;
    created.push(path.to_path_buf());
    Ok(())
}

fn ensure_patch_registry(
    path: &Path,
    repo_root: &Path,
    upstream: &str,
    base_ref: &str,
    created: &mut Vec<PathBuf>,
) -> Result<(), RedtapeError> {
    if path.exists() {
        if !path.is_file() {
            return Err(RedtapeError::Usage(format!(
                "expected file at {}",
                path.display()
            )));
        }

        let existing = fs::read_to_string(path)?;
        if existing.trim().is_empty() {
            let resolved_base_ref = resolve_base_ref(repo_root, base_ref)?;
            let header = schema::build_patch_header(upstream, &resolved_base_ref);
            atomic::write_file_atomic(path, header.as_bytes())?;
            return Ok(());
        }

        let document = patch_md::parse(&existing)?;
        let parsed_header = patch_md::parse_header(&document.header)?;
        if parsed_header.protocol != schema::PROTOCOL_VERSION {
            return Err(RedtapeError::Validation(format!(
                "existing patch.md protocol mismatch at {}: expected {} but found {}",
                path.display(),
                schema::PROTOCOL_VERSION,
                parsed_header.protocol
            )));
        }
        if parsed_header.state_root.as_deref() != Some("patch") {
            return Err(RedtapeError::Validation(format!(
                "existing patch.md state-root mismatch at {}: expected `patch`",
                path.display()
            )));
        }
        return Ok(());
    }

    let resolved_base_ref = resolve_base_ref(repo_root, base_ref)?;
    let header = schema::build_patch_header(upstream, &resolved_base_ref);
    atomic::write_new_file_atomic(path, header.as_bytes())?;
    created.push(path.to_path_buf());
    Ok(())
}
