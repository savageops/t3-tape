use std::process::Command;

use crate::exit::RedtapeError;
use crate::store::paths::ResolvedPaths;

use super::{child_relative_path, join_relative, repo_relative, ValidationReport};

pub fn validate(paths: &ResolvedPaths, report: &mut ValidationReport) -> Result<(), RedtapeError> {
    if !is_git_repo(paths)? {
        report.push_error("validate --staged requires a git repository");
        return Ok(());
    }

    let staged_paths = staged_paths(paths)?;
    if staged_paths.is_empty() {
        return Ok(());
    }

    let state_prefix = repo_relative(&paths.repo_root, &paths.state_dir);
    let patch_md_path = repo_relative(&paths.repo_root, &paths.patch_md_path)
        .unwrap_or_else(|| join_relative(state_prefix.as_deref(), "patch.md"));
    let patches_dir = repo_relative(&paths.repo_root, &paths.patches_dir)
        .unwrap_or_else(|| join_relative(state_prefix.as_deref(), "patches"));

    let mut has_project_code = false;
    let mut has_patch_md = false;
    let mut has_diff = false;
    let mut has_meta = false;

    for path in &staged_paths {
        if !child_relative_path(state_prefix.as_deref(), path) {
            has_project_code = true;
            continue;
        }

        if path == &patch_md_path {
            has_patch_md = true;
        }
        if child_relative_path(Some(&patches_dir), path) && path.ends_with(".diff") {
            has_diff = true;
        }
        if child_relative_path(Some(&patches_dir), path) && path.ends_with(".meta.json") {
            has_meta = true;
        }
    }

    if has_project_code && !(has_patch_md && has_diff && has_meta) {
        report.push_error(format!(
            "staged project code changes require PatchMD updates: stage `{patch_md_path}` plus at least one `.diff` and one `.meta.json` under `{patches_dir}`"
        ));
    }

    Ok(())
}

fn is_git_repo(paths: &ResolvedPaths) -> Result<bool, RedtapeError> {
    let output = Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .current_dir(&paths.repo_root)
        .output()?;

    Ok(output.status.success()
        && String::from_utf8_lossy(&output.stdout)
            .trim()
            .eq_ignore_ascii_case("true"))
}

fn staged_paths(paths: &ResolvedPaths) -> Result<Vec<String>, RedtapeError> {
    let output = Command::new("git")
        .args(["diff", "--name-only", "--cached"])
        .current_dir(&paths.repo_root)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(RedtapeError::Git(if stderr.is_empty() {
            "git diff --name-only --cached failed".to_string()
        } else {
            stderr
        }));
    }

    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| line.replace('\\', "/"))
        .collect())
}
