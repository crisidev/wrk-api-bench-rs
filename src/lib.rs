#![warn(missing_docs, missing_debug_implementations)]

#[macro_use]
extern crate derive_builder;
#[macro_use]
extern crate log;

mod config;
mod error;
mod result;
mod wrk;

pub use config::Benchmark;
pub use error::WrkError;
pub use result::WrkResult;
pub use wrk::Wrk;

pub(crate) type Result<T> = std::result::Result<T, WrkError>;
