use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::agent::schema::ScopeUpdate;
use crate::exit::RedtapeError;
use crate::store::atomic;
use crate::store::schema::PROTOCOL_VERSION;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct TriageSummary {
    pub schema_version: String,
    pub from_ref: String,
    pub to_ref: String,
    pub to_ref_resolved: String,
    pub upstream: String,
    pub timestamp: String,
    pub sandbox: SandboxSummary,
    pub patches: Vec<TriagePatch>,
    #[serde(default)]
    pub preview: Option<PreviewSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct SandboxSummary {
    pub path: String,
    pub worktree_branch: String,
    pub worktree_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct PreviewSummary {
    pub command: String,
    pub exit_code: i32,
    pub stdout_path: String,
    pub stderr_path: String,
}

impl PreviewSummary {
    pub fn succeeded(&self) -> bool {
        self.exit_code == 0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct TriagePatch {
    pub id: String,
    pub title: String,
    pub detected_status: String,
    pub triage_status: String,
    pub merged_upstream_candidate: bool,
    pub apply_stderr: String,
    #[serde(default)]
    pub confidence: Option<f64>,
    #[serde(default)]
    pub agent_mode: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub unresolved: Vec<String>,
    #[serde(default)]
    pub dependency_blockers: Vec<String>,
    #[serde(default)]
    pub resolved_diff_path: Option<String>,
    #[serde(default)]
    pub notes_path: Option<String>,
    #[serde(default)]
    pub raw_response_path: Option<String>,
    #[serde(default)]
    pub apply_commit: Option<String>,
    #[serde(default)]
    pub approved: bool,
    #[serde(default)]
    pub scope_update: Option<ScopeUpdate>,
}

impl TriageSummary {
    pub fn new(
        from_ref: String,
        to_ref: String,
        to_ref_resolved: String,
        upstream: String,
        timestamp: String,
        sandbox: SandboxSummary,
        patches: Vec<TriagePatch>,
    ) -> Self {
        Self {
            schema_version: PROTOCOL_VERSION.to_string(),
            from_ref,
            to_ref,
            to_ref_resolved,
            upstream,
            timestamp,
            sandbox,
            patches,
            preview: None,
        }
    }

    pub fn counts(&self) -> Vec<(String, usize)> {
        let mut clean = 0;
        let mut conflict = 0;
        let mut missing_surface = 0;
        let mut pending_review = 0;
        let mut needs_you = 0;

        for patch in &self.patches {
            match patch.triage_status.as_str() {
                "CLEAN" => clean += 1,
                "CONFLICT" => conflict += 1,
                "MISSING-SURFACE" => missing_surface += 1,
                "pending-review" => pending_review += 1,
                "NEEDS-YOU" => needs_you += 1,
                _ => {}
            }
        }

        vec![
            ("CLEAN".to_string(), clean),
            ("CONFLICT".to_string(), conflict),
            ("MISSING-SURFACE".to_string(), missing_surface),
            ("pending-review".to_string(), pending_review),
            ("NEEDS-YOU".to_string(), needs_you),
        ]
    }

    pub fn all_terminal(&self) -> bool {
        self.patches
            .iter()
            .all(|patch| match patch.triage_status.as_str() {
                "NEEDS-YOU" | "CONFLICT" | "MISSING-SURFACE" => false,
                "CLEAN" | "pending-review" => patch.approved,
                _ => patch.approved,
            })
    }

    pub fn find_patch_mut(&mut self, id: &str) -> Option<&mut TriagePatch> {
        self.patches.iter_mut().find(|patch| patch.id == id)
    }

    pub fn find_patch(&self, id: &str) -> Option<&TriagePatch> {
        self.patches.iter().find(|patch| patch.id == id)
    }
}

pub fn read(path: &Path) -> Result<TriageSummary, RedtapeError> {
    let content = fs::read_to_string(path)?;
    serde_json::from_str(&content)
        .map_err(|err| RedtapeError::Validation(format!("invalid triage summary: {err}")))
}

pub fn write(path: &Path, summary: &TriageSummary) -> Result<(), RedtapeError> {
    let mut rendered = serde_json::to_string_pretty(summary).map_err(|err| {
        RedtapeError::Validation(format!("failed to serialize triage summary: {err}"))
    })?;
    rendered.push('\n');
    atomic::write_file_atomic(path, rendered.as_bytes())
}

pub fn render_human(summary: &TriageSummary) -> String {
    let mut lines = Vec::new();
    for (label, count) in summary.counts() {
        lines.push(format!("{label}\t{count}"));
    }
    for patch in &summary.patches {
        let status = if patch.detected_status == patch.triage_status {
            patch.triage_status.clone()
        } else {
            format!("{} (from {})", patch.triage_status, patch.detected_status)
        };
        lines.push(format!(
            "{}\t{}\t{}{}{}",
            patch.id,
            patch.title,
            status,
            if patch.approved { "\tapproved" } else { "" },
            if patch.dependency_blockers.is_empty() {
                String::new()
            } else {
                format!("\tblocked-by={}", patch.dependency_blockers.join(","))
            }
        ));
    }

    let mut rendered = lines.join("\n");
    rendered.push('\n');
    rendered
}
