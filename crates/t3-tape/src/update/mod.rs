pub mod apply;
pub mod git;
pub mod resolve;
pub mod sandbox;
pub mod triage;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::cli::{RederiveArgs, TriageApproveArgs, UpdateArgs};
use crate::commands::GlobalOptions;
use crate::exit::RedtapeError;
use crate::patch::{self, PatchEntry, PatchId, UnifiedDiff};
use crate::store::atomic;
use crate::store::schema::{self, Config};
use crate::store::time;
use crate::validate::full;

use self::sandbox::SandboxContext;
use self::triage::{PreviewSummary, TriagePatch, TriageSummary};

#[derive(Debug, Clone)]
pub struct UpdateOutcome {
    pub summary: TriageSummary,
    pub exit_code: u8,
}

#[derive(Debug, Clone)]
pub struct ApprovalOutcome {
    pub patch_id: String,
    pub status: String,
    pub cycle_complete: bool,
}

#[derive(Debug, Clone)]
struct PatchMaterial {
    entry: PatchEntry,
    diff_path: PathBuf,
    raw_diff: String,
    changed_paths: Vec<String>,
}

struct ResolutionContext<'a> {
    config: &'a Config,
    paths: &'a crate::store::paths::ResolvedPaths,
    sandbox: &'a SandboxContext,
    from_ref: &'a str,
    to_ref_resolved: &'a str,
    threshold: f64,
    materials: &'a BTreeMap<String, PatchMaterial>,
}

pub fn run_update(
    global: &GlobalOptions,
    args: &UpdateArgs,
) -> Result<UpdateOutcome, RedtapeError> {
    let paths = patch::resolve_paths(global)?;
    ensure_repo_valid(&paths)?;
    let config = schema::read_config(&paths.config_path)?;
    let from_ref = git::head(&paths.repo_root)?;
    let sandbox = SandboxContext::new(&paths);

    append_started_log(&paths, &sandbox, &from_ref, &args.r#ref)?;
    run_hook(
        &config.hooks.pre_update,
        &paths.repo_root,
        &[
            ("T3_TAPE_REPO_ROOT", paths.repo_root.display().to_string()),
            ("T3_TAPE_STATE_DIR", paths.state_dir.display().to_string()),
            ("T3_TAPE_SANDBOX_PATH", sandbox.root.display().to_string()),
        ],
    )?;

    fs::create_dir_all(&sandbox.root)?;
    let to_ref_resolved = git::fetch_ref(&paths.repo_root, &config.upstream, &args.r#ref)?;
    git::create_worktree(
        &paths.repo_root,
        &sandbox.worktree_path,
        &sandbox.branch,
        &to_ref_resolved,
    )?;

    let (_, document) = patch::read_document(&paths)?;
    let active_entries = document
        .entries
        .into_iter()
        .filter(|entry| entry.status == "active")
        .collect::<Vec<_>>();

    let mut materials = BTreeMap::<String, PatchMaterial>::new();
    let mut patches = Vec::new();
    for entry in active_entries {
        let material = load_patch_material(&paths, &entry)?;
        let triage_patch = classify_patch(&sandbox, &material)?;
        materials.insert(material.entry.id.to_string(), material);
        patches.push(triage_patch);
    }

    let mut summary = TriageSummary::new(
        from_ref.clone(),
        args.r#ref.clone(),
        to_ref_resolved.clone(),
        config.upstream.clone(),
        time::current_utc_rfc3339(),
        sandbox.summary(),
        patches,
    );

    let resolution_context = ResolutionContext {
        config: &config,
        paths: &paths,
        sandbox: &sandbox,
        from_ref: &from_ref,
        to_ref_resolved: &to_ref_resolved,
        threshold: args
            .confidence_threshold
            .unwrap_or(config.agent.confidence_threshold),
        materials: &materials,
    };

    resolve_non_clean_patches(&resolution_context, &mut summary)?;

    apply_resolved_patches(&sandbox, &materials, &mut summary)?;
    maybe_run_preview(&config, &sandbox, &mut summary)?;

    triage::write(&sandbox.triage_path, &summary)?;
    triage::write(&paths.triage_path, &summary)?;
    append_triaged_log(&paths, &summary)?;

    if summary
        .patches
        .iter()
        .any(|patch| patch.triage_status == "NEEDS-YOU")
    {
        run_hook(
            &config.hooks.on_conflict,
            &paths.repo_root,
            &[
                ("T3_TAPE_REPO_ROOT", paths.repo_root.display().to_string()),
                ("T3_TAPE_STATE_DIR", paths.state_dir.display().to_string()),
                ("T3_TAPE_SANDBOX_PATH", sandbox.root.display().to_string()),
                (
                    "T3_TAPE_TRIAGE_PATH",
                    paths.triage_path.display().to_string(),
                ),
            ],
        )?;
    }

    let blocked = summary
        .patches
        .iter()
        .any(|patch| patch.triage_status == "NEEDS-YOU");
    let ci_non_clean = args.ci
        && summary
            .patches
            .iter()
            .any(|patch| patch.detected_status != "CLEAN");
    let exit_code = if blocked || ci_non_clean { 3 } else { 0 };

    Ok(UpdateOutcome { summary, exit_code })
}

pub fn read_latest_triage(global: &GlobalOptions) -> Result<TriageSummary, RedtapeError> {
    let paths = patch::resolve_paths(global)?;
    if !paths.triage_path.exists() {
        return Err(RedtapeError::Validation(format!(
            "triage summary does not exist: {}",
            paths.triage_path.display()
        )));
    }
    triage::read(&paths.triage_path)
}

pub fn approve_patch(
    global: &GlobalOptions,
    args: &TriageApproveArgs,
) -> Result<ApprovalOutcome, RedtapeError> {
    let paths = patch::resolve_paths(global)?;
    let config = schema::read_config(&paths.config_path)?;
    let mut summary = triage::read(&paths.triage_path)?;
    let to_ref_resolved = summary.to_ref_resolved.clone();
    let sandbox_triage_path = PathBuf::from(&summary.sandbox.path).join("triage.json");

    if let Some(preview) = &summary.preview {
        if !preview.succeeded() {
            return Err(RedtapeError::Blocked(
                "sandbox preview failed; fix the preview command or rerun update before approval"
                    .to_string(),
            ));
        }
    }

    let sandbox_worktree = PathBuf::from(&summary.sandbox.worktree_path);
    let patch_record = summary
        .find_patch_mut(&args.id)
        .ok_or_else(|| RedtapeError::Usage(format!("unknown triage patch id: {}", args.id)))?;

    if !matches!(
        patch_record.triage_status.as_str(),
        "CLEAN" | "pending-review"
    ) {
        return Err(RedtapeError::Blocked(format!(
            "patch {} is not ready for approval (status: {})",
            patch_record.id, patch_record.triage_status
        )));
    }

    let commit = patch_record.apply_commit.clone().ok_or_else(|| {
        RedtapeError::Blocked(format!(
            "patch {} has no staged sandbox commit to approve",
            patch_record.id
        ))
    })?;

    let patch_id: PatchId = args.id.parse()?;
    let diff_text = git::show_commit_patch(&sandbox_worktree, &commit)?;
    atomic::write_file_atomic(
        &patch::diff_path(&paths, patch_id),
        ensure_trailing_newline(&diff_text).as_bytes(),
    )?;

    let mut meta = patch::read_meta_for_id(&paths, patch_id)?.ok_or_else(|| {
        RedtapeError::Validation(format!("missing meta for {} during approval", args.id))
    })?;
    meta.base_ref = to_ref_resolved.clone();
    meta.current_ref = to_ref_resolved.clone();
    meta.apply_confidence = patch_record.confidence.unwrap_or(1.0);
    meta.last_applied = time::current_utc_rfc3339();
    meta.last_checked = meta.last_applied.clone();
    write_meta(&patch::meta_path(&paths, patch_id), &meta)?;

    let (_, mut document) = patch::read_document(&paths)?;
    document.header = patch::patch_md::rewrite_header_base_ref(&document.header, &to_ref_resolved);
    let mut final_status = "active".to_string();
    for entry in &mut document.entries {
        if entry.id == patch_id {
            if !matches!(entry.status.as_str(), "deprecated" | "merged-upstream") {
                entry.status = "active".to_string();
            }
            if let Some(scope_update) = &patch_record.scope_update {
                entry.scope_files = scope_update.files.clone();
                entry.scope_components = scope_update.components.clone();
            }
            final_status = entry.status.clone();
            break;
        }
    }
    let rendered = patch::patch_md::render_document(&document);
    atomic::write_file_atomic(&paths.patch_md_path, rendered.as_bytes())?;

    patch_record.approved = true;
    triage::write(&paths.triage_path, &summary)?;
    triage::write(&sandbox_triage_path, &summary)?;

    let cycle_complete = summary.all_terminal();
    if cycle_complete {
        append_complete_log(&paths, &summary)?;
        run_hook(
            &config.hooks.post_update,
            &paths.repo_root,
            &[
                ("T3_TAPE_REPO_ROOT", paths.repo_root.display().to_string()),
                ("T3_TAPE_STATE_DIR", paths.state_dir.display().to_string()),
                ("T3_TAPE_SANDBOX_PATH", summary.sandbox.path.clone()),
                (
                    "T3_TAPE_TRIAGE_PATH",
                    paths.triage_path.display().to_string(),
                ),
            ],
        )?;
    }

    Ok(ApprovalOutcome {
        patch_id: args.id.clone(),
        status: final_status,
        cycle_complete,
    })
}

pub fn rederive_patch(
    global: &GlobalOptions,
    args: &RederiveArgs,
) -> Result<TriageSummary, RedtapeError> {
    let paths = patch::resolve_paths(global)?;
    let config = schema::read_config(&paths.config_path)?;
    let mut summary = triage::read(&paths.triage_path)?;
    let (_, document) = patch::read_document(&paths)?;
    let entry = document
        .find(args.id.parse()?)
        .cloned()
        .ok_or_else(|| RedtapeError::Usage(format!("unknown patch id: {}", args.id)))?;

    if !config.agent.is_configured() {
        return Err(RedtapeError::Blocked("agent not configured".to_string()));
    }

    let sandbox = SandboxContext {
        timestamp: summary.timestamp.clone(),
        root: PathBuf::from(&summary.sandbox.path),
        triage_path: PathBuf::from(&summary.sandbox.path).join("triage.json"),
        resolved_dir: PathBuf::from(&summary.sandbox.path).join("resolved"),
        preview_dir: PathBuf::from(&summary.sandbox.path).join("preview"),
        worktree_path: PathBuf::from(&summary.sandbox.worktree_path),
        branch: summary.sandbox.worktree_branch.clone(),
    };

    let patch_record = summary
        .find_patch_mut(&args.id)
        .ok_or_else(|| RedtapeError::Usage(format!("unknown triage patch id: {}", args.id)))?;

    increment_agent_attempts(&paths, entry.id)?;
    let new_source = load_new_source(&sandbox.worktree_path, std::slice::from_ref(&entry.surface))?;
    let threshold = config.agent.confidence_threshold;
    resolve::rederive(
        &config.agent,
        &sandbox,
        patch_record,
        resolve::RederivationInput {
            intent: &entry.intent,
            behavior_assertions: &entry.behavior_assertions,
            new_source: &new_source,
            surface_hint: &entry.surface,
            threshold,
        },
    )?;

    if patch_record.triage_status == "pending-review" {
        if let Some(diff_path) = &patch_record.resolved_diff_path {
            let commit = apply::apply_and_commit(
                &sandbox.worktree_path,
                Path::new(diff_path),
                &patch_record.id,
                &patch_record.title,
            )?;
            patch_record.apply_commit = Some(commit);
        }
    }

    triage::write(&paths.triage_path, &summary)?;
    triage::write(&sandbox.triage_path, &summary)?;
    Ok(summary)
}

fn ensure_repo_valid(paths: &crate::store::paths::ResolvedPaths) -> Result<(), RedtapeError> {
    let report = full::validate(paths)?;
    if report.is_ok() {
        Ok(())
    } else {
        Err(RedtapeError::Validation(report.errors.join("; ")))
    }
}

fn load_patch_material(
    paths: &crate::store::paths::ResolvedPaths,
    entry: &PatchEntry,
) -> Result<PatchMaterial, RedtapeError> {
    let diff_path = patch::diff_path(paths, entry.id);
    let raw_diff = fs::read_to_string(&diff_path)?;
    let parsed = UnifiedDiff::parse(&raw_diff)?;
    Ok(PatchMaterial {
        entry: entry.clone(),
        diff_path,
        raw_diff,
        changed_paths: parsed.changed_paths(),
    })
}

fn classify_patch(
    sandbox: &SandboxContext,
    material: &PatchMaterial,
) -> Result<TriagePatch, RedtapeError> {
    let mut missing_surface = false;
    for changed_path in &material.changed_paths {
        if !path_in_worktree(&sandbox.worktree_path, changed_path).exists() {
            missing_surface = true;
            break;
        }
    }

    let (detected_status, apply_stderr) = if missing_surface {
        ("MISSING-SURFACE".to_string(), String::new())
    } else {
        match git::apply_check(&sandbox.worktree_path, &material.diff_path, false) {
            Ok(()) => ("CLEAN".to_string(), String::new()),
            Err(stderr) => ("CONFLICT".to_string(), truncate_message(&stderr)),
        }
    };

    let merged_upstream_candidate = detected_status != "CLEAN"
        && git::apply_check(&sandbox.worktree_path, &material.diff_path, true).is_ok();

    Ok(TriagePatch {
        id: material.entry.id.to_string(),
        title: material.entry.title.clone(),
        detected_status: detected_status.clone(),
        triage_status: detected_status,
        merged_upstream_candidate,
        apply_stderr,
        confidence: None,
        agent_mode: None,
        notes: None,
        unresolved: Vec::new(),
        resolved_diff_path: None,
        notes_path: None,
        raw_response_path: None,
        apply_commit: None,
        approved: false,
        scope_update: None,
    })
}

fn resolve_non_clean_patches(
    context: &ResolutionContext<'_>,
    summary: &mut TriageSummary,
) -> Result<(), RedtapeError> {
    for patch in &mut summary.patches {
        if patch.detected_status == "CLEAN" {
            continue;
        }

        if !context.config.agent.is_configured() {
            patch.triage_status = "NEEDS-YOU".to_string();
            patch.notes = Some("agent not configured".to_string());
            continue;
        }

        let patch_id: PatchId = patch.id.parse()?;
        let meta = patch::read_meta_for_id(context.paths, patch_id)?.ok_or_else(|| {
            RedtapeError::Validation(format!("missing meta for {} during update", patch.id))
        })?;
        if meta.agent_attempts >= u32::from(context.config.agent.max_attempts) {
            patch.triage_status = "NEEDS-YOU".to_string();
            patch.notes = Some(format!("agent attempt budget exhausted for {}", patch.id));
            continue;
        }

        increment_agent_attempts(context.paths, patch_id)?;
        let material = context.materials.get(&patch.id).ok_or_else(|| {
            RedtapeError::Validation(format!("missing patch material for {}", patch.id))
        })?;
        let new_source = load_new_source(&context.sandbox.worktree_path, &material.changed_paths)?;
        let upstream_diff = diff_between_refs(
            &context.paths.repo_root,
            context.from_ref,
            context.to_ref_resolved,
            &material.changed_paths,
        )?;

        let resolution = match patch.detected_status.as_str() {
            "CONFLICT" => resolve::resolve_conflict(
                &context.config.agent,
                context.sandbox,
                patch,
                resolve::ConflictResolutionInput {
                    intent: &material.entry.intent,
                    behavior_assertions: &material.entry.behavior_assertions,
                    original_diff: &material.raw_diff,
                    upstream_diff: &upstream_diff,
                    new_source: &new_source,
                    threshold: context.threshold,
                },
            ),
            _ => resolve::rederive(
                &context.config.agent,
                context.sandbox,
                patch,
                resolve::RederivationInput {
                    intent: &material.entry.intent,
                    behavior_assertions: &material.entry.behavior_assertions,
                    new_source: &new_source,
                    surface_hint: &material.entry.surface,
                    threshold: context.threshold,
                },
            ),
        };

        if let Err(err) = resolution {
            patch.triage_status = "NEEDS-YOU".to_string();
            patch.notes = Some(err.to_string());
        }
    }

    Ok(())
}

fn apply_resolved_patches(
    sandbox: &SandboxContext,
    materials: &BTreeMap<String, PatchMaterial>,
    summary: &mut TriageSummary,
) -> Result<(), RedtapeError> {
    for patch in &mut summary.patches {
        let maybe_diff = match patch.triage_status.as_str() {
            "CLEAN" => materials
                .get(&patch.id)
                .map(|material| material.diff_path.clone()),
            "pending-review" => patch.resolved_diff_path.as_ref().map(PathBuf::from),
            _ => None,
        };

        let Some(diff_path) = maybe_diff else {
            continue;
        };

        let commit =
            apply::apply_and_commit(&sandbox.worktree_path, &diff_path, &patch.id, &patch.title)?;
        patch.apply_commit = Some(commit);
        if patch.confidence.is_none() {
            patch.confidence = Some(1.0);
        }
    }

    Ok(())
}

fn maybe_run_preview(
    config: &Config,
    sandbox: &SandboxContext,
    summary: &mut TriageSummary,
) -> Result<(), RedtapeError> {
    if config.sandbox.preview_command.trim().is_empty() {
        return Ok(());
    }

    fs::create_dir_all(&sandbox.preview_dir)?;
    let stdout_path = sandbox.preview_dir.join("stdout.log");
    let stderr_path = sandbox.preview_dir.join("stderr.log");

    let output = if cfg!(windows) {
        Command::new("cmd")
            .arg("/C")
            .arg(&config.sandbox.preview_command)
            .current_dir(&sandbox.worktree_path)
            .output()?
    } else {
        Command::new("sh")
            .arg("-lc")
            .arg(&config.sandbox.preview_command)
            .current_dir(&sandbox.worktree_path)
            .output()?
    };

    atomic::write_file_atomic(&stdout_path, &output.stdout)?;
    atomic::write_file_atomic(&stderr_path, &output.stderr)?;

    summary.preview = Some(PreviewSummary {
        command: config.sandbox.preview_command.clone(),
        exit_code: output.status.code().unwrap_or(1),
        stdout_path: stdout_path.display().to_string(),
        stderr_path: stderr_path.display().to_string(),
    });

    Ok(())
}

fn increment_agent_attempts(
    paths: &crate::store::paths::ResolvedPaths,
    patch_id: PatchId,
) -> Result<(), RedtapeError> {
    let mut meta = patch::read_meta_for_id(paths, patch_id)?
        .ok_or_else(|| RedtapeError::Validation(format!("missing meta for {}", patch_id)))?;
    meta.agent_attempts += 1;
    write_meta(&patch::meta_path(paths, patch_id), &meta)
}

fn write_meta(path: &Path, meta: &crate::patch::PatchMeta) -> Result<(), RedtapeError> {
    let mut rendered = serde_json::to_string_pretty(meta)
        .map_err(|err| RedtapeError::Validation(format!("failed to serialize meta: {err}")))?;
    rendered.push('\n');
    atomic::write_file_atomic(path, rendered.as_bytes())
}

fn load_new_source(worktree_path: &Path, changed_paths: &[String]) -> Result<String, RedtapeError> {
    let mut sections = Vec::new();
    for changed_path in changed_paths {
        let file_path = path_in_worktree(worktree_path, changed_path);
        if file_path.exists() {
            let contents = fs::read_to_string(&file_path)?;
            sections.push(format!("FILE: {changed_path}\n{contents}"));
        }
    }
    Ok(sections.join("\n---\n"))
}

fn diff_between_refs(
    repo_root: &Path,
    from_ref: &str,
    to_ref: &str,
    changed_paths: &[String],
) -> Result<String, RedtapeError> {
    let mut command = Command::new("git");
    command.arg("diff").arg(from_ref).arg(to_ref).arg("--");
    for changed_path in changed_paths {
        command.arg(changed_path);
    }
    let output = command.current_dir(repo_root).output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(RedtapeError::Git(if stderr.is_empty() {
            format!("git diff {from_ref} {to_ref} failed")
        } else {
            stderr
        }));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn path_in_worktree(worktree_path: &Path, relative: &str) -> PathBuf {
    let mut path = worktree_path.to_path_buf();
    for segment in relative.split('/') {
        if !segment.is_empty() {
            path.push(segment);
        }
    }
    path
}

fn truncate_message(value: &str) -> String {
    const MAX_LEN: usize = 240;
    let trimmed = value.trim();
    if trimmed.len() <= MAX_LEN {
        trimmed.to_string()
    } else {
        let mut end = MAX_LEN;
        while !trimmed.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}...", &trimmed[..end])
    }
}

fn append_started_log(
    paths: &crate::store::paths::ResolvedPaths,
    sandbox: &SandboxContext,
    from_ref: &str,
    to_ref: &str,
) -> Result<(), RedtapeError> {
    append_log(
        &paths.migration_log_path,
        &[
            format!("[{}] UPDATE CYCLE", time::current_utc_rfc3339()),
            format!("  from-ref: {from_ref}"),
            format!("  to-ref:   {to_ref}"),
            format!("  sandbox:  {}", sandbox.root.display()),
            "  status:   STARTED".to_string(),
            "---".to_string(),
        ],
    )
}

fn append_triaged_log(
    paths: &crate::store::paths::ResolvedPaths,
    summary: &TriageSummary,
) -> Result<(), RedtapeError> {
    let counts = summary.counts();
    append_log(
        &paths.migration_log_path,
        &[
            format!("[{}] TRIAGED", time::current_utc_rfc3339()),
            format!("  from-ref: {}", summary.from_ref),
            format!("  to-ref:   {}", summary.to_ref_resolved),
            format!(
                "  clean:    {}",
                counts
                    .iter()
                    .find(|(label, _)| label == "CLEAN")
                    .map(|(_, count)| *count)
                    .unwrap_or(0)
            ),
            format!(
                "  review:   {}",
                counts
                    .iter()
                    .find(|(label, _)| label == "pending-review")
                    .map(|(_, count)| *count)
                    .unwrap_or(0)
            ),
            format!(
                "  needs:    {}",
                counts
                    .iter()
                    .find(|(label, _)| label == "NEEDS-YOU")
                    .map(|(_, count)| *count)
                    .unwrap_or(0)
            ),
            format!("  sandbox:  {}", summary.sandbox.path),
            "  status:   TRIAGED".to_string(),
            "---".to_string(),
        ],
    )
}

fn append_complete_log(
    paths: &crate::store::paths::ResolvedPaths,
    summary: &TriageSummary,
) -> Result<(), RedtapeError> {
    let clean = summary
        .patches
        .iter()
        .filter(|patch| patch.detected_status == "CLEAN")
        .count();
    let resolved = summary
        .patches
        .iter()
        .filter(|patch| {
            patch.agent_mode.as_deref() == Some("conflict-resolution") && patch.approved
        })
        .count();
    let rederived = summary
        .patches
        .iter()
        .filter(|patch| patch.agent_mode.as_deref() == Some("re-derivation") && patch.approved)
        .count();
    let failed = summary
        .patches
        .iter()
        .filter(|patch| patch.triage_status == "NEEDS-YOU")
        .count();
    append_log(
        &paths.migration_log_path,
        &[
            format!("[{}] UPDATE CYCLE", time::current_utc_rfc3339()),
            format!("  from-ref: {}", summary.from_ref),
            format!("  to-ref:   {}", summary.to_ref_resolved),
            format!("  patches:  {} active", summary.patches.len()),
            format!("  clean:    {clean}"),
            format!("  resolved: {resolved}"),
            format!("  rederived: {rederived}"),
            format!("  failed:   {failed}"),
            format!("  sandbox:  {}", summary.sandbox.path),
            "  status:   COMPLETE".to_string(),
            "---".to_string(),
        ],
    )
}

fn append_log(path: &Path, lines: &[String]) -> Result<(), RedtapeError> {
    let mut existing = if path.exists() {
        fs::read_to_string(path)?
    } else {
        String::new()
    };
    if !existing.is_empty() && !existing.ends_with('\n') {
        existing.push('\n');
    }
    existing.push_str(&lines.join("\n"));
    existing.push('\n');
    atomic::write_file_atomic(path, existing.as_bytes())
}

fn run_hook(command: &str, cwd: &Path, envs: &[(&str, String)]) -> Result<(), RedtapeError> {
    if command.trim().is_empty() {
        return Ok(());
    }

    let mut process = if cfg!(windows) {
        let mut child = Command::new("cmd");
        child.arg("/C").arg(command);
        child
    } else {
        let mut child = Command::new("sh");
        child.arg("-lc").arg(command);
        child
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

fn ensure_trailing_newline(value: &str) -> String {
    if value.ends_with('\n') {
        value.to_string()
    } else {
        format!("{value}\n")
    }
}
