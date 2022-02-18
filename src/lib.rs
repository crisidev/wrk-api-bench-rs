#![warn(missing_docs, missing_debug_implementations)]

#[macro_use]
extern crate derive_builder;
#[macro_use]
extern crate log;

mod config;
mod result;
mod wrk;
mod error;

pub use error::WrkError;
pub use wrk::Wrk;
pub use config::WrkConfig;
pub use result::WrkResult;

pub(crate) type Result<T> = std::result::Result<T, WrkError>;
