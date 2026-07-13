//! OrthoConfig-backed command-line parsing and process-level exit policy.

use std::ffi::OsString;

use camino::Utf8Path;
use clap::{CommandFactory as _, FromArgMatches as _, Parser, Subcommand};
use ortho_config::{CliValueExtractor as _, OrthoConfig};
use serde::{Deserialize, Serialize};

use super::{
    MakefileLosslessParser,
    source::{read_path, read_stdin},
};
use crate::{domain::ParseStatus, parse_source};

/// Root command line for `makeutil`.
#[derive(Debug, Parser)]
#[command(version, about)]
pub struct Cli {
    /// Selected operation.
    #[command(subcommand)]
    pub command: Command,
}

/// Available makeutil operations.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Parse one GNU Makefile into JSON facts.
    Parse(ParseArgs),
}

/// Explicit-only arguments for the parse subcommand.
#[derive(Debug, Clone, Default, Deserialize, Serialize, Parser, OrthoConfig)]
#[command(name = "parse")]
#[ortho_config(prefix = "MAKEUTIL_PARSE_")]
pub struct ParseArgs {
    /// Logical source name required when reading `-` from standard input.
    #[arg(long, requires = "path")]
    #[ortho_config(cli_default_as_absent)]
    pub stdin_filename: Option<String>,
    /// UTF-8 source path, or `-` for standard input.
    pub path: String,
}

/// Process outcome without terminating the embedding process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProcessOutcome {
    /// Conventional process exit code.
    pub exit_code: u8,
}

struct Streams<'stream> {
    stdin: &'stream mut dyn std::io::Read,
    stdout: &'stream mut dyn std::io::Write,
    stderr: &'stream mut dyn std::io::Write,
}

/// Parse arguments, run the command, and write only contract streams.
pub fn run_from<I, T>(
    command_line: I,
    stdin: &mut impl std::io::Read,
    stdout: &mut impl std::io::Write,
    stderr: &mut impl std::io::Write,
) -> ProcessOutcome
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let mut streams = Streams {
        stdin,
        stdout,
        stderr,
    };
    let command = Cli::command();
    let matches = match command.try_get_matches_from(command_line) {
        Ok(matches) => matches,
        Err(error) => return render_clap_error(&error, &mut streams),
    };
    let cli = match Cli::from_arg_matches(&matches) {
        Ok(cli) => cli,
        Err(error) => {
            let _write_result = streams.stderr.write_all(error.to_string().as_bytes());
            return ProcessOutcome { exit_code: 2 };
        }
    };
    match cli.command {
        Command::Parse(parse_arguments) => run_parse(&parse_arguments, &matches, &mut streams),
    }
}

fn render_clap_error(error: &clap::Error, streams: &mut Streams<'_>) -> ProcessOutcome {
    let exit_code = if error.use_stderr() { 2 } else { 0 };
    let writer = if error.use_stderr() {
        &mut streams.stderr
    } else {
        &mut streams.stdout
    };
    let _write_result = writer.write_all(error.to_string().as_bytes());
    ProcessOutcome { exit_code }
}

fn run_parse(
    parsed_arguments: &ParseArgs,
    matches: &clap::ArgMatches,
    streams: &mut Streams<'_>,
) -> ProcessOutcome {
    let Some(parse_matches) = matches.subcommand_matches("parse") else {
        return fatal(
            streams.stderr,
            "cli",
            "parse subcommand matches were absent",
        );
    };
    // This OrthoConfig extraction intentionally reads only explicit ArgMatches;
    // path identity must never come from environment or configuration files.
    let explicit = match parsed_arguments.extract_user_provided(parse_matches) {
        Ok(value) => value,
        Err(error) => return fatal(streams.stderr, "cli", &error.to_string()),
    };
    let explicit_arguments: ParseArgs = match serde_json::from_value(explicit) {
        Ok(explicit_arguments) => explicit_arguments,
        Err(error) => return fatal(streams.stderr, "cli", &error.to_string()),
    };
    let (bytes, logical_path) = match read_input(explicit_arguments, streams) {
        Ok(input) => input,
        Err(outcome) => return outcome,
    };
    let report = match parse_source(&bytes, &logical_path, &MakefileLosslessParser) {
        Ok(report) => report,
        Err(crate::ParseApplicationError::InvalidUtf8(error)) => {
            return fatal(streams.stderr, "source-utf8", &error.to_string());
        }
        Err(error) => return fatal(streams.stderr, "parse-internal", &error.to_string()),
    };
    let mut document = match serde_json::to_vec(&report) {
        Ok(document) => document,
        Err(error) => return fatal(streams.stderr, "json-serialize", &error.to_string()),
    };
    document.push(b'\n');
    if let Err(error) = streams.stdout.write_all(&document) {
        return fatal(streams.stderr, "stdout-write", &error.to_string());
    }
    ProcessOutcome {
        exit_code: u8::from(report.parse.status != ParseStatus::Complete),
    }
}

fn read_input(
    arguments: ParseArgs,
    streams: &mut Streams<'_>,
) -> Result<(Vec<u8>, String), ProcessOutcome> {
    if arguments.path == "-" {
        let logical_path = arguments.stdin_filename.ok_or_else(|| {
            fatal(
                streams.stderr,
                "cli",
                "--stdin-filename is required when PATH is -",
            )
        })?;
        return read_stdin(streams.stdin)
            .map(|bytes| (bytes, logical_path))
            .map_err(|error| fatal(streams.stderr, "source-read", &error.to_string()));
    }
    if arguments.stdin_filename.is_some() {
        return Err(fatal(
            streams.stderr,
            "cli",
            "--stdin-filename is only valid when PATH is -",
        ));
    }
    let path = Utf8Path::new(&arguments.path);
    read_path(path)
        .map(|bytes| (bytes, arguments.path))
        .map_err(|error| fatal(streams.stderr, error.operation(), &error.to_string()))
}

fn fatal(stderr: &mut dyn std::io::Write, operation: &str, detail: &str) -> ProcessOutcome {
    let message = format!("makeutil: {operation}: {detail}\n");
    let _write_result = stderr.write_all(message.as_bytes());
    ProcessOutcome { exit_code: 2 }
}
