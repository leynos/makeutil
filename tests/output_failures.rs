//! Injected output failures verify the stable writer error boundary.

use camino::Utf8Path;
use makeutil::adapters::{
    cli::{ProcessCapabilities, run_from, run_from_with_reader},
    source::SourceReader,
};
use rstest::rstest;

struct FailingWriter;

struct ReadFailureSourceReader;

impl SourceReader for ReadFailureSourceReader {
    fn open(&self, _path: &Utf8Path) -> std::io::Result<Box<dyn std::io::Read>> {
        Ok(Box::new(FailingReader))
    }
}

struct FailingReader;

impl std::io::Read for FailingReader {
    fn read(&mut self, _buffer: &mut [u8]) -> std::io::Result<usize> {
        Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "broken source stream",
        ))
    }
}

impl std::io::Write for FailingWriter {
    fn write(&mut self, _buffer: &[u8]) -> std::io::Result<usize> {
        Err(std::io::Error::new(
            std::io::ErrorKind::BrokenPipe,
            "closed",
        ))
    }

    fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
}

#[rstest]
fn broken_stdout_exits_two_with_stable_operation() {
    let mut stdin = b"all:\n\techo ok\n".as_slice();
    let mut stdout = FailingWriter;
    let mut stderr = Vec::new();
    let outcome = run_from(
        ["makeutil", "parse", "--stdin-filename", "Makefile", "-"],
        &mut stdin,
        &mut stdout,
        &mut stderr,
    );
    assert_eq!(outcome.exit_code, 2);
    assert!(String::from_utf8_lossy(&stderr).contains("makeutil: stdout-write:"));
}

#[rstest]
fn broken_path_reader_exits_two_with_stable_operation() {
    let mut stdin = std::io::empty();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let capabilities = ProcessCapabilities::new(
        &mut stdin,
        &mut stdout,
        &mut stderr,
        &ReadFailureSourceReader,
    );
    let outcome = run_from_with_reader(["makeutil", "parse", "Makefile"], capabilities);
    assert_eq!(outcome.exit_code, 2);
    assert!(String::from_utf8_lossy(&stderr).contains("makeutil: source-read:"));
}
