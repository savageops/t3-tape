pub mod export;
pub mod hooks;
pub mod patch_add;
pub mod patch_import;
pub mod patch_list;
pub mod patch_show;
pub mod rederive;
pub mod triage;
pub mod triage_approve;
pub mod update;
pub mod validate;

use std::path::PathBuf;

#[derive(Debug, Clone, Default)]
pub struct GlobalOptions {
    pub repo_root: Option<PathBuf>,
    pub state_dir: Option<PathBuf>,
    pub json: bool,
    pub cwd: Option<PathBuf>,
}
