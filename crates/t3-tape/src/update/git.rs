use std::path::Path;
use std::process::{Command, Output};

use crate::exit::RedtapeError;

pub fn head(repo_root: &Path) -> Result<String, RedtapeError> {
    capture_stdout(repo_root, &["rev-parse", "HEAD"])
}

pub fn fetch_ref(
    repo_root: &Path,
    upstream: &str,
    reference: &str,
) -> Result<String, RedtapeError> {
    run(repo_root, &["fetch", upstream, reference])?;
    capture_stdout(repo_root, &["rev-parse", "FETCH_HEAD"])
}

pub fn create_worktree(
    repo_root: &Path,
    worktree_path: &Path,
    branch: &str,
    target_ref: &str,
) -> Result<(), RedtapeError> {
    if worktree_path.exists() {
        return Err(RedtapeError::Git(format!(
            "sandbox worktree path already exists: {}",
            worktree_path.display()
        )));
    }

    let worktree = worktree_path.to_string_lossy().to_string();
    let branch_name = branch.to_string();
    let target = target_ref.to_string();
    run_owned(
        repo_root,
        &[
            "worktree".to_string(),
            "add".to_string(),
            "-b".to_string(),
            branch_name,
            worktree,
            target,
        ],
    )
}

pub fn remove_worktree(repo_root: &Path, worktree_path: &Path) -> Result<(), RedtapeError> {
    let worktree = worktree_path.to_string_lossy().to_string();
    run_owned(
        repo_root,
        &["worktree".to_string(), "remove".to_string(), worktree],
    )
}

pub fn delete_branch(repo_root: &Path, branch: &str) -> Result<(), RedtapeError> {
    run(repo_root, &["branch", "-D", branch])
}

pub fn apply_check(worktree_path: &Path, diff_path: &Path, reverse: bool) -> Result<(), String> {
    let mut args = vec!["apply", "--check"];
    if reverse {
        args.push("--reverse");
    }
    let diff = diff_path.to_string_lossy().to_string();
    let output = Command::new("git")
        .args(args)
        .arg(diff)
        .current_dir(worktree_path)
        .output()
        .map_err(|err| err.to_string())?;

    if output.status.success() {
        Ok(())
    } else {
        Err(stderr_or_fallback(&output, "git apply --check failed"))
    }
}

pub fn apply_patch(worktree_path: &Path, diff_path: &Path) -> Result<(), RedtapeError> {
    let diff = diff_path.to_string_lossy().to_string();
    let output = Command::new("git")
        .args(["apply", &diff])
        .current_dir(worktree_path)
        .output()?;

    if output.status.success() {
        Ok(())
    } else {
        Err(RedtapeError::Git(stderr_or_fallback(
            &output,
            "git apply failed",
        )))
    }
}

pub fn stage_all(worktree_path: &Path) -> Result<(), RedtapeError> {
    run(worktree_path, &["add", "."])
}

pub fn commit_all(worktree_path: &Path, message: &str) -> Result<String, RedtapeError> {
    run(worktree_path, &["commit", "-m", message, "--quiet"])?;
    capture_stdout(worktree_path, &["rev-parse", "HEAD"])
}

pub fn show_commit_patch(worktree_path: &Path, commit: &str) -> Result<String, RedtapeError> {
    capture_stdout(worktree_path, &["show", "--format=", commit])
}

pub fn read_file_at_ref(
    repo_root: &Path,
    target_ref: &str,
    relative_path: &str,
) -> Result<String, RedtapeError> {
    capture_stdout(
        repo_root,
        &["show", &format!("{target_ref}:{relative_path}")],
    )
}

pub fn current_head_matches(repo_root: &Path, expected: &str) -> Result<bool, RedtapeError> {
    Ok(head(repo_root)? == expected)
}

pub fn run(repo_root: &Path, args: &[&str]) -> Result<(), RedtapeError> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo_root)
        .output()?;
    if output.status.success() {
        Ok(())
    } else {
        Err(RedtapeError::Git(stderr_or_fallback(
            &output,
            &format!("git {:?} failed", args),
        )))
    }
}

pub fn run_owned(repo_root: &Path, args: &[String]) -> Result<(), RedtapeError> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo_root)
        .output()?;
    if output.status.success() {
        Ok(())
    } else {
        Err(RedtapeError::Git(stderr_or_fallback(
            &output,
            "git command failed",
        )))
    }
}

pub fn capture_stdout(repo_root: &Path, args: &[&str]) -> Result<String, RedtapeError> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo_root)
        .output()?;
    if !output.status.success() {
        return Err(RedtapeError::Git(stderr_or_fallback(
            &output,
            &format!("git {:?} failed", args),
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn stderr_or_fallback(output: &Output, fallback: &str) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if stderr.is_empty() {
        fallback.to_string()
    } else {
        stderr
    }
}
