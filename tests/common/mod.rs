//! Shared test-only capabilities for integration suites.

use std::io::Read;

use camino::Utf8Path;
use makeutil::adapters::source::SourceReader;

mockall::mock! {
    /// Mock source-reader capability shared by integration tests.
    pub SourceReader {}

    impl SourceReader for SourceReader {
        fn open(&self, path: &Utf8Path) -> std::io::Result<Box<dyn Read>>;
    }
}
