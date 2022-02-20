use thiserror::Error;

#[derive(Error, Debug)]
pub enum WrkError {
    // Custom errors
    #[error("Execution error: {0}")]
    Exec(String),
    // Custom errors
    #[error("History error: {0}")]
    History(String),
    #[error("Bitmap plotting error: {0}")]
    Bitmap(String),
    // Custom errors
    #[error("Lua error: {0}")]
    Lua(String),
    // Custom errors
    #[error("Plot error: {0}")]
    Plot(String),
    #[error("Statistics error: {0}")]
    Stats(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Serde(#[from] serde_json::Error),
    #[error(transparent)]
    Chrono(#[from] chrono::ParseError),
    #[error(transparent)]
    Url(#[from] url::ParseError),
    #[error(transparent)]
    Tempfile(#[from] tempfile::PersistError),
    #[error(transparent)]
    WrkBuilder(#[from] crate::wrk::WrkBuilderError),
    #[error(transparent)]
    WrkResultBuilder(#[from] crate::result::WrkResultBuilderError),
    #[error(transparent)]
    BenchmarkBuilder(#[from] crate::benchmark::BenchmarkBuilderError),
}
