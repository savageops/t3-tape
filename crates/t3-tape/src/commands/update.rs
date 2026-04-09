use crate::cli::UpdateArgs;
use crate::exit::RedtapeError;
use crate::update;
use crate::update::triage;

use super::GlobalOptions;

pub fn run(global: &GlobalOptions, args: &UpdateArgs) -> Result<(), RedtapeError> {
    let outcome = update::run_update(global, args)?;
    let rendered = if global.json {
        let mut json = serde_json::to_string_pretty(&outcome.summary).map_err(|err| {
            RedtapeError::Validation(format!("failed to serialize triage summary: {err}"))
        })?;
        json.push('\n');
        json
    } else {
        triage::render_human(&outcome.summary)
    };

    print!("{rendered}");
    if outcome.exit_code == 0 {
        Ok(())
    } else {
        Err(RedtapeError::Reported(outcome.exit_code))
    }
}
