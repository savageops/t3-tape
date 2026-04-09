pub mod agent;
pub mod cli;
pub mod commands;
pub mod exit;
pub mod patch;
pub mod store;
pub mod update;
pub mod validate;

use commands::GlobalOptions;
use exit::RedtapeError;

use cli::{Cli, Command, PatchCommand};

pub fn run(cli: Cli) -> Result<(), RedtapeError> {
    let Cli {
        repo_root,
        state_dir,
        json,
        command,
    } = cli;

    let global = GlobalOptions {
        repo_root,
        state_dir,
        json,
        cwd: None,
    };

    match command {
        Command::Init(args) => {
            let report = store::initialize(store::InitRequest {
                repo_root: global.repo_root.clone(),
                state_dir: global.state_dir.clone(),
                upstream: args.upstream,
                base_ref: args.base_ref,
                cwd: global.cwd.clone(),
            })?;

            println!(
                "initialized PatchMD store at {}",
                report.paths.state_dir.display()
            );
            println!("repo root: {}", report.paths.repo_root.display());
            println!("created directories: {}", report.created_directories.len());
            println!("created files: {}", report.created_files.len());
            Ok(())
        }
        Command::Patch(args) => match args.command {
            PatchCommand::Add(args) => commands::patch_add::run(&global, &args),
            PatchCommand::List(args) => commands::patch_list::run(&global, &args),
            PatchCommand::Show(args) => commands::patch_show::run(&global, &args),
            PatchCommand::Import(args) => commands::patch_import::run(&global, &args),
        },
        Command::Hooks(args) => commands::hooks::run(&global, &args),
        Command::Validate(args) => commands::validate::run(&global, &args),
        Command::Update(args) => commands::update::run(&global, &args),
        Command::Triage(args) => commands::triage::run(&global, &args),
        Command::Rederive(args) => commands::rederive::run(&global, &args),
        Command::Export(args) => commands::export::run(&global, &args),
    }
}
