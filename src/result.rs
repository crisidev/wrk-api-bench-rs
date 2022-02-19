use std::fmt;

use getset::{Getters, MutGetters, Setters};
use prettytable::{format, Attr, Cell, Row, Table};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Default, Clone, Getters, Setters, MutGetters, Builder)]
pub struct WrkResult {
    #[builder(default = "default_false()")]
    #[serde(default = "default_false")]
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    success: bool,
    #[builder(default = "String::new()")]
    #[serde(default)]
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    error: String,
    #[serde(default)]
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    date: String,
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    requests: f64,
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    errors: f64,
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    successes: f64,
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    requests_sec: f64,
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    avg_latency_ms: f64,
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    min_latency_ms: f64,
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    max_latency_ms: f64,
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    stdev_latency_ms: f64,
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    transfer_mb: f64,
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    errors_connect: f64,
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    errors_read: f64,
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    errors_write: f64,
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    errors_status: f64,
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    errors_timeout: f64,
}

fn default_false() -> bool {
    false
}

impl WrkResult {
    pub fn fail(error: String) -> Self {
        Self {
            error,
            ..Default::default()
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct Variance {
    pub variance: WrkResult,
    pub new: WrkResult,
    pub old: WrkResult,
}

impl Variance {
    pub fn new(variance: WrkResult, new: WrkResult, old: WrkResult) -> Self {
        Self { variance, new, old }
    }
}

impl fmt::Display for Variance {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut table = Table::new();
        table.set_format(*format::consts::FORMAT_CLEAN);
        table.add_row(Row::new(vec![
            Cell::new("Measurement").with_style(Attr::Bold),
            Cell::new("Variance").with_style(Attr::Bold),
            Cell::new("Current").with_style(Attr::Bold),
            Cell::new("Old").with_style(Attr::Bold),
        ]));
        table.add_row(Row::new(vec![
            Cell::new("Requests per second").with_style(Attr::Bold),
            Cell::new(&format!("{:.2}%", self.variance.requests_sec())),
            Cell::new(&self.new.requests_sec().to_string()),
            Cell::new(&self.old.requests_sec().to_string()),
        ]));
        table.add_row(Row::new(vec![
            Cell::new("Total requests").with_style(Attr::Bold),
            Cell::new(&format!("{:.2}%", self.variance.requests())),
            Cell::new(&self.new.requests().to_string()),
            Cell::new(&self.old.requests().to_string()),
        ]));
        table.add_row(Row::new(vec![
            Cell::new("Total errors").with_style(Attr::Bold),
            Cell::new(&format!("{:.2}%", self.variance.errors())),
            Cell::new(&self.new.errors().to_string()),
            Cell::new(&self.old.errors().to_string()),
        ]));
        table.add_row(Row::new(vec![
            Cell::new("Total successes").with_style(Attr::Bold),
            Cell::new(&format!("{:.2}%", self.variance.successes())),
            Cell::new(&self.new.successes().to_string()),
            Cell::new(&self.old.successes().to_string()),
        ]));
        table.add_row(Row::new(vec![
            Cell::new("Average latency ms").with_style(Attr::Bold),
            Cell::new(&format!("{:.2}%", self.variance.avg_latency_ms())),
            Cell::new(&self.new.avg_latency_ms().to_string()),
            Cell::new(&self.old.avg_latency_ms().to_string()),
        ]));
        table.add_row(Row::new(vec![
            Cell::new("Minimum latency ms").with_style(Attr::Bold),
            Cell::new(&format!("{:.2}%", self.variance.min_latency_ms())),
            Cell::new(&self.new.min_latency_ms().to_string()),
            Cell::new(&self.old.min_latency_ms().to_string()),
        ]));
        table.add_row(Row::new(vec![
            Cell::new("Maximum latency ms").with_style(Attr::Bold),
            Cell::new(&format!("{:.2}%", self.variance.max_latency_ms())),
            Cell::new(&self.new.max_latency_ms().to_string()),
            Cell::new(&self.old.max_latency_ms().to_string()),
        ]));
        table.add_row(Row::new(vec![
            Cell::new("Stdev latency ms").with_style(Attr::Bold),
            Cell::new(&format!("{:.2}%", self.variance.stdev_latency_ms())),
            Cell::new(&self.new.stdev_latency_ms().to_string()),
            Cell::new(&self.old.stdev_latency_ms().to_string()),
        ]));
        table.add_row(Row::new(vec![
            Cell::new("Transfer Mb").with_style(Attr::Bold),
            Cell::new(&format!("{:.2}%", self.variance.transfer_mb())),
            Cell::new(&self.new.transfer_mb().to_string()),
            Cell::new(&self.old.transfer_mb().to_string()),
        ]));
        table.add_row(Row::new(vec![
            Cell::new("Connect errors").with_style(Attr::Bold),
            Cell::new(&format!("{:.2}%", self.variance.errors_connect())),
            Cell::new(&self.new.errors_connect().to_string()),
            Cell::new(&self.old.errors_connect().to_string()),
        ]));
        table.add_row(Row::new(vec![
            Cell::new("Read errors").with_style(Attr::Bold),
            Cell::new(&format!("{:.2}%", self.variance.errors_read())),
            Cell::new(&self.new.errors_read().to_string()),
            Cell::new(&self.old.errors_read().to_string()),
        ]));
        table.add_row(Row::new(vec![
            Cell::new("Write errors").with_style(Attr::Bold),
            Cell::new(&format!("{:.2}%", self.variance.errors_write())),
            Cell::new(&self.new.errors_write().to_string()),
            Cell::new(&self.old.errors_write().to_string()),
        ]));
        table.add_row(Row::new(vec![
            Cell::new("Status errors (not 2xx/3xx)").with_style(Attr::Bold),
            Cell::new(&format!("{:.2}%", self.variance.errors_status())),
            Cell::new(&self.new.errors_status().to_string()),
            Cell::new(&self.old.errors_status().to_string()),
        ]));
        table.add_row(Row::new(vec![
            Cell::new("Timeout errors").with_style(Attr::Bold),
            Cell::new(&format!("{:.2}%", self.variance.errors_timeout())),
            Cell::new(&self.new.errors_timeout().to_string()),
            Cell::new(&self.old.errors_timeout().to_string()),
        ]));
        write!(f, "## Rust Wrk benchmark variance report:\n{}", table)
    }
}
