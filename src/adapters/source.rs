//! Capability-oriented file input and narrow standard-input support.

use std::io::Read as _;

use camino::Utf8Path;
use cap_std::{ambient_authority, fs_utf8::File};
use thiserror::Error;

/// Source input failure classified for stable CLI diagnostics.
#[derive(Debug, Error)]
pub enum SourceReadError {
    /// The path could not be opened.
    #[error("could not open {path}: {source}")]
    Open {
        /// Logical input path.
        path: String,
        /// Operating-system error.
        source: std::io::Error,
    },
    /// An opened source could not be read completely.
    #[error("could not read {path}: {source}")]
    Read {
        /// Logical input path.
        path: String,
        /// Input/output error.
        source: std::io::Error,
    },
}

impl SourceReadError {
    /// Stable operation identifier for process diagnostics.
    #[must_use]
    pub const fn operation(&self) -> &'static str {
        match self {
            Self::Open { .. } => "source-open",
            Self::Read { .. } => "source-read",
        }
    }
}

/// Read exact bytes from a UTF-8 path using an explicit ambient authority.
///
/// # Errors
///
/// Returns [`SourceReadError`] when the source cannot be opened or read.
pub fn read_path(path: &Utf8Path) -> Result<Vec<u8>, SourceReadError> {
    let display_path = path.as_str().to_owned();
    let mut file =
        File::open_ambient(path, ambient_authority()).map_err(|source| SourceReadError::Open {
            path: display_path.clone(),
            source,
        })?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|source| SourceReadError::Read {
            path: display_path,
            source,
        })?;
    Ok(bytes)
}

/// Read all bytes from an injected standard-input reader.
///
/// # Errors
///
/// Returns [`SourceReadError::Read`] when the stream fails.
pub fn read_stdin(reader: &mut (impl std::io::Read + ?Sized)) -> Result<Vec<u8>, SourceReadError> {
    let mut bytes = Vec::new();
    reader
        .read_to_end(&mut bytes)
        .map_err(|source| SourceReadError::Read {
            path: "standard input".to_owned(),
            source,
        })?;
    Ok(bytes)
}
