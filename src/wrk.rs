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
use chrono::{DateTime, Duration as ChronoDuration, NaiveDateTime, Utc};
use getset::{Getters, MutGetters, Setters};
use rslua::lexer::Lexer;
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;
use url::Url;

use crate::{
    benchmark::{Benchmark, BenchmarkBuilder},
    error::WrkError,
    result::{Variance, WrkResult, WrkResultBuilder},
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
const DATE_FORMAT: &str = "%Y-%m-%d-%H:%M:%S-%z";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum HistoryPeriod {
    Last,
    Hour,
    Day,
    Week,
    Month,
    Forever,
}

impl Default for HistoryPeriod {
    fn default() -> Self {
        HistoryPeriod::Last
    }
}

impl HistoryPeriod {
    pub fn last_valid_datapoint(&self) -> DateTime<Utc> {
        let now = Utc::now();
        match self {
            Self::Last => now,
            Self::Hour => now.sub(ChronoDuration::hours(1)),
            Self::Day => now.sub(ChronoDuration::days(1)),
            Self::Week => now.sub(ChronoDuration::weeks(1)),
            Self::Month => now.sub(ChronoDuration::weeks(4)),
            Self::Forever => DateTime::from_utc(NaiveDateTime::from_timestamp(1, 0), Utc),
        }
    }
}

pub type Benchmarks = Vec<WrkResult>;
pub type Headers = HashMap<String, String>;

/// Wrapper around Wrk enabling to run benchmarks, record historical data and plot graphs.
#[derive(Debug, Clone, Serialize, Deserialize, Getters, Setters, MutGetters, Builder)]
pub struct Wrk {
    /// Url of the service to benchmark against. Use the full URL of the request.
    /// IE: http://localhost:1234/some/uri.
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    url: String,
    /// Set of benchmarks for the current instance.
    #[builder(default)]
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    benchmarks: Benchmarks,
    /// Historical benchmarks data, indexed by dates.
    #[builder(default)]
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    benchmarks_history: Benchmarks,
    /// Directory on disk where to store and read the historical benchmark data.
    #[builder(default = "Path::new(\".\").join(\".wrk-api-bench\")")]
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    history_dir: PathBuf,
    /// User defined LUA script to run through wrk.
    /// **NOTE: This script MUST not override the wrk function `done()` as it already
    /// overriden by this crate to allow wrk to spit out a parsable JSON output.
    #[builder(default)]
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    user_script: Option<PathBuf>,
    /// Header to add to the wrk request.
    #[builder(default)]
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    headers: Headers,
    /// Method for the wrk request.
    #[builder(default = "String::from(\"GET\")")]
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    method: String,
    /// Body for the wrk request.
    #[builder(default)]
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    body: String,
    /// Max percentage of errors vs total request to conside a benchmark healthy.
    #[builder(default = "2")]
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    max_error_percentage: u8,
    /// Current benchmark date and time.
    #[serde(skip)]
    #[builder(default)]
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    benchmark_date: Option<DateTime<Utc>>,
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
        let date = Utc::now();
        *self.benchmark_date_mut() = Some(date);
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
            *run.date_mut() = date;
            *run.benchmark_mut() = benchmark.clone();
            println!("Current run: {:?}", run);
            self.benchmarks_mut().push(run);
        }
        self.dump(date)?;
        self.load(HistoryPeriod::Last, false)?;
        Ok(())
    }

    pub fn bench_exponential(&mut self, duration: Option<Duration>) -> Result<()> {
        self.bench(&BenchmarkBuilder::exponential(duration))?;
        Ok(())
    }

    fn dump(&self, date: DateTime<Utc>) -> Result<()> {
        let filename = format!("result.{}.json", date.format(DATE_FORMAT));
        println!("Dumping on filename");
        let file = File::create(self.history_dir().join(&filename))?;
        let writer = BufWriter::new(file);
        println!("Writing current benchmark to {}", filename);
        serde_json::to_writer(writer, &self.benchmarks())?;
        Ok(())
    }

    fn load(&mut self, period: HistoryPeriod, best: bool) -> Result<()> {
        if !self.history_dir().exists() {
            fs::create_dir(self.history_dir())?;
        }
        let mut paths: Vec<_> = fs::read_dir(self.history_dir())?.map(|r| r.unwrap()).collect();
        paths.sort_by_key(|dir| {
            let metadata = fs::metadata(dir.path()).unwrap();
            metadata.modified().unwrap()
        });
        let mut history = Benchmarks::new();
        if period == HistoryPeriod::Last {
            let file = File::open(paths.pop().unwrap().path())?;
            let mut reader = BufReader::new(file);
            history = serde_json::from_reader(&mut reader)?;
            let benchmark = history.pop().unwrap();
            if let Some(benchmark_date) = self.benchmark_date() {
                if benchmark_date == benchmark.date() && !paths.is_empty() {
                    let file = File::open(paths.pop().unwrap().path())?;
                    let mut reader = BufReader::new(file);
                    history = serde_json::from_reader(&mut reader)?;
                    if best {
                        let best = self.best_benchmark(&history)?;
                        history = vec![best];
                    }
                }
            }
        } else {
            for path in paths {
                if let Some(date_str) = path.file_name().to_string_lossy().split('.').nth(1) {
                    let date = DateTime::parse_from_str(date_str, DATE_FORMAT)?;
                    if date >= period.last_valid_datapoint() {
                        let file = File::open(path.path())?;
                        let mut reader = BufReader::new(file);
                        let mut benchmarks: Vec<_> = serde_json::from_reader(&mut reader)?;
                        benchmarks.retain(|x| !self.benchmarks_history().contains(x));
                        if best {
                            let best = self.best_benchmark(&benchmarks)?;
                            history.push(best);
                        } else {
                            history.append(&mut benchmarks);
                        }
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
        best.cloned().ok_or_else(|| {
            WrkError::Stats(format!(
                "Unable to calculate best in a set of {} elements",
                benchmarks.len()
            ))
        })
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

    pub fn variance(&mut self, period: HistoryPeriod) -> Result<Variance> {
        let new = self.best()?;
        let old = self.historical_best()?;
        Ok(Variance::new(new, old))
    }
}

mod tests {
    use std::{thread, time::Duration};

    use super::*;
    use crate::benchmark::BenchmarkBuilder;

    #[test]
    fn benchmark() {
        let mut wrk = WrkBuilder::default()
            .url("http://localhost:13734/pokemon-species/pikachu".to_string())
            .build()
            .unwrap();
        // wrk.bench_exponential(Some(Duration::from_secs(5))).unwrap();
        wrk.bench(&vec![BenchmarkBuilder::default()
            .duration(Duration::from_secs(5))
            .build()
            .unwrap()])
            .unwrap();
        println!("MEEEEEEEEEEEEEEEE {:?}", wrk.benchmarks_history());
        println!("MEEEEEEEEEEEEEEEE {}", wrk.variance(HistoryPeriod::Hour).unwrap());
    }
}
