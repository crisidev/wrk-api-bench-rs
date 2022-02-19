use std::time::Duration;

use getset::{Getters, MutGetters, Setters};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, PartialEq, Hash, Clone, Serialize, Deserialize, Getters, Setters, MutGetters, Builder)]
pub struct Benchmark {
    #[builder(default = "8")]
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    threads: u16,
    #[builder(default = "32")]
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    connections: u16,
    #[builder(default = "Duration::from_secs(30)")]
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    duration: Duration,
}

impl BenchmarkBuilder {
    pub fn exponential(duration: Option<Duration>) -> Vec<Benchmark> {
        let duration = duration.unwrap_or_else(|| Duration::from_secs(30));
        let threads_list = [2, 4, 8, 16];
        let connections_list = [32, 64, 128, 256];
        let mut benchmarks = Vec::new();
        for threads in threads_list {
            for connections in connections_list {
                benchmarks.push(Benchmark {
                    threads,
                    connections,
                    duration,
                });
            }
        }
        benchmarks
    }
}

impl Benchmark {
    pub fn new(threads: u16, connections: u16, duration: u64) -> Self {
        Self {
            threads,
            connections,
            duration: Duration::from_secs(duration),
        }
    }

    pub fn to_key(&self) -> String {
        format!("{}-{}-{}", self.threads, self.connections, self.duration.as_secs())
    }
}
