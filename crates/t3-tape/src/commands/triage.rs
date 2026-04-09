use crate::cli::TriageArgs;
use crate::exit::RedtapeError;
use crate::update;
use crate::update::triage;

use super::GlobalOptions;

pub fn run(global: &GlobalOptions, args: &TriageArgs) -> Result<(), RedtapeError> {
    match &args.command {
        Some(crate::cli::TriageCommand::Approve(approve)) => {
            super::triage_approve::run(global, approve)
        }
        None => {
            let summary = update::read_latest_triage(global)?;
            let rendered = if global.json {
                let mut json = serde_json::to_string_pretty(&summary).map_err(|err| {
                    RedtapeError::Validation(format!("failed to serialize triage summary: {err}"))
                })?;
                json.push('\n');
                json
            } else {
                triage::render_human(&summary)
            };
            print!("{rendered}");
            Ok(())
        }
    }
}
