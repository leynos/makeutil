//! Injected output failures verify the stable writer error boundary.

mod common;
#[path = "common/failing_reader.rs"]
mod failing_reader;

use common::MockSourceReader;
use failing_reader::failing_reader;
use makeutil::adapters::cli::{ProcessCapabilities, run_from, run_from_with_reader};
use rstest::rstest;

struct FailingWriter;

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
    let mut source_reader = MockSourceReader::new();
    source_reader
        .expect_open()
        .returning(|_| Ok(failing_reader()));
    let mut stdin = std::io::empty();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let capabilities =
        ProcessCapabilities::new(&mut stdin, &mut stdout, &mut stderr, &source_reader);
    let outcome = run_from_with_reader(["makeutil", "parse", "Makefile"], capabilities);
    assert_eq!(outcome.exit_code, 2);
    assert!(String::from_utf8_lossy(&stderr).contains("makeutil: source-read:"));
}
