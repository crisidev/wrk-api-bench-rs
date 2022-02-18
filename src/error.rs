use thiserror::Error;

#[derive(Error, Debug)]
pub enum WrkError {
    // Custom errors
    #[error("Execution error: {0}")]
    Exec(String),
    // Custom errors
    #[error("Lua error: {0}")]
    Lua(String),
    #[error("Statistics error: {0}")]
    Stats(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Serde(#[from] serde_json::Error),
    #[error(transparent)]
    TimeFormat(#[from] time::error::Format),
    #[error(transparent)]
    TimeParse(#[from] time::error::Parse),
    #[error(transparent)]
    TimeDesc(#[from] time::error::InvalidFormatDescription),
    #[error(transparent)]
    Url(#[from] url::ParseError),
    #[error(transparent)]
    WrkBuilder(#[from] crate::wrk::WrkBuilderError),
    #[error(transparent)]
    ConfigBuilder(#[from] crate::config::BenchmarkBuilderError),
}
