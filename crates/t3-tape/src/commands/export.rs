use crate::cli::ExportArgs;
use crate::exit::RedtapeError;
use crate::patch;
use crate::store::atomic;

use super::GlobalOptions;

pub fn run(global: &GlobalOptions, args: &ExportArgs) -> Result<(), RedtapeError> {
    if args.format.trim() != "markdown" {
        return Err(RedtapeError::Usage(
            "unsupported export format; use --format markdown".to_string(),
        ));
    }

    let paths = patch::resolve_paths(global)?;
    let (_, document) = patch::read_document(&paths)?;
    let rendered = render_markdown_export(&document);
    atomic::write_file_atomic(&args.output, rendered.as_bytes())?;
    println!("wrote export to {}", args.output.display());
    Ok(())
}

fn render_markdown_export(document: &patch::PatchDocument) -> String {
    let mut rendered = String::from("# PatchMD Export\n\n");
    if document.entries.is_empty() {
        rendered.push_str("_No patches recorded._\n");
        return rendered;
    }

    for entry in &document.entries {
        rendered.push_str(&format!("## [{}] {}\n\n", entry.id, entry.title));
        rendered.push_str(&format!("Status: {}\n\n", entry.status));
        rendered.push_str("Intent\n\n");
        rendered.push_str(entry.intent.trim());
        rendered.push_str("\n\nBehavior Contract\n\n");
        if entry.behavior_assertions.is_empty() {
            rendered.push_str("- none recorded\n\n");
        } else {
            for assertion in &entry.behavior_assertions {
                rendered.push_str(&format!("- {assertion}\n"));
            }
            rendered.push('\n');
        }
    }

    rendered
}
