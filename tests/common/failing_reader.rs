//! Shared reader failure used by source-boundary integration tests.

use std::io::Read;

/// Return a reader that fails while consuming an opened source.
pub fn failing_reader() -> Box<dyn Read> { Box::new(FailingReader) }

struct FailingReader;

impl Read for FailingReader {
    fn read(&mut self, _buffer: &mut [u8]) -> std::io::Result<usize> {
        Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "broken source stream",
        ))
    }
}
