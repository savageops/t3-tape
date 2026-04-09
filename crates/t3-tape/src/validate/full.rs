use std::collections::{BTreeSet, HashMap};
use std::fs;

use serde_json::Value;

use crate::exit::RedtapeError;
use crate::patch::{self, PatchDocument};
use crate::store::paths::ResolvedPaths;
use crate::store::schema::{self, PROTOCOL_VERSION};

use super::{display_path, expected_diff_file, is_allowed_patch_status, ValidationReport};

pub fn validate(paths: &ResolvedPaths) -> Result<ValidationReport, RedtapeError> {
    let mut report = ValidationReport::new(paths);

    validate_state_dir(paths, &mut report)?;
    validate_config(paths, &mut report)?;
    validate_migration_log(paths, &mut report);
    validate_triage(paths, &mut report)?;

    if let Some(document) = validate_patch_document(paths, &mut report)? {
        validate_patch_entries(paths, &document, &mut report)?;
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

    if let Err(err) = schema::read_config(&paths.config_path) {
        report.push_error(err.to_string().replace("validation failed: ", ""));
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
) -> Result<(), RedtapeError> {
    if !paths.triage_path.exists() {
        return Ok(());
    }

    if !paths.triage_path.is_file() {
        report.push_error(format!(
            "expected triage summary file at {}",
            display_path(&paths.triage_path)
        ));
        return Ok(());
    }

    let raw = fs::read_to_string(&paths.triage_path)?;
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed == "{}" {
        return Ok(());
    }

    let value: Value = match serde_json::from_str(&raw) {
        Ok(value) => value,
        Err(err) => {
            report.push_error(format!(
                "invalid triage summary JSON at {}: {err}",
                display_path(&paths.triage_path)
            ));
            return Ok(());
        }
    };

    let Some(object) = value.as_object() else {
        report.push_error(format!(
            "triage summary must be a JSON object at {}",
            display_path(&paths.triage_path)
        ));
        return Ok(());
    };

    match object
        .get("schema-version")
        .and_then(|value| value.as_str())
    {
        Some(PROTOCOL_VERSION) => Ok(()),
        Some(other) => {
            report.push_error(format!(
                "triage summary schema-version must be {} but found {other}",
                PROTOCOL_VERSION
            ));
            Ok(())
        }
        None => {
            report.push_error(format!(
                "triage summary missing schema-version at {}",
                display_path(&paths.triage_path)
            ));
            Ok(())
        }
    }
}

fn validate_patch_document(
    paths: &ResolvedPaths,
    report: &mut ValidationReport,
) -> Result<Option<PatchDocument>, RedtapeError> {
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

    let mut ids = BTreeSet::new();
    for entry in &document.entries {
        if !ids.insert(entry.id.to_string()) {
            report.push_error(format!("duplicate patch id in patch.md: {}", entry.id));
        }
    }

    Ok(Some(document))
}

fn validate_patch_entries(
    paths: &ResolvedPaths,
    document: &PatchDocument,
    report: &mut ValidationReport,
) -> Result<(), RedtapeError> {
    let known_ids = document
        .entries
        .iter()
        .map(|entry| entry.id.to_string())
        .collect::<BTreeSet<_>>();

    for entry in &document.entries {
        if !is_allowed_patch_status(&entry.status) {
            report.push_error(format!(
                "patch {} uses unsupported status `{}`",
                entry.id, entry.status
            ));
        }

        let diff_path = patch::diff_path(paths, entry.id);
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

    if let Some(cycle) = detect_dependency_cycle(document) {
        report.push_error(format!(
            "patch dependency cycle detected: {}",
            cycle.join(" -> ")
        ));
    }

    Ok(())
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
