use reqwest::blocking::Client;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};

use crate::exit::RedtapeError;

pub fn post(endpoint: &str, body: &str) -> Result<String, RedtapeError> {
    let client = Client::builder()
        .build()
        .map_err(|err| RedtapeError::Agent(format!("failed to build HTTP agent client: {err}")))?;

    let mut request = client
        .post(endpoint)
        .header(CONTENT_TYPE, "application/json")
        .body(body.to_string());

    if let Ok(token) = std::env::var("T3_TAPE_AGENT_AUTH_TOKEN") {
        if !token.trim().is_empty() {
            request = request.header(AUTHORIZATION, format!("Bearer {token}"));
        }
    }

    let response = request
        .send()
        .map_err(|err| RedtapeError::Agent(format!("agent HTTP request failed: {err}")))?;

    let status = response.status();
    let body = response
        .text()
        .map_err(|err| RedtapeError::Agent(format!("failed to read agent HTTP response: {err}")))?;

    if !status.is_success() {
        return Err(RedtapeError::Agent(format!(
            "agent HTTP request failed with {}: {}",
            status,
            body.trim()
        )));
    }

    Ok(body)
}
