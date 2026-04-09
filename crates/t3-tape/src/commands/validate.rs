use crate::cli::ValidateArgs;
use crate::exit::RedtapeError;
use crate::patch;
use crate::validate::{self, full, staged};

use super::GlobalOptions;

pub fn run(global: &GlobalOptions, args: &ValidateArgs) -> Result<(), RedtapeError> {
    let paths = patch::resolve_paths(global)?;
    let mut report = full::validate(&paths)?;
    if args.staged {
        staged::validate(&paths, &mut report)?;
        report.refresh_status();
    }

    let rendered = if global.json {
        validate::render_json(&report)?
    } else {
        validate::render_human(&report)
    };

    print!("{rendered}");

    if report.is_ok() {
        Ok(())
    } else {
        Err(RedtapeError::Reported(2))
    }
}
