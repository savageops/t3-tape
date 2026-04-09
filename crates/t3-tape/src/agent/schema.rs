use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct ScopeUpdate {
    pub files: Vec<String>,
    pub components: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct ConflictResolutionRequest {
    pub mode: String,
    pub patch_id: String,
    pub intent: String,
    pub behavior_assertions: Vec<String>,
    pub original_diff: String,
    pub upstream_diff: String,
    pub new_source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct ConflictResolutionResponse {
    pub resolved_diff: String,
    pub confidence: f64,
    pub notes: String,
    pub unresolved: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct RederivationRequest {
    pub mode: String,
    pub patch_id: String,
    pub intent: String,
    pub behavior_assertions: Vec<String>,
    pub new_source: String,
    pub surface_hint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct RederivationResponse {
    pub derived_diff: String,
    pub confidence: f64,
    pub scope_update: ScopeUpdate,
    pub notes: String,
    pub unresolved: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct IntentAssistRequest {
    pub mode: String,
    pub diff: String,
    pub context: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct IntentAssistResponse {
    pub suggested_title: String,
    pub suggested_intent: String,
    pub suggested_assertions: Vec<String>,
    pub suggested_surface: String,
    pub suggested_scope: ScopeUpdate,
}
