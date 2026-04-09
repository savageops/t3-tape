use std::process::ExitCode;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum RedtapeError {
    #[error("usage error: {0}")]
    Usage(String),

    #[error("validation failed: {0}")]
    Validation(String),

    #[error("update/triage blocked: {0}")]
    Blocked(String),

    #[error("git error: {0}")]
    Git(String),

    #[error("agent error: {0}")]
    Agent(String),

    #[error("command already reported output")]
    Reported(u8),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl RedtapeError {
    pub fn exit_code(&self) -> u8 {
        match self {
            Self::Usage(_) => 1,
            Self::Validation(_) => 2,
            Self::Blocked(_) => 3,
            Self::Git(_) => 4,
            Self::Agent(_) => 5,
            Self::Reported(code) => *code,
            Self::Io(_) => 1,
        }
    }
}

pub fn run(cli: crate::cli::Cli) -> ExitCode {
    match crate::run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(RedtapeError::Reported(code)) => ExitCode::from(code),
        Err(err) => {
            eprintln!("{err}");
            ExitCode::from(err.exit_code())
        }
    }
}
