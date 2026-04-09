use crate::cli::TriageApproveArgs;
use crate::exit::RedtapeError;
use crate::update;

use super::GlobalOptions;

pub fn run(global: &GlobalOptions, args: &TriageApproveArgs) -> Result<(), RedtapeError> {
    let outcome = update::approve_patch(global, args)?;
    if outcome.cycle_complete {
        println!("{}\t{}\tCOMPLETE", outcome.patch_id, outcome.status);
    } else {
        println!("{}\t{}", outcome.patch_id, outcome.status);
    }
    Ok(())
}
