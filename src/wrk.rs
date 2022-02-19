use std::{
    collections::HashMap,
    env,
    fs::{self, File},
    io::{BufReader, BufWriter, Read, Write},
    ops::Sub,
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, SystemTime},
};

use assert_cmd::prelude::OutputOkExt;
use getset::{Getters, MutGetters, Setters};
use rslua::lexer::Lexer;
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;
use time::{format_description, Instant, OffsetDateTime};
use url::Url;

use crate::{
    config::{Benchmark, BenchmarkBuilder},
    error::WrkError,
    result::{Variance, WrkResultBuilder, WrkResult},
    Result,
};

const LUA_DEFAULT_DONE_FUNCTION: &str = r#"
-- The done() function is called at the end of wrk execution
-- and allows us to produce a well formed JSON output, prefixed
-- by the string "JSON" which allows us to parse the wrk output
-- easily.
done = function(summary, latency, requests)
    local errors = summary.errors.connect
        + summary.errors.read
        + summary.errors.write
        + summary.errors.status
        + summary.errors.timeout
    io.write("JSON")
    io.write(string.format(
        [[{
    "requests": %d,
    "errors": %d,
    "successes": %d,
    "requests_sec": %d,
    "avg_latency_ms": %.2f,
    "min_latency_ms": %.2f,
    "max_latency_ms": %.2f,
    "stdev_latency_ms": %.2f,
    "transfer_mb": %.d,
    "errors_connect": %d,
    "errors_read": %d,
    "errors_write": %d,
    "errors_status": %d,
    "errors_timeout": %d
}
]],
        summary.requests,
        errors,
        summary.requests - errors,
        summary.requests / (summary.duration / 1000000),
        (latency.mean / 1000),
        (latency.min / 1000),
        (latency.max / 1000),
        (latency.stdev / 1000),
        (summary.bytes / 1048576),
        summary.errors.connect,
        summary.errors.read,
        summary.errors.write,
        summary.errors.status,
        summary.errors.timeout
    ))
end
"#;
const DATE_FORMAT: &str =
    "[year]-[month]-[day]:[hour]:[minute]:[second]-[offset_hour sign:mandatory]:[offset_minute]:[offset_second]";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum HistoryPeriod {
    Last,
    Hour,
    Day,
    Week,
    Month,
}

impl Default for HistoryPeriod {
    fn default() -> Self {
        HistoryPeriod::Last
    }
}

impl HistoryPeriod {
    pub fn last(&self) -> OffsetDateTime {
        let now = OffsetDateTime::now_utc();
        match self {
            Self::Last => now,
            Self::Hour => now.sub(time::Duration::HOUR),
            Self::Day => now.sub(time::Duration::DAY),
            Self::Week => now.sub(time::Duration::WEEK),
            Self::Month => now.sub(time::Duration::WEEK * 4),
        }
    }
}

pub type Benchmarks = Vec<WrkResult>;
pub type BenchmarksHistory = Vec<Benchmarks>;
pub type Headers = HashMap<String, String>;

/// Wrapper around Wrk enabling to run benchmarks, record historical data and plot graphs.
#[derive(Debug, Clone, Serialize, Deserialize, Getters, Setters, MutGetters, Builder)]
pub struct Wrk {
    /// Url of the service to benchmark against. Use the full URL of the request.
    /// IE: http://localhost:1234/some/uri.
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    url: String,
    /// Set of benchmarks for the current instance.
    #[builder(default = "self.default_benchmarks()")]
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    benchmarks: Benchmarks,
    /// Historical benchmarks data, indexed by dates.
    #[builder(default = "self.default_benchmarks()")]
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    benchmarks_history: Benchmarks,
    /// Directory on disk where to store and read the historical benchmark data.
    #[builder(default = "self.default_storage_dir()")]
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    history_dir: PathBuf,
    /// User defined LUA script to run through wrk.
    /// **NOTE: This script MUST not override the wrk function `done()` as it already
    /// overriden by this crate to allow wrk to spit out a parsable JSON output.
    #[builder(default = "self.default_user_script()")]
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    user_script: Option<PathBuf>,
    /// Header to add to the wrk request.
    #[builder(default = "self.default_headers()")]
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    headers: Headers,
    /// Method for the wrk request.
    #[builder(default = "self.default_method()")]
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    method: String,
    /// Body for the wrk request.
    #[builder(default = "self.default_body()")]
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    body: String,
    /// Max percentage of errors vs total request to conside a benchmark healthy.
    #[builder(default = "self.default_max_error_percentage()")]
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    max_error_percentage: u8,
    /// Current benchmark date and time.
    #[serde(skip)]
    #[builder(default = "self.default_benchmark_date()")]
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    benchmark_date: Option<String>,
}

impl WrkBuilder {
    fn default_method(&self) -> String {
        String::from("GET")
    }
    fn default_body(&self) -> String {
        String::new()
    }
    fn default_storage_dir(&self) -> PathBuf {
        Path::new(".").join(".wrk-api-bench")
    }
    fn default_benchmarks(&self) -> Benchmarks {
        Benchmarks::new()
    }
    fn default_headers(&self) -> Headers {
        Headers::new()
    }
    fn default_user_script(&self) -> Option<PathBuf> {
        None
    }
    fn default_max_error_percentage(&self) -> u8 {
        2
    }
    fn default_benchmark_date(&self) -> Option<String> {
        None
    }
}

impl Wrk {
    fn lua_script_from_config(&self, uri: &str) -> Result<PathBuf> {
        let request = format!(
            r#"
-- The request() function is called by wrk on all requests
-- and allow us to configure things like headers, method, body, etc..
request = function()
    wrk.method = "{}"
    wrk.body = "{}"
    {}
    return wrk.format("{}", "{}")
end
        "#,
            self.method(),
            self.body(),
            self.lua_headers()?,
            self.method(),
            uri
        );
        let buffer = request + LUA_DEFAULT_DONE_FUNCTION;
        let mut tmpfile = NamedTempFile::new()?;
        tmpfile.write_all(buffer.as_bytes())?;
        let result = tmpfile.path().to_path_buf();
        tmpfile.keep().unwrap();
        Ok(result)
    }

    fn lua_script_from_user(&self, lua_script: &Path) -> Result<PathBuf> {
        let file = File::open(lua_script)?;
        let mut reader = BufReader::new(file);
        let mut buffer = String::new();
        reader.read_to_string(&mut buffer)?;
        let mut lexer = Lexer::new();
        let tokens = lexer.run(&buffer).map_err(|e| WrkError::Lua(format!("{:?}", e)))?;
        let buffer = buffer + LUA_DEFAULT_DONE_FUNCTION;
        let mut tmpfile = NamedTempFile::new()?;
        tmpfile.write_all(buffer.as_bytes())?;
        Ok(tmpfile.path().to_path_buf())
    }

    fn lua_headers(&self) -> Result<String> {
        let mut result = String::new();
        for (k, v) in self.headers() {
            result += &format!(r#"wrk.headers["{}"] = "{}"\n"#, k, v);
        }
        Ok(result)
    }

    fn lua_find_script(&self, uri: &str) -> Result<PathBuf> {
        match self.user_script() {
            Some(lua_script) => {
                if !lua_script.exists() {
                    error!(
                        "Wrk Lua file {} not found in {}",
                        env::current_dir().expect("unable to get current directory").display(),
                        lua_script.display()
                    );
                    Err(WrkError::Lua("Wrk Lua file not found".to_string()))
                } else {
                    Ok(self.lua_script_from_user(lua_script)?)
                }
            }
            None => Ok(self.lua_script_from_config(uri)?),
        }
    }

    fn wrk_args(&self, benchmark: &Benchmark) -> Result<Vec<String>> {
        let url = Url::parse(self.url())?;
        let lua_script = self.lua_find_script(url.path())?;
        Ok(vec![
            "-t".to_string(),
            benchmark.threads().to_string(),
            "-c".to_string(),
            benchmark.connections().to_string(),
            "-d".to_string(),
            format!("{}s", benchmark.duration().as_secs()),
            "-s".to_string(),
            lua_script.to_string_lossy().to_string(),
            url.to_string(),
        ])
    }

    fn wrk_result(&self, wrk_json: &str) -> WrkResult {
        match serde_json::from_str::<WrkResult>(wrk_json) {
            Ok(mut run) => {
                let error_percentage = run.errors() / 100.0 * run.requests();
                if error_percentage < *self.max_error_percentage() as f64 {
                    *run.success_mut() = true;
                } else {
                    error!(
                        "Errors percentage is {}%, which is more than {}%",
                        error_percentage, self.max_error_percentage
                    );
                }
                run
            }
            Err(e) => {
                error!("Wrk JSON result deserialize failed: {}", e);
                WrkResult::fail(e.to_string())
            }
        }
    }

    pub fn bench(&mut self, benchmarks: &Vec<Benchmark>) -> Result<()> {
        if !self.history_dir().exists() {
            fs::create_dir(self.history_dir()).unwrap_or_else(|e| {
                error!(
                    "Unable to create storage dir {}: {}. Statistics calculation could be impaired",
                    self.history_dir().display(),
                    e
                );
            });
        }
        let format = format_description::parse(DATE_FORMAT)?;
        let date = OffsetDateTime::now_utc();
        *self.benchmark_date_mut() = Some(date.format(&format)?);
        for benchmark in benchmarks {
            let mut run = match Command::new("wrk").args(self.wrk_args(benchmark)?).output() {
                Ok(wrk) => {
                    let output = String::from_utf8_lossy(&wrk.stdout);
                    let error = String::from_utf8_lossy(&wrk.stderr);
                    if wrk.status.success() {
                        debug!("Wrk execution succeded:\n{}", output);
                        let wrk_json = output
                            .split("JSON")
                            .nth(1)
                            .ok_or_else(|| WrkError::Lua("Wrk returned empty JSON".to_string()))?;
                        self.wrk_result(wrk_json)
                    } else {
                        error!("Wrk execution failed.\nOutput: {}\nError: {}", output, error);
                        WrkResult::fail(error.to_string())
                    }
                }
                Err(e) => {
                    error!("Wrk execution failed: {}", e);
                    WrkResult::fail(e.to_string())
                }
            };
            *run.date_mut() = date.format(&format)?;
            println!("Current run: {:?}", run);
            self.benchmarks_mut().push(run);
        }
        self.dump(&date.format(&format)?)?;
        Ok(())
    }

    pub fn bench_exponential(&mut self, duration: Option<Duration>) -> Result<()> {
        self.bench(&BenchmarkBuilder::exponential(duration))?;
        Ok(())
    }

    fn dump(&self, date: &str) -> Result<()> {
        let filename = format!("result.{}.json", date);
        let file = File::create(self.history_dir().join(&filename))?;
        let writer = BufWriter::new(file);
        println!("Writing current benchmark to {}", filename);
        serde_json::to_writer(writer, &self.benchmarks())?;
        Ok(())
    }

    fn flatten<T>(&self, nested: Vec<Vec<T>>) -> Vec<T> {
        nested.into_iter().flatten().collect()
    }

    fn load(&mut self, period: HistoryPeriod) -> Result<()> {
        if !self.history_dir().exists() {
            fs::create_dir(self.history_dir())?;
        }
        let mut paths: Vec<_> = fs::read_dir(self.history_dir())?.map(|r| r.unwrap()).collect();
        paths.sort_by_key(|dir| {
            let metadata = fs::metadata(dir.path()).unwrap();
            metadata.modified().unwrap()
        });
        let format = format_description::parse(DATE_FORMAT)?;
        let mut history = Benchmarks::new();
        if period == HistoryPeriod::Last {
            let file = File::open(paths.pop().unwrap().path())?;
            let mut reader = BufReader::new(file);
            history = serde_json::from_reader(&mut reader)?;
            let date_str = history.pop().unwrap().date().to_string();
            if let Some(benchmark_date) = self.benchmark_date() {
                if benchmark_date == &date_str && !paths.is_empty() {
                    let file = File::open(paths.pop().unwrap().path())?;
                    let mut reader = BufReader::new(file);
                    history = serde_json::from_reader(&mut reader)?;
                }
            }
        } else {
            for path in paths {
                if let Some(date_str) = path.file_name().to_string_lossy().split('.').nth(1) {
                    let date = OffsetDateTime::parse(date_str, &format)?;
                    if date >= period.last() {
                        let file = File::open(path.path())?;
                        let mut reader = BufReader::new(file);
                        let benchmarks = serde_json::from_reader(&mut reader)?;
                        let best = self.best_benchmark(&benchmarks)?;
                        history.push(best);
                    }
                }
            }
        }
        *self.benchmarks_history_mut() = history;
        Ok(())
    }

    fn best_benchmark(&self, benchmarks: &Benchmarks) -> Result<WrkResult> {
        let best = benchmarks.iter().filter(|v| *v.success()).max_by(|a, b| {
            (*a.requests_sec() as i64)
                .cmp(&(*b.requests_sec() as i64))
                .then((*a.successes() as i64).cmp(&(*b.successes() as i64)))
                .then((*a.requests() as i64).cmp(&(*b.requests() as i64)))
                .then((*a.requests() as i64).cmp(&(*b.requests() as i64)))
                .then((*a.transfer_mb() as i64).cmp(&(*b.transfer_mb() as i64)))
        });
        best.cloned()
            .ok_or_else(|| WrkError::Stats("Unable to calculate best run in set".to_string()))
    }

    fn best(&self) -> Result<WrkResult> {
        self.best_benchmark(self.benchmarks())
    }

    fn historical_best(&self) -> Result<WrkResult> {
        self.best_benchmark(self.benchmarks_history())
    }

    fn calculate_variance(&self, new: &f64, old: &f64) -> f64 {
        (new - old) / old * 100.0
    }

    pub fn variance(&self) -> Result<Variance> {
        let new = self.best()?;
        let old = self.historical_best()?;
        Ok(Variance::new(new, old))
    }
}

mod tests {
    use std::{thread, time::Duration};

    use super::*;
    use crate::config::BenchmarkBuilder;

    #[test]
    fn benchmark() {
        let mut wrk = WrkBuilder::default()
            .url("http://localhost:13734/pokemon-species/pikachu".to_string())
            .build()
            .unwrap();
        // wrk.bench_exponential(Some(Duration::from_secs(5))).unwrap();
        wrk.bench(&vec![BenchmarkBuilder::default()
            .duration(Duration::from_secs(20))
            .build()
            .unwrap()])
            .unwrap();
        wrk.load(HistoryPeriod::default()).unwrap();
        println!("MEEEEEEEEEEEEEEEE {:?}", wrk.benchmarks_history());
        println!("MEEEEEEEEEEEEEEEE {}", wrk.variance().unwrap());
    }
}
