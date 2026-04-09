use crate::cli::PatchAddArgs;
use crate::exit::RedtapeError;
use crate::patch::{self, NewPatchSpec};
use crate::store::lock::StateLock;

use super::GlobalOptions;

pub fn run(global: &GlobalOptions, args: &PatchAddArgs) -> Result<(), RedtapeError> {
    let paths = patch::resolve_paths(global)?;
    let _lock = StateLock::acquire(&paths.lock_path)?;
    let intent = resolve_intent(args)?;
    let raw_diff = patch::capture_git_diff(&paths.repo_root, args.staged)?;
    let context = patch::build_write_context(&paths.repo_root)?;

    let created = patch::create_patch_records(
        &paths,
        &context,
        &[NewPatchSpec {
            title: args.title.clone(),
            intent,
            assertions: args.assertions.clone(),
            surface: args.surface.clone(),
            raw_diff,
        }],
    )?;

    let created = &created[0];
    println!("created patch {}", created.id);
    println!("diff: {}", created.diff_path.display());
    Ok(())
}

fn resolve_intent(args: &PatchAddArgs) -> Result<String, RedtapeError> {
    match (&args.intent, &args.intent_file) {
        (Some(_), Some(_)) => Err(RedtapeError::Usage(
            "use exactly one of --intent or --intent-file".to_string(),
        )),
        (Some(intent), None) => Ok(intent.trim().to_string()),
        (None, Some(path)) => patch::read_intent_from_file(path),
        (None, None) => {
            if !patch::stdin_is_terminal() {
                return Err(RedtapeError::Usage(
                    "intent required: use --intent or --intent-file when stdin is not a TTY"
                        .to_string(),
                ));
            }
            patch::prompt_line("Intent: ")
        }
    }
}
