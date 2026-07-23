//! OrthoConfig-backed command-line parsing and process-level exit policy.

use std::ffi::OsString;

use camino::Utf8Path;
use cap_std::{AmbientAuthority, ambient_authority, fs_utf8::File};
use clap::{CommandFactory as _, FromArgMatches as _, Parser, Subcommand};
use ortho_config::{CliValueExtractor as _, OrthoConfig};
use serde::{Deserialize, Serialize};

use super::{
    MakefileLosslessParser,
    source::{SourceReader, read_path, read_stdin},
};
use crate::{
    domain::{ParseReport, ParseStatus},
    parse_source,
};

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

/// Process-owned input, output, diagnostic, and source-reader capabilities.
pub struct ProcessCapabilities<'stream> {
    stdin: &'stream mut dyn std::io::Read,
    stdout: &'stream mut dyn std::io::Write,
    stderr: &'stream mut dyn std::io::Write,
    source_reader: &'stream dyn SourceReader,
}

impl<'stream> ProcessCapabilities<'stream> {
    /// Bundle process capabilities for an injected command invocation.
    pub fn new(
        stdin: &'stream mut dyn std::io::Read,
        stdout: &'stream mut dyn std::io::Write,
        stderr: &'stream mut dyn std::io::Write,
        source_reader: &'stream dyn SourceReader,
    ) -> Self {
        Self {
            stdin,
            stdout,
            stderr,
            source_reader,
        }
    }
}

struct AmbientSourceReader {
    authority: AmbientAuthority,
}

impl AmbientSourceReader {
    fn new() -> Self {
        Self {
            authority: ambient_authority(),
        }
    }
}

impl SourceReader for AmbientSourceReader {
    fn open(&self, path: &Utf8Path) -> std::io::Result<Box<dyn std::io::Read>> {
        File::open_ambient(path, self.authority)
            .map(|file| Box::new(file) as Box<dyn std::io::Read>)
    }
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
    let source_reader = AmbientSourceReader::new();
    run_from_with_reader(
        command_line,
        ProcessCapabilities::new(stdin, stdout, stderr, &source_reader),
    )
}

/// Parse arguments and run the command with an injected source-reader capability.
pub fn run_from_with_reader<I, T>(
    command_line: I,
    mut capabilities: ProcessCapabilities<'_>,
) -> ProcessOutcome
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let command = Cli::command();
    let matches = match command.try_get_matches_from(command_line) {
        Ok(matches) => matches,
        Err(error) => return render_clap_error(&error, &mut capabilities),
    };
    let cli = match Cli::from_arg_matches(&matches) {
        Ok(cli) => cli,
        Err(error) => return fatal(capabilities.stderr, "cli", &error.to_string()),
    };
    match cli.command {
        Command::Parse(parse_arguments) => run_parse(&parse_arguments, &matches, &mut capabilities),
    }
}

fn render_clap_error(error: &clap::Error, streams: &mut ProcessCapabilities<'_>) -> ProcessOutcome {
    let rendered = error.to_string();
    if error.use_stderr() {
        return fatal(streams.stderr, "cli", rendered.trim_end());
    }
    match streams.stdout.write_all(rendered.as_bytes()) {
        Ok(()) => ProcessOutcome { exit_code: 0 },
        Err(write_error) => fatal(streams.stderr, "stdout-write", &write_error.to_string()),
    }
}

fn run_parse(
    parsed_arguments: &ParseArgs,
    matches: &clap::ArgMatches,
    streams: &mut ProcessCapabilities<'_>,
) -> ProcessOutcome {
    let Some(parse_matches) = matches.subcommand_matches("parse") else {
        return fatal(
            streams.stderr,
            "cli",
            "parse subcommand matches were absent",
        );
    };
    let explicit_arguments = match extract_explicit_arguments(parsed_arguments, parse_matches) {
        Ok(arguments) => arguments,
        Err(error) => return fatal(streams.stderr, "cli", &error),
    };
    let report = match produce_report(explicit_arguments, streams) {
        Ok(report) => report,
        Err(outcome) => return outcome,
    };
    emit_report(&report, streams)
}

fn extract_explicit_arguments(
    parsed_arguments: &ParseArgs,
    parse_matches: &clap::ArgMatches,
) -> Result<ParseArgs, String> {
    // This OrthoConfig extraction intentionally reads only explicit ArgMatches;
    // path identity must never come from environment or configuration files.
    let explicit = parsed_arguments
        .extract_user_provided(parse_matches)
        .map_err(|error| error.to_string())?;
    serde_json::from_value(explicit).map_err(|error| error.to_string())
}

fn produce_report(
    arguments: ParseArgs,
    streams: &mut ProcessCapabilities<'_>,
) -> Result<ParseReport, ProcessOutcome> {
    let (bytes, logical_path) = read_input(arguments, streams)?;
    match parse_source(&bytes, &logical_path, &MakefileLosslessParser) {
        Ok(report) => Ok(report),
        Err(crate::ParseApplicationError::InvalidUtf8(error)) => {
            Err(fatal(streams.stderr, "source-utf8", &error.to_string()))
        }
        Err(error) => Err(fatal(streams.stderr, "parse-internal", &error.to_string())),
    }
}

fn emit_report(report: &ParseReport, streams: &mut ProcessCapabilities<'_>) -> ProcessOutcome {
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
    streams: &mut ProcessCapabilities<'_>,
) -> Result<(Vec<u8>, String), ProcessOutcome> {
    if arguments.path == "-" {
        let logical_path = arguments.stdin_filename.ok_or_else(|| {
            fatal(
                streams.stderr,
                "cli",
                "--stdin-filename is required when PATH is -",
            )
        })?;
        return read_stdin(streams.stdin, &logical_path)
            .map(|bytes| (bytes, logical_path))
            .map_err(|error| fatal(streams.stderr, error.operation(), &error.to_string()));
    }
    if arguments.stdin_filename.is_some() {
        return Err(fatal(
            streams.stderr,
            "cli",
            "--stdin-filename is only valid when PATH is -",
        ));
    }
    let path = Utf8Path::new(&arguments.path);
    read_path(streams.source_reader, path)
        .map(|bytes| (bytes, arguments.path))
        .map_err(|error| fatal(streams.stderr, error.operation(), &error.to_string()))
}

fn fatal(stderr: &mut dyn std::io::Write, operation: &str, detail: &str) -> ProcessOutcome {
    let message = format!("makeutil: {operation}: {detail}\n");
    let _write_result = stderr.write_all(message.as_bytes());
    ProcessOutcome { exit_code: 2 }
}
