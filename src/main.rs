//! `makeutil` application entry point.

use std::process::ExitCode;

use makeutil::adapters::cli::run_from;

/// Compose process streams and return the classified exit code.
fn main() -> ExitCode {
    let outcome = run_from(
        std::env::args_os(),
        &mut std::io::stdin().lock(),
        &mut std::io::stdout().lock(),
        &mut std::io::stderr().lock(),
    );
    ExitCode::from(outcome.exit_code)
}
