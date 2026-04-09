pub mod diff;
pub mod meta;
pub mod patch_id;
pub mod patch_md;
pub mod surface_hash;

use std::collections::BTreeSet;
use std::fs;
use std::io::{self, BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::commands::GlobalOptions;
use crate::exit::RedtapeError;
use crate::store::atomic;
use crate::store::paths::{self, ResolveOptions, ResolvedPaths};
use crate::store::schema::{self, Config};
use crate::store::time;

pub use diff::UnifiedDiff;
pub use meta::PatchMeta;
pub use patch_id::PatchId;
pub use patch_md::{PatchDocument, PatchEntry};

#[derive(Debug, Clone)]
pub struct NewPatchSpec {
    pub title: String,
    pub intent: String,
    pub assertions: Vec<String>,
    pub surface: Option<String>,
    pub raw_diff: String,
}

#[derive(Debug, Clone)]
pub struct PatchWriteContext {
    pub base_ref: String,
    pub current_ref: String,
    pub author: String,
    pub added_date: String,
}

#[derive(Debug, Clone)]
pub struct CreatedPatch {
    pub id: PatchId,
    pub title: String,
    pub diff_path: PathBuf,
    pub meta_path: PathBuf,
}

pub fn resolve_paths(global: &GlobalOptions) -> Result<ResolvedPaths, RedtapeError> {
    paths::resolve(&ResolveOptions {
        repo_root_override: global.repo_root.clone(),
        state_dir_override: global.state_dir.clone(),
        cwd: global.cwd.clone(),
    })
}

pub fn read_document(paths: &ResolvedPaths) -> Result<(String, PatchDocument), RedtapeError> {
    let content = fs::read_to_string(&paths.patch_md_path)?;
    let document = patch_md::parse(&content)?;
    Ok((content, document))
}

pub fn read_meta_for_id(
    paths: &ResolvedPaths,
    id: PatchId,
) -> Result<Option<meta::PatchMeta>, RedtapeError> {
    meta::read(&meta_path(paths, id))
}

pub fn diff_path(paths: &ResolvedPaths, id: PatchId) -> PathBuf {
    paths.patches_dir.join(format!("{id}.diff"))
}

pub fn meta_path(paths: &ResolvedPaths, id: PatchId) -> PathBuf {
    paths.patches_dir.join(format!("{id}.meta.json"))
}

pub fn load_config(paths: &ResolvedPaths) -> Result<Config, RedtapeError> {
    schema::read_config(&paths.config_path)
}

pub fn head_ref(repo_root: &Path) -> Result<String, RedtapeError> {
    git_output(repo_root, &["rev-parse", "HEAD"])
}

pub fn capture_git_diff(paths: &ResolvedPaths, staged: bool) -> Result<String, RedtapeError> {
    let args = if staged {
        vec!["diff", "--cached", "--no-ext-diff"]
    } else {
        vec!["diff", "--no-ext-diff", "HEAD"]
    };

    let diff = git_output(&paths.repo_root, &args)?;
    if diff.trim().is_empty() {
        return Err(RedtapeError::Usage(
            "no diff to record; make a change or use --staged".to_string(),
        ));
    }

    let parsed = UnifiedDiff::parse(&diff)?;
    let patch_registry_path = repo_relative(&paths.repo_root, &paths.patch_md_path);
    let plugin_root = repo_relative(&paths.repo_root, &paths.plugin_root);
    let filtered = parsed
        .files
        .into_iter()
        .filter(|file| !is_patchmd_owned_path(&file.path, patch_registry_path.as_deref(), plugin_root.as_deref()))
        .collect::<Vec<_>>();

    if filtered.is_empty() {
        let ownership = match (patch_registry_path.as_deref(), plugin_root.as_deref()) {
            (Some(registry), Some(plugin_root)) => format!("`{registry}` and `{plugin_root}/**`"),
            (Some(registry), None) => format!("`{registry}`"),
            (None, Some(plugin_root)) => format!("`{plugin_root}/**`"),
            (None, None) => "PatchMD-owned state".to_string(),
        };
        return Err(RedtapeError::Usage(format!(
            "no diff to record; remaining changes only touch PatchMD-owned state under {ownership}"
        )));
    }

    Ok(UnifiedDiff::render_files(&filtered))
}

pub fn default_author(repo_root: &Path) -> String {
    git_output(repo_root, &["config", "user.name"])
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| std::env::var("T3_TAPE_AUTHOR").ok())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

pub fn build_write_context(repo_root: &Path) -> Result<PatchWriteContext, RedtapeError> {
    let base_ref = head_ref(repo_root)?;
    Ok(PatchWriteContext {
        base_ref: base_ref.clone(),
        current_ref: base_ref,
        author: default_author(repo_root),
        added_date: time::current_utc_date(),
    })
}

pub fn read_intent_from_file(path: &Path) -> Result<String, RedtapeError> {
    let content = fs::read_to_string(path)?;
    let trimmed = content.trim().to_string();
    if trimmed.is_empty() {
        return Err(RedtapeError::Usage(format!(
            "intent file was empty: {}",
            path.display()
        )));
    }
    Ok(trimmed)
}

pub fn stdin_is_terminal() -> bool {
    io::stdin().is_terminal()
}

pub fn prompt_line(prompt: &str) -> Result<String, RedtapeError> {
    print!("{prompt}");
    io::stdout().flush()?;
    let mut line = String::new();
    io::stdin().lock().read_line(&mut line)?;
    let trimmed = line.trim().to_string();
    if trimmed.is_empty() {
        return Err(RedtapeError::Usage(
            "input was required but not provided".to_string(),
        ));
    }
    Ok(trimmed)
}

pub fn confirm(prompt: &str) -> Result<bool, RedtapeError> {
    print!("{prompt}");
    io::stdout().flush()?;
    let mut line = String::new();
    io::stdin().lock().read_line(&mut line)?;
    let normalized = line.trim().to_ascii_lowercase();
    Ok(matches!(normalized.as_str(), "y" | "yes"))
}

pub fn create_patch_records(
    paths: &ResolvedPaths,
    context: &PatchWriteContext,
    specs: &[NewPatchSpec],
) -> Result<Vec<CreatedPatch>, RedtapeError> {
    if specs.is_empty() {
        return Err(RedtapeError::Usage(
            "no patch records were provided".to_string(),
        ));
    }

    let config = load_config(paths)?;
    let (patch_md_before, document) = read_document(paths)?;
    let mut next_id = next_patch_id(paths, &document)?;

    run_hook(
        &config.hooks.pre_patch,
        &paths.repo_root,
        &[
            ("T3_TAPE_REPO_ROOT", paths.repo_root.display().to_string()),
            ("T3_TAPE_STATE_DIR", paths.state_dir.display().to_string()),
        ],
    )?;

    let timestamp = time::current_utc_rfc3339();
    let mut entries = Vec::new();
    let mut metas = Vec::new();
    let mut diff_targets = Vec::new();
    let mut meta_targets = Vec::new();
    let mut created = Vec::new();

    for spec in specs {
        let parsed_diff = diff::UnifiedDiff::parse(&spec.raw_diff)?;
        let changed_files = parsed_diff.changed_paths();
        let surface = spec
            .surface
            .clone()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| changed_files.join(", "));

        if surface.trim().is_empty() {
            return Err(RedtapeError::Usage(
                "unable to derive patch surface from diff".to_string(),
            ));
        }

        let id = next_id;
        next_id = next_id.next_after();
        let diff_path = diff_path(paths, id);
        let meta_path = meta_path(paths, id);
        let diff_file = format!("patches/{id}.diff");

        entries.push(patch_md::PatchEntry {
            id,
            title: spec.title.clone(),
            status: "active".to_string(),
            surface,
            added: context.added_date.clone(),
            author: context.author.clone(),
            intent: spec.intent.clone(),
            behavior_assertions: spec.assertions.clone(),
            scope_files: changed_files,
            scope_components: Vec::new(),
            scope_entry_points: Vec::new(),
            requires: Vec::new(),
            conflicts_with: Vec::new(),
            notes: None,
            extra_sections: Vec::new(),
            raw_block: String::new(),
        });

        metas.push(meta::PatchMeta {
            id: id.to_string(),
            title: spec.title.clone(),
            status: "active".to_string(),
            base_ref: context.base_ref.clone(),
            current_ref: context.current_ref.clone(),
            diff_file,
            apply_confidence: 1.0,
            last_applied: timestamp.clone(),
            last_checked: timestamp.clone(),
            agent_attempts: 0,
            surface_hash: surface_hash::compute(&parsed_diff),
            behavior_assertions: spec.assertions.clone(),
        });

        diff_targets.push((diff_path.clone(), spec.raw_diff.clone()));
        meta_targets.push(meta_path.clone());
        created.push(CreatedPatch {
            id,
            title: spec.title.clone(),
            diff_path,
            meta_path,
        });
    }

    let mut created_files = Vec::new();
    let result = (|| {
        for (path, raw_diff) in &diff_targets {
            atomic::write_new_file_atomic(path, ensure_trailing_newline(raw_diff).as_bytes())?;
            created_files.push(path.clone());
        }

        for (path, meta) in meta_targets.iter().zip(metas.iter()) {
            meta::write_new(path, meta)?;
            created_files.push(path.clone());
        }

        if std::env::var_os("T3_TAPE_INTERNAL_TEST_FAIL_AFTER_PATCH_FILES").is_some() {
            return Err(RedtapeError::Blocked(
                "injected failure after patch files were created".to_string(),
            ));
        }

        let patch_md_after = patch_md::append_entries(&patch_md_before, &entries);
        atomic::write_file_atomic(&paths.patch_md_path, patch_md_after.as_bytes())?;
        Ok(())
    })();

    if let Err(err) = result {
        cleanup_created_files(&created_files);
        return Err(err);
    }

    let patch_ids = created
        .iter()
        .map(|patch| patch.id.to_string())
        .collect::<Vec<_>>()
        .join(",");

    run_hook(
        &config.hooks.post_patch,
        &paths.repo_root,
        &[
            ("T3_TAPE_REPO_ROOT", paths.repo_root.display().to_string()),
            ("T3_TAPE_STATE_DIR", paths.state_dir.display().to_string()),
            ("T3_TAPE_PATCH_IDS", patch_ids),
        ],
    )?;

    Ok(created)
}

fn next_patch_id(paths: &ResolvedPaths, document: &PatchDocument) -> Result<PatchId, RedtapeError> {
    let mut ids = BTreeSet::new();
    for entry in &document.entries {
        ids.insert(entry.id);
    }

    if paths.patches_dir.is_dir() {
        for entry in fs::read_dir(&paths.patches_dir)? {
            let entry = entry?;
            if entry.path().extension().and_then(|ext| ext.to_str()) == Some("diff") {
                if let Some(id) = PatchId::from_diff_path(&entry.path()) {
                    ids.insert(id);
                }
            }
        }
    }

    let next_value = ids.iter().next_back().map(|id| id.value() + 1).unwrap_or(1);
    PatchId::new(next_value)
}

fn cleanup_created_files(paths: &[PathBuf]) {
    for path in paths.iter().rev() {
        let _ = fs::remove_file(path);
    }
}

fn run_hook(command: &str, cwd: &Path, envs: &[(&str, String)]) -> Result<(), RedtapeError> {
    if command.trim().is_empty() {
        return Ok(());
    }

    let mut process = if cfg!(windows) {
        let mut command_process = Command::new("cmd");
        command_process.arg("/C").arg(command);
        command_process
    } else {
        let mut command_process = Command::new("sh");
        command_process.arg("-lc").arg(command);
        command_process
    };

    process.current_dir(cwd);
    for (key, value) in envs {
        process.env(key, value);
    }

    let output = process.output()?;
    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let detail = if !stderr.is_empty() {
        stderr
    } else if !stdout.is_empty() {
        stdout
    } else {
        format!("exit status {}", output.status)
    };

    Err(RedtapeError::Blocked(format!(
        "hook failed: `{command}` ({detail})"
    )))
}

fn git_output(repo_root: &Path, args: &[&str]) -> Result<String, RedtapeError> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo_root)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let detail = if stderr.is_empty() {
            format!("git {:?} failed with {}", args, output.status)
        } else {
            stderr
        };
        return Err(RedtapeError::Git(detail));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn ensure_trailing_newline(value: &str) -> String {
    if value.ends_with('\n') {
        value.to_string()
    } else {
        format!("{value}\n")
    }
}

fn repo_relative(repo_root: &Path, target: &Path) -> Option<String> {
    let relative = target.strip_prefix(repo_root).ok()?;
    let rendered = relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy().replace('\\', "/"))
        .collect::<Vec<_>>()
        .join("/");
    if rendered.is_empty() {
        None
    } else {
        Some(rendered)
    }
}

fn is_patchmd_owned_path(path: &str, patch_registry_path: Option<&str>, plugin_root: Option<&str>) -> bool {
    if patch_registry_path.is_some_and(|registry| path == registry) {
        return true;
    }

    plugin_root.is_some_and(|root| path == root || path.starts_with(&format!("{root}/")))
}
