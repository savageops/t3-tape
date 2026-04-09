use std::fs;

use crate::agent;
use crate::agent::schema::{
    ConflictResolutionRequest, ConflictResolutionResponse, RederivationRequest,
    RederivationResponse, ScopeUpdate,
};
use crate::exit::RedtapeError;
use crate::store::atomic;
use crate::store::schema::AgentConfig;

use super::sandbox::SandboxContext;
use super::triage::TriagePatch;

pub struct ConflictResolutionInput<'a> {
    pub intent: &'a str,
    pub behavior_assertions: &'a [String],
    pub original_diff: &'a str,
    pub upstream_diff: &'a str,
    pub new_source: &'a str,
    pub threshold: f64,
}

pub struct RederivationInput<'a> {
    pub intent: &'a str,
    pub behavior_assertions: &'a [String],
    pub new_source: &'a str,
    pub surface_hint: &'a str,
    pub threshold: f64,
}

struct ResolutionArtifacts<'a> {
    diff: String,
    confidence: f64,
    notes: String,
    unresolved: Vec<String>,
    truncated: bool,
    scope_update: Option<ScopeUpdate>,
    agent_mode: &'a str,
}

pub fn resolve_conflict(
    config: &AgentConfig,
    sandbox: &SandboxContext,
    patch: &mut TriagePatch,
    input: ConflictResolutionInput<'_>,
) -> Result<(), RedtapeError> {
    let (new_source, truncated) = agent::truncate_source(input.new_source);
    let request = ConflictResolutionRequest {
        mode: "conflict-resolution".to_string(),
        patch_id: patch.id.clone(),
        intent: input.intent.to_string(),
        behavior_assertions: input.behavior_assertions.to_vec(),
        original_diff: input.original_diff.to_string(),
        upstream_diff: input.upstream_diff.to_string(),
        new_source,
    };

    let response: ConflictResolutionResponse = agent::send_request(config, &request)?;
    write_resolution_artifacts(
        sandbox,
        patch,
        ResolutionArtifacts {
            diff: response.resolved_diff,
            confidence: response.confidence,
            notes: response.notes,
            unresolved: response.unresolved,
            truncated,
            scope_update: None,
            agent_mode: "conflict-resolution",
        },
        input.threshold,
    )
}

pub fn rederive(
    config: &AgentConfig,
    sandbox: &SandboxContext,
    patch: &mut TriagePatch,
    input: RederivationInput<'_>,
) -> Result<(), RedtapeError> {
    let (new_source, truncated) = agent::truncate_source(input.new_source);
    let request = RederivationRequest {
        mode: "re-derivation".to_string(),
        patch_id: patch.id.clone(),
        intent: input.intent.to_string(),
        behavior_assertions: input.behavior_assertions.to_vec(),
        new_source,
        surface_hint: input.surface_hint.to_string(),
    };

    let response: RederivationResponse = agent::send_request(config, &request)?;
    write_resolution_artifacts(
        sandbox,
        patch,
        ResolutionArtifacts {
            diff: response.derived_diff,
            confidence: response.confidence,
            notes: response.notes,
            unresolved: response.unresolved,
            truncated,
            scope_update: Some(response.scope_update),
            agent_mode: "re-derivation",
        },
        input.threshold,
    )
}

fn write_resolution_artifacts(
    sandbox: &SandboxContext,
    patch: &mut TriagePatch,
    artifacts: ResolutionArtifacts<'_>,
    threshold: f64,
) -> Result<(), RedtapeError> {
    fs::create_dir_all(&sandbox.resolved_dir)?;
    let diff_path = sandbox.resolved_dir.join(format!("{}.diff", patch.id));
    let notes_path = sandbox.resolved_dir.join(format!("{}.notes.txt", patch.id));
    let raw_response_path = sandbox.resolved_dir.join(format!("{}.json", patch.id));

    let note_body = if artifacts.truncated {
        format!(
            "{}\n\n[t3-tape truncated new-source before sending the request]\n",
            artifacts.notes
        )
    } else {
        artifacts.notes.clone()
    };

    let raw_response = serde_json::json!({
        "confidence": artifacts.confidence,
        "notes": artifacts.notes,
        "unresolved": artifacts.unresolved,
        "scope-update": artifacts.scope_update,
        "agent-mode": artifacts.agent_mode,
        "diff": artifacts.diff,
    });

    atomic::write_file_atomic(
        &diff_path,
        ensure_trailing_newline(&artifacts.diff).as_bytes(),
    )?;
    atomic::write_file_atomic(&notes_path, ensure_trailing_newline(&note_body).as_bytes())?;
    let mut raw_json = serde_json::to_string_pretty(&raw_response)
        .map_err(|err| RedtapeError::Agent(format!("failed to serialize agent response: {err}")))?;
    raw_json.push('\n');
    atomic::write_file_atomic(&raw_response_path, raw_json.as_bytes())?;

    patch.confidence = Some(artifacts.confidence);
    patch.agent_mode = Some(artifacts.agent_mode.to_string());
    patch.notes = Some(note_body.trim().to_string());
    patch.unresolved = artifacts.unresolved;
    patch.resolved_diff_path = Some(diff_path.display().to_string());
    patch.notes_path = Some(notes_path.display().to_string());
    patch.raw_response_path = Some(raw_response_path.display().to_string());
    patch.scope_update = artifacts.scope_update;
    patch.triage_status = if artifacts.confidence >= threshold {
        "pending-review".to_string()
    } else {
        "NEEDS-YOU".to_string()
    };

    Ok(())
}

fn ensure_trailing_newline(value: &str) -> String {
    if value.ends_with('\n') {
        value.to_string()
    } else {
        format!("{value}\n")
    }
}
