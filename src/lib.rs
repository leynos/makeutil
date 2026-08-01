//! Parse one GNU Makefile into deterministic, versioned JSON facts.

pub mod adapters;
pub mod application;
pub mod domain;
pub mod ports;

pub use application::{ParseApplicationError, parse_source};
pub use domain::ParseReport;
