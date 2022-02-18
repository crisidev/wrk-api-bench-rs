use serde::{Serialize, Deserialize};
use getset::{Getters, Setters, MutGetters};

#[derive(Serialize, Deserialize, Debug, Default, Clone, Getters, Setters, MutGetters)]
pub struct WrkResult {
    #[serde(skip)]
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    success: bool,
    #[serde(skip)]
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    error: String,
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    requests: u64,
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    errors: u64,
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    successes: u64,
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    requests_sec: u64,
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    avg_latency_ms: f64,
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    min_latency_ms: f64,
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    max_latency_ms: f64,
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    stdev_latency_ms: f64,
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    transfer_mb: u64,
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    errors_connect: usize,
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    errors_read: usize,
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    errors_write: usize,
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    errors_status: usize,
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    errors_timeout: usize,
}

impl WrkResult {
    pub fn fail(error: String) -> Self {
        Self {
            error,
            ..Default::default()
        }
    }
}
