pub mod exec;
pub mod http;
pub mod none;
pub mod schema;

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::exit::RedtapeError;
use crate::store::schema::AgentConfig;

pub const MAX_SOURCE_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderKind {
    None,
    Http,
    Exec,
}

pub fn provider_kind(config: &AgentConfig) -> ProviderKind {
    if !config.is_configured() {
        return ProviderKind::None;
    }

    match config.provider.trim().to_ascii_lowercase().as_str() {
        "http" => ProviderKind::Http,
        "exec" => ProviderKind::Exec,
        _ if config.endpoint.trim_start().starts_with("http://")
            || config.endpoint.trim_start().starts_with("https://") =>
        {
            ProviderKind::Http
        }
        _ => ProviderKind::Exec,
    }
}

pub fn send_request<TReq, TResp>(
    config: &AgentConfig,
    request: &TReq,
) -> Result<TResp, RedtapeError>
where
    TReq: Serialize,
    TResp: DeserializeOwned,
{
    let body = serde_json::to_string(request)
        .map_err(|err| RedtapeError::Agent(format!("failed to serialize agent request: {err}")))?;

    let raw = match provider_kind(config) {
        ProviderKind::None => return Err(none::blocked()),
        ProviderKind::Http => http::post(&config.endpoint, &body)?,
        ProviderKind::Exec => exec::post(&config.endpoint, &body)?,
    };

    serde_json::from_str(&raw)
        .map_err(|err| RedtapeError::Agent(format!("failed to parse agent response JSON: {err}")))
}

pub fn truncate_source(input: &str) -> (String, bool) {
    if input.len() <= MAX_SOURCE_BYTES {
        return (input.to_string(), false);
    }

    let mut end = MAX_SOURCE_BYTES;
    while !input.is_char_boundary(end) {
        end -= 1;
    }

    let mut truncated = input[..end].to_string();
    truncated.push_str("\n[truncated by t3-tape]\n");
    (truncated, true)
}
