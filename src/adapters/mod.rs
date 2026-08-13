//! Edge adapters for GNU Make parsing, source input, JSON, and the CLI.

pub mod cli;
mod makefile;
mod makefile_export;
pub mod source;

pub use makefile::MakefileLosslessParser;
