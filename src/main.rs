use std::process::ExitCode;

use opencode_tui::{app, runtime};

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("opencode_tui=info")),
        )
        .with_writer(std::io::stderr)
        .init();

    let args = match runtime::Args::from_env() {
        Ok(args) => args,
        Err(err) => {
            eprintln!("opencode-tui: {err}");
            return ExitCode::FAILURE;
        }
    };

    match app::run(args).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("opencode-tui: {err:#}");
            ExitCode::FAILURE
        }
    }
}
