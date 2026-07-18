//! Capability-oriented file input and narrow standard-input support.

use std::io::Read as _;

use camino::Utf8Path;
use thiserror::Error;

/// Maximum accepted source size for both path and standard-input reads.
pub const MAX_SOURCE_BYTES: usize = 16 * 1024 * 1024;

const BOUNDED_READ_BYTES: u64 = 16 * 1024 * 1024 + 1;

/// Capability for opening one logical source path as a byte stream.
pub trait SourceReader {
    /// Open `path` for reading without resolving ambient authority.
    ///
    /// # Errors
    ///
    /// Returns an input/output error when the capability cannot open `path`.
    fn open(&self, path: &Utf8Path) -> std::io::Result<Box<dyn std::io::Read>>;
}

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
    /// The source exceeded [`MAX_SOURCE_BYTES`].
    #[error("source {path} exceeds the {limit}-byte limit")]
    TooLarge {
        /// Logical input path.
        path: String,
        /// Maximum accepted byte length.
        limit: usize,
    },
}

impl SourceReadError {
    /// Stable operation identifier for process diagnostics.
    #[must_use]
    pub const fn operation(&self) -> &'static str {
        match self {
            Self::Open { .. } => "source-open",
            Self::Read { .. } => "source-read",
            Self::TooLarge { .. } => "source-too-large",
        }
    }
}

/// Read exact bytes from a UTF-8 path using an injected reader capability.
///
/// # Errors
///
/// Returns [`SourceReadError`] when the source cannot be opened, read, or
/// exceeds [`MAX_SOURCE_BYTES`].
pub fn read_path(
    reader: &(impl SourceReader + ?Sized),
    path: &Utf8Path,
) -> Result<Vec<u8>, SourceReadError> {
    let display_path = path.as_str().to_owned();
    let mut file = reader.open(path).map_err(|source| SourceReadError::Open {
        path: display_path.clone(),
        source,
    })?;
    read_bounded(&mut file, display_path)
}

/// Read all bytes from an injected standard-input reader.
///
/// # Errors
///
/// Returns [`SourceReadError`] when the stream fails or exceeds
/// [`MAX_SOURCE_BYTES`].
pub fn read_stdin(reader: &mut (impl std::io::Read + ?Sized)) -> Result<Vec<u8>, SourceReadError> {
    read_bounded(reader, "standard input".to_owned())
}

fn read_bounded(
    reader: &mut (impl std::io::Read + ?Sized),
    display_path: String,
) -> Result<Vec<u8>, SourceReadError> {
    let mut bytes = Vec::new();
    reader
        .take(BOUNDED_READ_BYTES)
        .read_to_end(&mut bytes)
        .map_err(|source| SourceReadError::Read {
            path: display_path.clone(),
            source,
        })?;
    if bytes.len() > MAX_SOURCE_BYTES {
        Err(SourceReadError::TooLarge {
            path: display_path,
            limit: MAX_SOURCE_BYTES,
        })
    } else {
        Ok(bytes)
    }
}
