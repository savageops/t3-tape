use std::path::Path;

use crate::exit::RedtapeError;

use super::git;

pub fn apply_and_commit(
    worktree_path: &Path,
    diff_path: &Path,
    patch_id: &str,
    title: &str,
) -> Result<String, RedtapeError> {
    git::apply_patch(worktree_path, diff_path)?;
    git::stage_all(worktree_path)?;
    git::commit_all(worktree_path, &format!("{patch_id}: {title}"))
}
