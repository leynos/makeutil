//! Injected output failures verify the stable writer error boundary.

use makeutil::adapters::cli::run_from;
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
