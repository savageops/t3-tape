use std::fmt;
use std::path::Path;
use std::str::FromStr;

use crate::exit::RedtapeError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PatchId(u32);

impl PatchId {
    pub fn new(value: u32) -> Result<Self, RedtapeError> {
        if value == 0 {
            return Err(RedtapeError::Validation(
                "patch ids must be positive integers".to_string(),
            ));
        }

        Ok(Self(value))
    }

    pub fn value(self) -> u32 {
        self.0
    }

    pub fn next_after(self) -> Self {
        Self(self.0 + 1)
    }

    pub fn from_diff_path(path: &Path) -> Option<Self> {
        let stem = path.file_stem()?.to_str()?;
        stem.parse().ok()
    }
}

impl fmt::Display for PatchId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PATCH-{:03}", self.0)
    }
}

impl FromStr for PatchId {
    type Err = RedtapeError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let trimmed = value.trim();
        let numeric = trimmed.strip_prefix("PATCH-").ok_or_else(|| {
            RedtapeError::Validation(format!("invalid patch id format: {trimmed}"))
        })?;

        let parsed: u32 = numeric
            .parse()
            .map_err(|_| RedtapeError::Validation(format!("invalid patch id number: {trimmed}")))?;

        Self::new(parsed)
    }
}
