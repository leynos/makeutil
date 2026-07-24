//! Convert zero-based byte spans into stable one-based display locations.

use serde::Serialize;
use thiserror::Error;

/// A validated zero-based, end-exclusive byte span.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceSpan {
    /// First byte included in the span.
    pub start: usize,
    /// First byte excluded from the span.
    pub end: usize,
}

impl SourceSpan {
    /// Construct a span for offsets in a source of `source_length` bytes.
    ///
    /// # Errors
    ///
    /// Returns [`LocationError`] when the offsets are reversed or out of bounds.
    pub const fn new(
        start: usize,
        end: usize,
        source_length: usize,
    ) -> Result<Self, LocationError> {
        if start > end || end > source_length {
            return Err(LocationError::InvalidSpan {
                start,
                end,
                source_length,
            });
        }
        Ok(Self { start, end })
    }
}

/// Complete machine and display location for a source span.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SourceLocation {
    /// Zero-based first byte.
    pub start_byte: usize,
    /// Zero-based exclusive end byte.
    pub end_byte: usize,
    /// One-based first line.
    pub start_line: usize,
    /// One-based byte column on the first line.
    pub start_column: usize,
    /// One-based line at the exclusive end.
    pub end_line: usize,
    /// One-based byte column at the exclusive end.
    pub end_column: usize,
}

/// Failure to map an invalid byte span.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum LocationError {
    /// Span ordering or bounds do not fit the source.
    #[error("invalid source span {start}..{end} for {source_length} bytes")]
    InvalidSpan {
        /// Requested start offset.
        start: usize,
        /// Requested exclusive end offset.
        end: usize,
        /// Available source length.
        source_length: usize,
    },
    /// An offset splits a UTF-8 code point.
    #[error("source offset {offset} is not a UTF-8 boundary")]
    NonUtf8Boundary {
        /// Invalid byte offset.
        offset: usize,
    },
}

/// Reusable index from byte offsets to line and byte-column positions.
#[derive(Debug, Clone)]
pub struct LocationIndex<'source> {
    source: &'source str,
    line_starts: Vec<usize>,
}

impl<'source> LocationIndex<'source> {
    /// Index a UTF-8 source.
    #[must_use]
    pub fn new(source: &'source str) -> Self {
        let line_starts = std::iter::once(0)
            .chain(
                source
                    .bytes()
                    .enumerate()
                    .filter_map(|(offset, byte)| (byte == b'\n').then_some(offset + 1)),
            )
            .collect();
        Self {
            source,
            line_starts,
        }
    }

    /// Map a validated byte span to its stable location.
    ///
    /// # Errors
    ///
    /// Returns [`LocationError`] for invalid bounds or split UTF-8 code points.
    pub fn locate(&self, span: SourceSpan) -> Result<SourceLocation, LocationError> {
        let validated = SourceSpan::new(span.start, span.end, self.source.len())?;
        self.require_boundary(validated.start)?;
        self.require_boundary(validated.end)?;
        let (start_line, start_column) = self.position(validated.start);
        let (end_line, end_column) = self.position(validated.end);
        Ok(SourceLocation {
            start_byte: validated.start,
            end_byte: validated.end,
            start_line,
            start_column,
            end_line,
            end_column,
        })
    }

    const fn require_boundary(&self, offset: usize) -> Result<(), LocationError> {
        if self.source.is_char_boundary(offset) {
            Ok(())
        } else {
            Err(LocationError::NonUtf8Boundary { offset })
        }
    }

    fn position(&self, offset: usize) -> (usize, usize) {
        let line_index = self.line_starts.partition_point(|start| *start <= offset) - 1;
        let line_start = self
            .line_starts
            .get(line_index)
            .copied()
            .unwrap_or_default();
        (line_index + 1, offset - line_start + 1)
    }
}

#[cfg(test)]
mod tests {
    //! Regression tests for rejected source-location boundaries.

    use pretty_assertions::assert_eq;
    use rstest::rstest;

    use super::{LocationError, LocationIndex, SourceSpan};

    #[rstest]
    #[case::reversed(3, 2, 3)]
    #[case::end_out_of_bounds(0, 4, 3)]
    fn invalid_spans_are_rejected(
        #[case] start: usize,
        #[case] end: usize,
        #[case] source_length: usize,
    ) {
        assert_eq!(
            SourceSpan::new(start, end, source_length),
            Err(LocationError::InvalidSpan {
                start,
                end,
                source_length,
            })
        );
    }

    #[rstest]
    #[case::start_boundary(SourceSpan { start: 1, end: 2 })]
    #[case::end_boundary(SourceSpan { start: 0, end: 1 })]
    fn split_utf8_boundaries_are_rejected(#[case] span: SourceSpan) {
        assert_eq!(
            LocationIndex::new("é").locate(span),
            Err(LocationError::NonUtf8Boundary { offset: 1 })
        );
    }
}
