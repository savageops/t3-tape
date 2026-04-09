use crate::exit::RedtapeError;

pub fn blocked() -> RedtapeError {
    RedtapeError::Blocked("agent not configured".to_string())
}
