use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(name = "t3-tape", version, about = "T3 Tape - PatchMD toolchain")]
pub struct Cli {
    /// Repo root directory (defaults to git root when available, else cwd)
    #[arg(long, global = true)]
    pub repo_root: Option<PathBuf>,

    /// State directory path (defaults to .t3/ under repo root)
    #[arg(long, global = true)]
    pub state_dir: Option<PathBuf>,

    /// Emit JSON output for supported commands (triage, validate)
    #[arg(long, global = true)]
    pub json: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Init(InitArgs),
    Patch(PatchArgs),
    Hooks(HooksArgs),
    Validate(ValidateArgs),
    Update(UpdateArgs),
    Triage(TriageArgs),
    Rederive(RederiveArgs),
    Export(ExportArgs),
}

#[derive(Debug, Parser)]
pub struct InitArgs {
    #[arg(long)]
    pub upstream: String,

    #[arg(long)]
    pub base_ref: String,
}

#[derive(Debug, Parser)]
pub struct PatchArgs {
    #[command(subcommand)]
    pub command: PatchCommand,
}

#[derive(Debug, Subcommand)]
pub enum PatchCommand {
    Add(PatchAddArgs),
    List(PatchListArgs),
    Show(PatchShowArgs),
    Import(PatchImportArgs),
}

#[derive(Debug, Parser)]
pub struct PatchAddArgs {
    #[arg(long)]
    pub title: String,

    #[arg(long)]
    pub intent: Option<String>,

    #[arg(long)]
    pub intent_file: Option<PathBuf>,

    #[arg(long)]
    pub staged: bool,

    #[arg(long)]
    pub surface: Option<String>,

    #[arg(long = "assert")]
    pub assertions: Vec<String>,
}

#[derive(Debug, Parser)]
pub struct PatchListArgs {}

#[derive(Debug, Parser)]
pub struct PatchShowArgs {
    pub id: String,

    #[arg(long)]
    pub diff: bool,
}

#[derive(Debug, Parser)]
pub struct PatchImportArgs {
    #[arg(long)]
    pub diff: PathBuf,

    #[arg(long)]
    pub title: Option<String>,

    #[arg(long)]
    pub intent: Option<String>,

    #[arg(long)]
    pub intent_file: Option<PathBuf>,

    #[arg(long)]
    pub surface: Option<String>,
}

#[derive(Debug, Parser)]
pub struct HooksArgs {
    #[command(subcommand)]
    pub command: HooksCommand,
}

#[derive(Debug, Subcommand)]
pub enum HooksCommand {
    Print(HooksPrintArgs),
    Install(HooksInstallArgs),
}

#[derive(Debug, Parser)]
pub struct HooksPrintArgs {
    #[arg(value_enum)]
    pub kind: HooksPrintKind,
}

#[derive(Debug, Clone, ValueEnum)]
pub enum HooksPrintKind {
    PreCommit,
    Gitignore,
    Gitattributes,
}

#[derive(Debug, Parser)]
pub struct HooksInstallArgs {
    #[arg(value_enum)]
    pub kind: HooksInstallKind,

    #[arg(long)]
    pub force: bool,
}

#[derive(Debug, Clone, ValueEnum)]
pub enum HooksInstallKind {
    PreCommit,
}

#[derive(Debug, Parser)]
pub struct ValidateArgs {
    #[arg(long)]
    pub staged: bool,
}

#[derive(Debug, Parser)]
pub struct UpdateArgs {
    #[arg(long)]
    pub r#ref: String,

    #[arg(long)]
    pub ci: bool,

    #[arg(long)]
    pub confidence_threshold: Option<f64>,
}

#[derive(Debug, Parser)]
pub struct TriageArgs {
    #[command(subcommand)]
    pub command: Option<TriageCommand>,
}

#[derive(Debug, Subcommand)]
pub enum TriageCommand {
    Approve(TriageApproveArgs),
}

#[derive(Debug, Parser)]
pub struct TriageApproveArgs {
    pub id: String,
}

#[derive(Debug, Parser)]
pub struct RederiveArgs {
    pub id: String,
}

#[derive(Debug, Parser)]
pub struct ExportArgs {
    #[arg(long)]
    pub format: String,

    #[arg(long)]
    pub output: PathBuf,
}
