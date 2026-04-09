pub mod full;
pub mod staged;

use std::path::Path;

use serde::Serialize;

use crate::patch::PatchId;
use crate::store::paths::ResolvedPaths;
use crate::store::schema::PROTOCOL_VERSION;

pub const ALLOWED_PATCH_STATUSES: &[&str] = &[
    "active",
    "deprecated",
    "merged-upstream",
    "conflict",
    "pending-review",
];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct ValidationReport {
    pub schema_version: String,
    pub status: String,
    pub repo_root: String,
    pub state_dir: String,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

impl ValidationReport {
    pub fn new(paths: &ResolvedPaths) -> Self {
        Self {
            schema_version: PROTOCOL_VERSION.to_string(),
            status: "ok".to_string(),
            repo_root: display_path(&paths.repo_root),
            state_dir: display_path(&paths.state_dir),
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    pub fn push_error(&mut self, message: impl Into<String>) {
        self.errors.push(message.into());
    }

    pub fn push_warning(&mut self, message: impl Into<String>) {
        self.warnings.push(message.into());
    }

    pub fn refresh_status(&mut self) {
        self.status = if self.errors.is_empty() {
            "ok".to_string()
        } else {
            "error".to_string()
        };
    }

    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }
}

pub fn render_human(report: &ValidationReport) -> String {
    if report.errors.is_empty() {
        return "OK\n".to_string();
    }

    let mut lines = Vec::new();
    for error in &report.errors {
        lines.push(format!("ERROR: {error}"));
    }
    for warning in &report.warnings {
        lines.push(format!("WARNING: {warning}"));
    }

    let mut rendered = lines.join("\n");
    rendered.push('\n');
    rendered
}

pub fn render_json(report: &ValidationReport) -> Result<String, crate::exit::RedtapeError> {
    let mut rendered = serde_json::to_string_pretty(report).map_err(|err| {
        crate::exit::RedtapeError::Validation(format!(
            "failed to serialize validation report: {err}"
        ))
    })?;
    rendered.push('\n');
    Ok(rendered)
}

pub fn display_path(path: &Path) -> String {
    path.display().to_string()
}

pub fn repo_relative(repo_root: &Path, path: &Path) -> Option<String> {
    let relative = path.strip_prefix(repo_root).ok()?;
    let normalized = normalize_relative_path(relative);
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

pub fn normalize_relative_path(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy().replace('\\', "/"))
        .collect::<Vec<_>>()
        .join("/")
}

pub fn join_relative(prefix: Option<&str>, suffix: &str) -> String {
    match prefix {
        Some(prefix) if !prefix.is_empty() => format!("{prefix}/{suffix}"),
        _ => suffix.to_string(),
    }
}

pub fn expected_diff_file(id: PatchId) -> String {
    format!("patches/{id}.diff")
}

pub fn is_allowed_patch_status(status: &str) -> bool {
    ALLOWED_PATCH_STATUSES.contains(&status)
}

pub fn child_relative_path(base: Option<&str>, candidate: &str) -> bool {
    match base {
        Some(base) if !base.is_empty() => {
            candidate == base || candidate.starts_with(&format!("{base}/"))
        }
        _ => true,
    }
}
