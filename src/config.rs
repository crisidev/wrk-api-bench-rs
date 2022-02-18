use core::fmt;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use getset::{Getters, Setters, MutGetters};

#[derive(Debug, PartialEq, Hash, Clone, Serialize, Deserialize, Getters, Setters, MutGetters)]
pub struct WrkConfig {
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    threads: u16,
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    connections: u16,
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    duration: Duration,
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    uri: String,
}

impl Eq for WrkConfig {}

impl WrkConfig {
    pub fn new(threads: u16, connections: u16, duration: u64, uri: &str) -> Self {
        Self {
            threads,
            connections,
            duration: Duration::from_secs(duration),
            uri: uri.to_string(),
        }
    }
}

impl fmt::Display for WrkConfig {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "threads: {} connections: {} duration: {} secs",
            self.threads,
            self.connections,
            self.duration.as_secs()
        )
    }
}
