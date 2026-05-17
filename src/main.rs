//! Thin binary boundary: parse args, run the async app, map the result
//! to a process exit code. All logic lives in the library (the
//! lib+thin-bin split; COMPILER-TRUTH Law 6).

use std::process::ExitCode;

use anyhow::Context;

#[tokio::main]
async fn main() -> ExitCode {
    let cli = ctx::cli::parse();
    match ctx::run_app(cli).await.context("ctx run") {
        Ok(code) => ExitCode::from(u8::try_from(code).unwrap_or(1)),
        Err(e) => {
            eprintln!("ctx: {e:#}");
            ExitCode::FAILURE
        }
    }
}
