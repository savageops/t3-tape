use crate::cli::RederiveArgs;
use crate::exit::RedtapeError;
use crate::update;
use crate::update::triage;

use super::GlobalOptions;

pub fn run(global: &GlobalOptions, args: &RederiveArgs) -> Result<(), RedtapeError> {
    let summary = update::rederive_patch(global, args)?;
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
