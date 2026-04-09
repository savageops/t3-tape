use clap::Parser;

fn main() -> std::process::ExitCode {
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(false)
        .init();

    let cli = t3_tape::cli::Cli::parse();
    t3_tape::exit::run(cli)
}
