//! Edge adapters for GNU Make parsing, source input, JSON, and the CLI.

pub mod cli;
mod makefile;
pub mod source;

pub use makefile::MakefileLosslessParser;
