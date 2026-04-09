use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::exit::RedtapeError;
use crate::store::atomic;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct PatchMeta {
    pub id: String,
    pub title: String,
    pub status: String,
    pub base_ref: String,
    pub current_ref: String,
    pub diff_file: String,
    pub apply_confidence: f64,
    pub last_applied: String,
    pub last_checked: String,
    pub agent_attempts: u32,
    pub surface_hash: String,
    pub behavior_assertions: Vec<String>,
}

pub fn read(path: &Path) -> Result<Option<PatchMeta>, RedtapeError> {
    if !path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(path)?;
    let meta = serde_json::from_str(&content)
        .map_err(|err| RedtapeError::Validation(format!("invalid meta file: {err}")))?;
    Ok(Some(meta))
}

pub fn write_new(path: &Path, meta: &PatchMeta) -> Result<(), RedtapeError> {
    let mut rendered = serde_json::to_string_pretty(meta)
        .map_err(|err| RedtapeError::Validation(format!("failed to serialize meta: {err}")))?;
    rendered.push('\n');
    atomic::write_new_file_atomic(path, rendered.as_bytes())
}
