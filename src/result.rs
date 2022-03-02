use std::fmt;

use chrono::{DateTime, Utc};
use getset::{Getters, MutGetters, Setters};
use prettytable::{format, Attr, Cell, Row, Table};
use serde::{Deserialize, Serialize};

use crate::{Benchmark, BenchmarkBuilder};

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone, Getters, Setters, MutGetters, Builder)]
pub struct WrkResult {
    #[builder(default)]
    #[serde(default)]
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    success: bool,
    #[builder(default = "String::new()")]
    #[serde(default)]
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    error: String,
    #[builder(default)]
    #[serde(default)]
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    benchmark: Benchmark,
    #[builder(default = "Utc::now()")]
    #[serde(default = "Utc::now")]
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    date: DateTime<Utc>,
    #[builder(default = "0.0")]
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    requests: f64,
    #[builder(default = "0.0")]
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    errors: f64,
    #[builder(default = "0.0")]
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    successes: f64,
    #[builder(default = "0.0")]
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    requests_sec: f64,
    #[builder(default = "0.0")]
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    avg_latency_ms: f64,
    #[builder(default = "0.0")]
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    min_latency_ms: f64,
    #[builder(default = "0.0")]
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    max_latency_ms: f64,
    #[builder(default = "0.0")]
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    stdev_latency_ms: f64,
    #[builder(default = "0.0")]
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    transfer_mb: f64,
    #[builder(default = "0.0")]
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    errors_connect: f64,
    #[builder(default = "0.0")]
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    errors_read: f64,
    #[builder(default = "0.0")]
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    errors_write: f64,
    #[builder(default = "0.0")]
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    errors_status: f64,
    #[builder(default = "0.0")]
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    errors_timeout: f64,
}

impl Default for WrkResult {
    fn default() -> Self {
        Self {
            success: false,
            error: String::new(),
            benchmark: Benchmark::default(),
            date: Utc::now(),
            requests: 0.0,
            errors: 0.0,
            successes: 0.0,
            requests_sec: 0.0,
            avg_latency_ms: 0.0,
            min_latency_ms: 0.0,
            max_latency_ms: 0.0,
            stdev_latency_ms: 0.0,
            transfer_mb: 0.0,
            errors_connect: 0.0,
            errors_read: 0.0,
            errors_write: 0.0,
            errors_status: 0.0,
            errors_timeout: 0.0,
        }
    }
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
    pub fn new(new: WrkResult, old: WrkResult) -> Self {
        let requests_sec = Self::calculate(new.requests_sec(), old.requests_sec());
        let requests = Self::calculate(new.requests(), old.requests());
        let successes = Self::calculate(new.successes(), old.successes());
        let errors = Self::calculate(new.errors(), old.errors());
        let avg_latency_ms = Self::calculate(new.avg_latency_ms(), old.avg_latency_ms());
        let min_latency_ms = Self::calculate(new.min_latency_ms(), old.min_latency_ms());
        let max_latency_ms = Self::calculate(new.max_latency_ms(), old.max_latency_ms());
        let stdev_latency_ms = Self::calculate(new.stdev_latency_ms(), old.stdev_latency_ms());
        let transfer_mb = Self::calculate(new.transfer_mb(), old.transfer_mb());
        let errors_connect = Self::calculate(new.errors_connect(), old.errors_connect());
        let errors_read = Self::calculate(new.errors_read(), old.errors_read());
        let errors_write = Self::calculate(new.errors_write(), old.errors_write());
        let errors_status = Self::calculate(new.errors_status(), old.errors_status());
        let errors_timeout = Self::calculate(new.errors_timeout(), old.errors_timeout());
        let variance = WrkResultBuilder::default()
            .date(*new.date())
            .requests(requests)
            .errors(errors)
            .successes(successes)
            .requests_sec(requests_sec)
            .avg_latency_ms(avg_latency_ms)
            .min_latency_ms(min_latency_ms)
            .max_latency_ms(max_latency_ms)
            .stdev_latency_ms(stdev_latency_ms)
            .transfer_mb(transfer_mb)
            .errors_connect(errors_connect)
            .errors_read(errors_read)
            .errors_write(errors_write)
            .errors_status(errors_status)
            .errors_timeout(errors_timeout)
            .build()
            .unwrap();
        Self { variance, new, old }
    }

    fn calculate(new: &f64, old: &f64) -> f64 {
        (new - old) / old * 100.0
    }

    pub fn to_markdown(&self) -> String {
        let mut result =
            String::from("### Rust Wrk benchmark variance report:\n\n|Measurement|Variance|Current|Old|\n|-|-|-|-|\n");
        result += &format!(
            "|Requests/sec|{:.2}%|{}|{}|\n",
            self.variance.requests_sec(),
            self.new.requests_sec(),
            self.old.requests_sec()
        );
        result += &format!(
            "|Total requests|{:.2}%|{}|{}|\n",
            self.variance.requests(),
            self.new.requests(),
            self.old.requests()
        );
        result += &format!(
            "|Total errors|{:.2}%|{}|{}|\n",
            self.variance.errors(),
            self.new.errors(),
            self.old.errors()
        );
        result += &format!(
            "|Total successes|{:.2}%|{}|{}|\n",
            self.variance.successes(),
            self.new.successes(),
            self.old.successes()
        );
        result += &format!(
            "|Average latency ms|{:.2}%|{}|{}|\n",
            self.variance.avg_latency_ms(),
            self.new.avg_latency_ms(),
            self.old.avg_latency_ms()
        );
        result += &format!(
            "|Minimum latency ms|{:.2}%|{}|{}|\n",
            self.variance.min_latency_ms(),
            self.new.min_latency_ms(),
            self.old.min_latency_ms()
        );
        result += &format!(
            "|Maximum latency ms|{:.2}%|{}|{}|\n",
            self.variance.max_latency_ms(),
            self.new.max_latency_ms(),
            self.old.max_latency_ms()
        );
        result += &format!(
            "|Stdev latency ms|{:.2}%|{}|{}|\n",
            self.variance.stdev_latency_ms(),
            self.new.stdev_latency_ms(),
            self.old.stdev_latency_ms()
        );
        result += &format!(
            "|Transfer Mb|{:.2}%|{}|{}|\n",
            self.variance.transfer_mb(),
            self.new.transfer_mb(),
            self.old.transfer_mb()
        );
        result += &format!(
            "|Connect errors|{:.2}%|{}|{}|\n",
            self.variance.errors_connect(),
            self.new.errors_connect(),
            self.old.errors_connect()
        );
        result += &format!(
            "|Read errors|{:.2}%|{}|{}|\n",
            self.variance.errors_read(),
            self.new.errors_read(),
            self.old.errors_read()
        );
        result += &format!(
            "|Write errors|{:.2}%|{}|{}|\n",
            self.variance.errors_write(),
            self.new.errors_write(),
            self.old.errors_write()
        );
        result += &format!(
            "|Status errors (not 2xx/3xx)|{:.2}%|{}|{}|\n",
            self.variance.errors_status(),
            self.new.errors_status(),
            self.old.errors_status()
        );
        result += &format!(
            "|Timeout errors|{:.2}%|{}|{}|\n",
            self.variance.errors_timeout(),
            self.new.errors_timeout(),
            self.old.errors_timeout()
        );
        result
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
