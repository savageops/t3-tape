use crate::cli::PatchShowArgs;
use crate::exit::RedtapeError;
use crate::patch::{self, PatchId};

use super::GlobalOptions;

pub fn run(global: &GlobalOptions, args: &PatchShowArgs) -> Result<(), RedtapeError> {
    let paths = patch::resolve_paths(global)?;
    let (_, document) = patch::read_document(&paths)?;
    let id: PatchId = args.id.parse()?;
    let entry = document
        .find(id)
        .ok_or_else(|| RedtapeError::Usage(format!("patch not found: {}", id)))?;

    print!("{}", entry.raw_block);
    if !entry.raw_block.ends_with('\n') {
        println!();
    }

    if let Some(meta) = patch::read_meta_for_id(&paths, id)? {
        println!("meta:");
        println!("  id: {}", meta.id);
        println!("  status: {}", meta.status);
        println!("  diff-file: {}", meta.diff_file);
        println!("  base-ref: {}", meta.base_ref);
        println!("  current-ref: {}", meta.current_ref);
        println!("  last-checked: {}", meta.last_checked);
    } else {
        println!("meta: missing");
    }

    if args.diff {
        println!("{}", patch::diff_path(&paths, id).display());
    }

    Ok(())
}
