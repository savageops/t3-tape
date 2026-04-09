use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::exit::RedtapeError;

pub const PROTOCOL_VERSION: &str = "0.1.0";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct Config {
    pub protocol: String,
    pub upstream: String,
    pub agent: AgentConfig,
    pub sandbox: SandboxConfig,
    pub hooks: HooksConfig,
    pub commit_both_layers: bool,
    pub gitignore_sandbox: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct AgentConfig {
    #[serde(default)]
    pub provider: String,
    pub endpoint: String,
    pub model: String,
    pub confidence_threshold: f64,
    pub max_attempts: u8,
    pub parallel_rederivation: bool,
}

impl AgentConfig {
    pub fn is_configured(&self) -> bool {
        !self.endpoint.trim().is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct SandboxConfig {
    pub auto_spin: bool,
    pub preview_command: String,
    pub cleanup_after_days: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct HooksConfig {
    pub pre_patch: String,
    pub post_patch: String,
    pub pre_update: String,
    pub post_update: String,
    pub on_conflict: String,
}

pub fn render_config(upstream: &str) -> Result<String, RedtapeError> {
    let mut rendered = serde_json::to_string_pretty(&default_config(upstream))
        .map_err(|err| RedtapeError::Usage(format!("failed to serialize config.json: {err}")))?;
    rendered.push('\n');
    Ok(rendered)
}

pub fn read_config(path: &Path) -> Result<Config, RedtapeError> {
    let content = fs::read_to_string(path)?;
    serde_json::from_str(&content)
        .map_err(|err| RedtapeError::Validation(format!("invalid config.json: {err}")))
}

pub fn default_config(upstream: &str) -> Config {
    Config {
        protocol: PROTOCOL_VERSION.to_string(),
        upstream: upstream.to_string(),
        agent: AgentConfig {
            provider: String::new(),
            endpoint: String::new(),
            model: String::new(),
            confidence_threshold: 0.80,
            max_attempts: 3,
            parallel_rederivation: true,
        },
        sandbox: SandboxConfig {
            auto_spin: true,
            preview_command: String::new(),
            cleanup_after_days: 7,
        },
        hooks: HooksConfig {
            pre_patch: String::new(),
            post_patch: String::new(),
            pre_update: String::new(),
            post_update: String::new(),
            on_conflict: String::new(),
        },
        commit_both_layers: true,
        gitignore_sandbox: true,
    }
}

pub fn build_patch_header(upstream: &str, base_ref: &str) -> String {
    format!(
        "# PatchMD\n> project: {}\n> upstream: {}\n> base-ref: {}\n> protocol: {}\n\n---\n",
        derive_project_name(upstream),
        upstream,
        base_ref,
        PROTOCOL_VERSION
    )
}

pub fn empty_triage_summary() -> String {
    "{}\n".to_string()
}

pub fn empty_migration_log() -> String {
    String::new()
}

fn derive_project_name(upstream: &str) -> String {
    let trimmed = upstream.trim().trim_end_matches('/');
    let slash_split = trimmed.rsplit('/').next().unwrap_or(trimmed);
    let colon_split = slash_split.rsplit(':').next().unwrap_or(slash_split);
    let backslash_split = colon_split.rsplit('\\').next().unwrap_or(colon_split);
    let candidate = backslash_split.trim_end_matches(".git").trim();

    if candidate.is_empty() {
        "unknown".to_string()
    } else {
        candidate.to_string()
    }
}
