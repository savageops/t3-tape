use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::process::Command;

use serde_json::Value;

use crate::exit::RedtapeError;
use crate::patch::{self, PatchDocument, UnifiedDiff};
use crate::store::paths::ResolvedPaths;
use crate::store::schema::{self, PROTOCOL_VERSION};
use crate::update::triage::TriageSummary;

use super::{display_path, expected_diff_file, is_allowed_patch_status, ValidationReport};

struct PatchDocumentState {
    document: PatchDocument,
    header: Option<patch::patch_md::PatchHeader>,
}

pub fn validate(paths: &ResolvedPaths) -> Result<ValidationReport, RedtapeError> {
    let mut report = ValidationReport::new(paths);

    validate_state_dir(paths, &mut report)?;
    validate_config(paths, &mut report)?;
    validate_migration_log(paths, &mut report);
    let triage_summary = validate_triage(paths, &mut report)?;

    if let Some(document_state) = validate_patch_document(paths, &mut report)? {
        validate_patch_entries(paths, &document_state, triage_summary.as_ref(), &mut report)?;
    }

    report.refresh_status();
    Ok(report)
}

fn validate_state_dir(
    paths: &ResolvedPaths,
    report: &mut ValidationReport,
) -> Result<(), RedtapeError> {
    if !paths.state_dir.exists() {
        report.push_error(format!(
            "state dir does not exist: {}",
            display_path(&paths.state_dir)
        ));
        return Ok(());
    }

    if !paths.state_dir.is_dir() {
        report.push_error(format!(
            "state dir is not a directory: {}",
            display_path(&paths.state_dir)
        ));
        return Ok(());
    }

    let _ = fs::read_dir(&paths.state_dir)?;
    Ok(())
}

fn validate_config(
    paths: &ResolvedPaths,
    report: &mut ValidationReport,
) -> Result<(), RedtapeError> {
    if !paths.config_path.exists() {
        report.push_error(format!(
            "missing PatchMD config: {}",
            display_path(&paths.config_path)
        ));
        return Ok(());
    }

    if !paths.config_path.is_file() {
        report.push_error(format!(
            "expected config file at {}",
            display_path(&paths.config_path)
        ));
        return Ok(());
    }

    match schema::read_config(&paths.config_path) {
        Ok(config) => {
            if config.protocol != PROTOCOL_VERSION {
                report.push_error(format!(
                    "config.json protocol mismatch: expected {} but found {}",
                    PROTOCOL_VERSION, config.protocol
                ));
            }
        }
        Err(err) => report.push_error(err.to_string().replace("validation failed: ", "")),
    }

    Ok(())
}

fn validate_migration_log(paths: &ResolvedPaths, report: &mut ValidationReport) {
    if !paths.migration_log_path.exists() {
        report.push_error(format!(
            "missing migration log: {}",
            display_path(&paths.migration_log_path)
        ));
        return;
    }

    if !paths.migration_log_path.is_file() {
        report.push_error(format!(
            "expected migration log file at {}",
            display_path(&paths.migration_log_path)
        ));
    }
}

fn validate_triage(
    paths: &ResolvedPaths,
    report: &mut ValidationReport,
) -> Result<Option<TriageSummary>, RedtapeError> {
    if !paths.triage_path.exists() {
        return Ok(None);
    }

    if !paths.triage_path.is_file() {
        report.push_error(format!(
            "expected triage summary file at {}",
            display_path(&paths.triage_path)
        ));
        return Ok(None);
    }

    let raw = fs::read_to_string(&paths.triage_path)?;
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed == "{}" {
        return Ok(None);
    }

    let value: Value = match serde_json::from_str(&raw) {
        Ok(value) => value,
        Err(err) => {
            report.push_error(format!(
                "invalid triage summary JSON at {}: {err}",
                display_path(&paths.triage_path)
            ));
            return Ok(None);
        }
    };

    let Some(object) = value.as_object() else {
        report.push_error(format!(
            "triage summary must be a JSON object at {}",
            display_path(&paths.triage_path)
        ));
        return Ok(None);
    };

    match object
        .get("schema-version")
        .and_then(|value| value.as_str())
    {
        Some(PROTOCOL_VERSION) => {}
        Some(other) => {
            report.push_error(format!(
                "triage summary schema-version must be {} but found {other}",
                PROTOCOL_VERSION
            ));
            return Ok(None);
        }
        None => {
            report.push_error(format!(
                "triage summary missing schema-version at {}",
                display_path(&paths.triage_path)
            ));
            return Ok(None);
        }
    }

    let summary = serde_json::from_value::<TriageSummary>(value)
        .map_err(|err| RedtapeError::Validation(format!("invalid triage summary: {err}")))?;
    Ok(Some(summary))
}

fn validate_patch_document(
    paths: &ResolvedPaths,
    report: &mut ValidationReport,
) -> Result<Option<PatchDocumentState>, RedtapeError> {
    if !paths.patch_md_path.exists() {
        report.push_error(format!(
            "missing PatchMD registry: {}",
            display_path(&paths.patch_md_path)
        ));
        return Ok(None);
    }

    if !paths.patch_md_path.is_file() {
        report.push_error(format!(
            "expected patch registry file at {}",
            display_path(&paths.patch_md_path)
        ));
        return Ok(None);
    }

    let content = fs::read_to_string(&paths.patch_md_path)?;
    let document = match patch::patch_md::parse(&content) {
        Ok(document) => document,
        Err(err) => {
            report.push_error(err.to_string().replace("validation failed: ", ""));
            return Ok(None);
        }
    };

    let header = match patch::patch_md::parse_header(&document.header) {
        Ok(header) => {
            if header.protocol != PROTOCOL_VERSION {
                report.push_error(format!(
                    "patch.md header protocol mismatch: expected {} but found {}",
                    PROTOCOL_VERSION, header.protocol
                ));
            }
            match header.state_root.as_deref() {
                Some("patch") => {}
                Some(other) => report.push_error(format!(
                    "patch.md header state-root mismatch: expected `patch` but found `{other}`"
                )),
                None => report.push_error("patch.md header missing required state-root `patch`"),
            }
            if header.base_ref.trim().is_empty() {
                report.push_error("patch.md header base-ref cannot be empty");
            }
            Some(header)
        }
        Err(err) => {
            report.push_error(err.to_string().replace("validation failed: ", ""));
            None
        }
    };

    let mut ids = BTreeSet::new();
    for entry in &document.entries {
        if !ids.insert(entry.id.to_string()) {
            report.push_error(format!("duplicate patch id in patch.md: {}", entry.id));
        }
    }

    Ok(Some(PatchDocumentState { document, header }))
}

fn validate_patch_entries(
    paths: &ResolvedPaths,
    state: &PatchDocumentState,
    triage_summary: Option<&TriageSummary>,
    report: &mut ValidationReport,
) -> Result<(), RedtapeError> {
    let known_ids = state
        .document
        .entries
        .iter()
        .map(|entry| entry.id.to_string())
        .collect::<BTreeSet<_>>();

    let mut git_ref_cache = HashMap::<String, bool>::new();

    if let Some(header) = state.header.as_ref() {
        if !git_ref_exists(paths, &header.base_ref, &mut git_ref_cache) {
            report.push_error(format!(
                "patch.md header base-ref does not resolve in git: `{}`",
                header.base_ref
            ));
        }
    }

    validate_triage_ref_consistency(
        paths,
        state.header.as_ref(),
        triage_summary,
        &known_ids,
        &mut git_ref_cache,
        report,
    );

    for entry in &state.document.entries {
        if !is_allowed_patch_status(&entry.status) {
            report.push_error(format!(
                "patch {} uses unsupported status `{}`",
                entry.id, entry.status
            ));
        }

        let diff_path = patch::diff_path(paths, entry.id);
        let mut parsed_diff = None;
        if !diff_path.exists() {
            report.push_error(format!(
                "patch {} is missing diff file at {}",
                entry.id,
                display_path(&diff_path)
            ));
        } else if !diff_path.is_file() {
            report.push_error(format!(
                "patch {} diff path is not a file: {}",
                entry.id,
                display_path(&diff_path)
            ));
        } else {
            let raw_diff = fs::read_to_string(&diff_path)?;
            match UnifiedDiff::parse(&raw_diff) {
                Ok(diff) => parsed_diff = Some(diff),
                Err(err) => {
                    report.push_error(format!("patch {} diff parse failed: {}", entry.id, err))
                }
            }
        }

        let meta_path = patch::meta_path(paths, entry.id);
        if !meta_path.exists() {
            report.push_error(format!(
                "patch {} is missing meta file at {}",
                entry.id,
                display_path(&meta_path)
            ));
        } else if !meta_path.is_file() {
            report.push_error(format!(
                "patch {} meta path is not a file: {}",
                entry.id,
                display_path(&meta_path)
            ));
        }

        if let Some(meta) = patch::read_meta_for_id(paths, entry.id)? {
            validate_meta_parity(
                paths,
                entry,
                &meta,
                parsed_diff.as_ref(),
                state.header.as_ref(),
                triage_summary,
                &mut git_ref_cache,
                report,
            );
        }

        for required in &entry.requires {
            if !known_ids.contains(required) {
                report.push_error(format!(
                    "patch {} requires unknown patch id `{required}`",
                    entry.id
                ));
            }
        }
    }

    if let Some(cycle) = detect_dependency_cycle(&state.document) {
        report.push_error(format!(
            "patch dependency cycle detected: {}",
            cycle.join(" -> ")
        ));
    }

    Ok(())
}

fn validate_meta_parity(
    paths: &ResolvedPaths,
    entry: &patch::PatchEntry,
    meta: &patch::PatchMeta,
    parsed_diff: Option<&UnifiedDiff>,
    _header: Option<&patch::patch_md::PatchHeader>,
    triage_summary: Option<&TriageSummary>,
    git_ref_cache: &mut HashMap<String, bool>,
    report: &mut ValidationReport,
) {
    if meta.id != entry.id.to_string() {
        report.push_error(format!(
            "patch {} meta id mismatch: expected {} but found {}",
            entry.id, entry.id, meta.id
        ));
    }
    if meta.title != entry.title {
        report.push_error(format!(
            "patch {} meta title mismatch: expected `{}` but found `{}`",
            entry.id, entry.title, meta.title
        ));
    }
    if meta.status != entry.status {
        report.push_error(format!(
            "patch {} meta status mismatch: expected `{}` but found `{}`",
            entry.id, entry.status, meta.status
        ));
    }
    let expected_diff = expected_diff_file(entry.id);
    if meta.diff_file != expected_diff {
        report.push_error(format!(
            "patch {} meta diff-file mismatch: expected `{expected_diff}` but found `{}`",
            entry.id, meta.diff_file
        ));
    }

    let expected_ref =
        expected_patch_ref(paths, entry.id.to_string(), triage_summary, git_ref_cache);
    if let Some(expected_ref) = expected_ref {
        if meta.base_ref != expected_ref {
            report.push_error(format!(
                "patch {} meta base-ref mismatch: expected `{expected_ref}` but found `{}`",
                entry.id, meta.base_ref
            ));
        }
        if meta.current_ref != expected_ref {
            report.push_error(format!(
                "patch {} meta current-ref mismatch: expected `{expected_ref}` but found `{}`",
                entry.id, meta.current_ref
            ));
        }
    }

    if meta.base_ref != meta.current_ref {
        report.push_error(format!(
            "patch {} meta refs diverged: base-ref `{}` vs current-ref `{}`",
            entry.id, meta.base_ref, meta.current_ref
        ));
    }

    if !git_ref_exists(paths, &meta.base_ref, git_ref_cache) {
        report.push_error(format!(
            "patch {} meta base-ref does not resolve in git: `{}`",
            entry.id, meta.base_ref
        ));
    }
    if !git_ref_exists(paths, &meta.current_ref, git_ref_cache) {
        report.push_error(format!(
            "patch {} meta current-ref does not resolve in git: `{}`",
            entry.id, meta.current_ref
        ));
    }

    if meta.behavior_assertions != entry.behavior_assertions {
        report.push_error(format!(
            "patch {} meta behavior-assertions mismatch: expected {} but found {}",
            entry.id,
            render_string_list(&entry.behavior_assertions),
            render_string_list(&meta.behavior_assertions)
        ));
    }

    match parsed_diff {
        Some(diff) => {
            let expected_surface_hash = patch::surface_hash::compute(diff);
            if meta.surface_hash.trim().is_empty() {
                report.push_error(format!(
                    "patch {} meta surface-hash cannot be empty",
                    entry.id
                ));
            } else if meta.surface_hash != expected_surface_hash {
                report.push_error(format!(
                    "patch {} meta surface-hash mismatch: expected `{expected_surface_hash}` but found `{}`",
                    entry.id, meta.surface_hash
                ));
            }
        }
        None => {
            if meta.surface_hash.trim().is_empty() {
                report.push_error(format!(
                    "patch {} meta surface-hash cannot be empty",
                    entry.id
                ));
            }
        }
    }
}

fn validate_triage_ref_consistency(
    paths: &ResolvedPaths,
    header: Option<&patch::patch_md::PatchHeader>,
    triage_summary: Option<&TriageSummary>,
    known_ids: &BTreeSet<String>,
    git_ref_cache: &mut HashMap<String, bool>,
    report: &mut ValidationReport,
) {
    let Some(summary) = triage_summary else {
        return;
    };

    let Some(header) = header else {
        return;
    };

    if summary.all_terminal() {
        let expected_header_ref = summary.to_ref_resolved.as_str();
        let header_resolved = resolve_git_ref(paths, &header.base_ref, git_ref_cache);
        let expected_header_resolved = resolve_git_ref(paths, expected_header_ref, git_ref_cache);

        if header_resolved
            .as_deref()
            .unwrap_or(header.base_ref.as_str())
            != expected_header_resolved
                .as_deref()
                .unwrap_or(expected_header_ref)
        {
            report.push_error(format!(
                "patch.md header base-ref mismatch: expected `{expected_header_ref}` but found `{}`",
                header.base_ref
            ));
        }
    }

    if !git_ref_exists(paths, &summary.from_ref, git_ref_cache) {
        report.push_error(format!(
            "triage summary from-ref does not resolve in git: `{}`",
            summary.from_ref
        ));
    }
    if !git_ref_exists(paths, &summary.to_ref_resolved, git_ref_cache) {
        report.push_error(format!(
            "triage summary to-ref-resolved does not resolve in git: `{}`",
            summary.to_ref_resolved
        ));
    }

    for patch in &summary.patches {
        if !known_ids.contains(&patch.id) {
            report.push_error(format!(
                "triage summary references unknown patch id `{}`",
                patch.id
            ));
        }
    }
}

fn expected_patch_ref(
    paths: &ResolvedPaths,
    patch_id: String,
    triage_summary: Option<&TriageSummary>,
    git_ref_cache: &mut HashMap<String, bool>,
) -> Option<String> {
    let Some(summary) = triage_summary else {
        return None;
    };

    let Some(record) = summary.find_patch(&patch_id) else {
        return None;
    };

    if summary.all_terminal() || record.approved {
        Some(
            resolve_git_ref(paths, &summary.to_ref_resolved, git_ref_cache)
                .unwrap_or_else(|| summary.to_ref_resolved.clone()),
        )
    } else {
        None
    }
}

fn render_string_list(values: &[String]) -> String {
    if values.is_empty() {
        "[]".to_string()
    } else {
        format!("[{}]", values.join(", "))
    }
}

fn git_ref_exists(
    paths: &ResolvedPaths,
    ref_name: &str,
    cache: &mut HashMap<String, bool>,
) -> bool {
    resolve_git_ref(paths, ref_name, cache).is_some()
}

fn resolve_git_ref(
    paths: &ResolvedPaths,
    ref_name: &str,
    cache: &mut HashMap<String, bool>,
) -> Option<String> {
    if ref_name.trim().is_empty() {
        return None;
    }

    if cache.get(ref_name).copied() == Some(false) {
        return None;
    }

    let output = Command::new("git")
        .args(["rev-parse", "--verify", &format!("{ref_name}^{{commit}}")])
        .current_dir(&paths.repo_root)
        .output()
        .ok()?;
    if !output.status.success() {
        cache.insert(ref_name.to_string(), false);
        return None;
    }

    let resolved = String::from_utf8_lossy(&output.stdout).trim().to_string();
    cache.insert(ref_name.to_string(), true);
    Some(resolved)
}

fn detect_dependency_cycle(document: &PatchDocument) -> Option<Vec<String>> {
    let graph = document
        .entries
        .iter()
        .map(|entry| (entry.id.to_string(), entry.requires.clone()))
        .collect::<HashMap<_, _>>();

    let mut states = HashMap::<String, u8>::new();
    let mut stack = Vec::<String>::new();

    for id in graph.keys() {
        if states.get(id).copied().unwrap_or_default() == 0 {
            if let Some(cycle) = dfs_cycle(id, &graph, &mut states, &mut stack) {
                return Some(cycle);
            }
        }
    }

    None
}

fn dfs_cycle(
    id: &str,
    graph: &HashMap<String, Vec<String>>,
    states: &mut HashMap<String, u8>,
    stack: &mut Vec<String>,
) -> Option<Vec<String>> {
    states.insert(id.to_string(), 1);
    stack.push(id.to_string());

    if let Some(neighbors) = graph.get(id) {
        for neighbor in neighbors {
            match states.get(neighbor).copied().unwrap_or_default() {
                0 => {
                    if let Some(cycle) = dfs_cycle(neighbor, graph, states, stack) {
                        return Some(cycle);
                    }
                }
                1 => {
                    if let Some(start) = stack.iter().position(|value| value == neighbor) {
                        let mut cycle = stack[start..].to_vec();
                        cycle.push(neighbor.clone());
                        return Some(cycle);
                    }
                }
                _ => {}
            }
        }
    }

    stack.pop();
    states.insert(id.to_string(), 2);
    None
}
