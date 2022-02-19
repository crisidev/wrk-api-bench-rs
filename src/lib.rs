#![warn(missing_docs, missing_debug_implementations)]

#[macro_use]
extern crate derive_builder;
#[macro_use]
extern crate log;

mod benchmark;
mod error;
mod lua;
mod result;
mod wrk;

pub use benchmark::{Benchmark, BenchmarkBuilder, BenchmarkBuilderError};
pub use error::WrkError;
pub use lua::LuaScript;
pub use result::{WrkResult, WrkResultBuilder, WrkResultBuilderError};
pub use wrk::{Headers, Wrk, WrkBuilder, WrkBuilderError};

pub(crate) type Result<T> = std::result::Result<T, WrkError>;
