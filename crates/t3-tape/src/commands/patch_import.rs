use std::fs;

use crate::cli::PatchImportArgs;
use crate::exit::RedtapeError;
use crate::patch::{self, NewPatchSpec, UnifiedDiff};
use crate::store::lock::StateLock;

use super::GlobalOptions;

pub fn run(global: &GlobalOptions, args: &PatchImportArgs) -> Result<(), RedtapeError> {
    let paths = patch::resolve_paths(global)?;
    let _lock = StateLock::acquire(&paths.lock_path)?;
    let raw_diff = fs::read_to_string(&args.diff)?;
    let specs = build_specs(args, &raw_diff)?;
    let context = patch::build_write_context(&paths.repo_root)?;

    let created = patch::create_patch_records(&paths, &context, &specs)?;

    for patch in created {
        println!("created patch {}", patch.id);
    }

    Ok(())
}

fn build_specs(args: &PatchImportArgs, raw_diff: &str) -> Result<Vec<NewPatchSpec>, RedtapeError> {
    let explicit_intent = match (&args.intent, &args.intent_file) {
        (Some(_), Some(_)) => {
            return Err(RedtapeError::Usage(
                "use exactly one of --intent or --intent-file for patch import".to_string(),
            ))
        }
        (Some(intent), None) => Some(intent.trim().to_string()),
        (None, Some(path)) => Some(patch::read_intent_from_file(path)?),
        (None, None) => None,
    };

    if let (Some(title), Some(intent)) = (&args.title, explicit_intent.as_ref()) {
        return Ok(vec![NewPatchSpec {
            title: title.clone(),
            intent: intent.clone(),
            assertions: Vec::new(),
            surface: args.surface.clone(),
            raw_diff: raw_diff.to_string(),
        }]);
    }

    if args.title.is_some() || explicit_intent.is_some() {
        return Err(RedtapeError::Usage(
            "single-record import requires both --title and intent input".to_string(),
        ));
    }

    let parsed = UnifiedDiff::parse(raw_diff)?;
    println!("proposed {} patch record(s):", parsed.files.len());
    for file in &parsed.files {
        println!("- {}", file.path);
    }

    if !patch::confirm("Continue import? [y/N]: ")? {
        return Err(RedtapeError::Usage("patch import aborted".to_string()));
    }

    let mut specs = Vec::new();
    for file in parsed.files {
        let title = patch::prompt_line(&format!("Title for {}: ", file.path))?;
        let intent = patch::prompt_line(&format!("Intent for {}: ", file.path))?;
        specs.push(NewPatchSpec {
            title,
            intent,
            assertions: Vec::new(),
            surface: Some(file.path.clone()),
            raw_diff: file.raw,
        });
    }

    Ok(specs)
}
