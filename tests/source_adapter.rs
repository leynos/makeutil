//! Injected source readers verify path input without ambient file-system access.

mod common;
#[path = "common/failing_reader.rs"]
mod failing_reader;

use std::io::Cursor;

use camino::Utf8Path;
use common::MockSourceReader;
use failing_reader::failing_reader;
use googletest::prelude::*;
use makeutil::adapters::source::read_path;
use rstest::rstest;

#[rstest]
fn injected_reader_supplies_source_bytes() -> googletest::Result<()> {
    let mut reader = MockSourceReader::new();
    reader
        .expect_open()
        .returning(|_| Ok(Box::new(Cursor::new(b"all:\n\techo ok\n"))));
    let bytes = read_path(&reader, Utf8Path::new("Makefile"))?;
    verify_that!(bytes, eq(b"all:\n\techo ok\n"))
}

#[rstest]
fn open_failures_keep_the_stable_operation() -> googletest::Result<()> {
    let mut reader = MockSourceReader::new();
    reader.expect_open().returning(|_| {
        Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "denied by test capability",
        ))
    });
    let error = read_path(&reader, Utf8Path::new("Makefile")).expect_err("opening should fail");
    verify_that!(error.operation(), eq("source-open"))
}

#[rstest]
fn read_failures_keep_the_stable_operation() -> googletest::Result<()> {
    let mut reader = MockSourceReader::new();
    reader.expect_open().returning(|_| Ok(failing_reader()));
    let error = read_path(&reader, Utf8Path::new("Makefile")).expect_err("reading should fail");
    verify_that!(error.operation(), eq("source-read"))
}
