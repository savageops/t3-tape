use crate::cli::PatchListArgs;
use crate::exit::RedtapeError;
use crate::patch::{self, PatchId};

use super::GlobalOptions;

pub fn run(global: &GlobalOptions, _args: &PatchListArgs) -> Result<(), RedtapeError> {
    let paths = patch::resolve_paths(global)?;
    let (_, document) = patch::read_document(&paths)?;

    if document.entries.is_empty() {
        println!("no patches recorded");
        return Ok(());
    }

    for entry in document.entries {
        let last_checked = patch::read_meta_for_id(&paths, PatchId::new(entry.id.value())?)?
            .map(|meta| meta.last_checked)
            .unwrap_or_else(|| "-".to_string());
        println!(
            "{}\t{}\t{}\t{}",
            entry.id, entry.title, entry.status, last_checked
        );
    }

    Ok(())
}
