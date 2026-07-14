//! Injected source readers verify path input without ambient file-system access.

use std::io::{Cursor, Read};

use camino::Utf8Path;
use googletest::prelude::*;
use makeutil::adapters::source::{SourceReader, read_path};
use rstest::rstest;

struct MemorySourceReader;

impl SourceReader for MemorySourceReader {
    fn open(&self, _path: &Utf8Path) -> std::io::Result<Box<dyn Read>> {
        Ok(Box::new(Cursor::new(b"all:\n\techo ok\n")))
    }
}

struct OpenFailureSourceReader;

impl SourceReader for OpenFailureSourceReader {
    fn open(&self, _path: &Utf8Path) -> std::io::Result<Box<dyn Read>> {
        Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "denied by test capability",
        ))
    }
}

struct ReadFailureSourceReader;

impl SourceReader for ReadFailureSourceReader {
    fn open(&self, _path: &Utf8Path) -> std::io::Result<Box<dyn Read>> {
        Ok(Box::new(FailingReader))
    }
}

struct FailingReader;

impl Read for FailingReader {
    fn read(&mut self, _buffer: &mut [u8]) -> std::io::Result<usize> {
        Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "broken source stream",
        ))
    }
}

#[rstest]
fn injected_reader_supplies_source_bytes() -> googletest::Result<()> {
    let bytes = read_path(&MemorySourceReader, Utf8Path::new("Makefile"))?;
    verify_that!(bytes, eq(b"all:\n\techo ok\n"))
}

#[rstest]
fn open_failures_keep_the_stable_operation() -> googletest::Result<()> {
    let error = read_path(&OpenFailureSourceReader, Utf8Path::new("Makefile"))
        .expect_err("opening should fail");
    verify_that!(error.operation(), eq("source-open"))
}

#[rstest]
fn read_failures_keep_the_stable_operation() -> googletest::Result<()> {
    let error = read_path(&ReadFailureSourceReader, Utf8Path::new("Makefile"))
        .expect_err("reading should fail");
    verify_that!(error.operation(), eq("source-read"))
}
