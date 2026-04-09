use std::fs;
use std::process::Command;

use crate::cli::{HooksArgs, HooksCommand, HooksInstallKind, HooksPrintKind};
use crate::exit::RedtapeError;
use crate::patch;
use crate::store::atomic;

use super::GlobalOptions;

pub fn run(global: &GlobalOptions, args: &HooksArgs) -> Result<(), RedtapeError> {
    match &args.command {
        HooksCommand::Print(args) => {
            print!("{}", render_print(&args.kind));
            Ok(())
        }
        HooksCommand::Install(args) => install(global, args.kind.clone(), args.force),
    }
}

fn render_print(kind: &HooksPrintKind) -> String {
    match kind {
        HooksPrintKind::PreCommit => pre_commit_hook(),
        HooksPrintKind::Gitignore => {
            ".t3/patch/sandbox/\n.t3/patch/config.json.local\n.t3/patch/state.lock\n"
                .to_string()
        }
        HooksPrintKind::Gitattributes => {
            ".t3/patch.md merge=union\n.t3/patch/migration.log merge=union\n".to_string()
        }
    }
}

fn install(
    global: &GlobalOptions,
    kind: HooksInstallKind,
    force: bool,
) -> Result<(), RedtapeError> {
    match kind {
        HooksInstallKind::PreCommit => install_pre_commit(global, force),
    }
}

fn install_pre_commit(global: &GlobalOptions, force: bool) -> Result<(), RedtapeError> {
    let paths = patch::resolve_paths(global)?;
    let hooks_dir = git_hooks_dir(&paths.repo_root)?;
    fs::create_dir_all(&hooks_dir)?;

    let hook_path = hooks_dir.join("pre-commit");
    if hook_path.exists() && !force {
        return Err(RedtapeError::Usage(format!(
            "refusing to overwrite existing pre-commit hook at {} (use --force)",
            hook_path.display()
        )));
    }

    let contents = pre_commit_hook();
    if hook_path.exists() {
        atomic::write_file_atomic(&hook_path, contents.as_bytes())?;
    } else {
        atomic::write_new_file_atomic(&hook_path, contents.as_bytes())?;
    }

    set_hook_permissions(&hook_path)?;
    println!("installed pre-commit hook at {}", hook_path.display());
    Ok(())
}

fn git_hooks_dir(repo_root: &std::path::Path) -> Result<std::path::PathBuf, RedtapeError> {
    let output = Command::new("git")
        .args(["rev-parse", "--git-path", "hooks"])
        .current_dir(repo_root)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(RedtapeError::Git(if stderr.is_empty() {
            "unable to resolve git hooks directory".to_string()
        } else {
            stderr
        }));
    }

    let resolved = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if resolved.is_empty() {
        return Err(RedtapeError::Git(
            "git did not return a hooks directory".to_string(),
        ));
    }

    Ok(repo_root.join(resolved))
}

fn pre_commit_hook() -> String {
    "#!/bin/sh\nt3-tape validate --staged\nif [ $? -ne 0 ]; then\n  echo \"PatchMD: staged changes missing intent entry. Run: t3-tape patch add\"\n  exit 1\nfi\n".to_string()
}

#[cfg(unix)]
fn set_hook_permissions(path: &std::path::Path) -> Result<(), RedtapeError> {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = fs::metadata(path)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions)?;
    Ok(())
}

#[cfg(not(unix))]
fn set_hook_permissions(_path: &std::path::Path) -> Result<(), RedtapeError> {
    Ok(())
}
