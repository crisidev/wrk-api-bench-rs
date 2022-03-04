use std::{
    collections::HashMap,
    fs::{self, File},
    io::{BufReader, BufWriter},
    ops::Sub,
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};

use chrono::{DateTime, Duration as ChronoDuration, NaiveDateTime, Utc};
use getset::{Getters, MutGetters, Setters};
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;
use url::Url;

use crate::{
    benchmark::{Benchmark, BenchmarkBuilder},
    error::WrkError,
    result::{Variance, WrkResult, WrkResultBuilder},
    Gnuplot, LuaScript, Result,
};

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
    /// Wrk timeout in seconds
    #[serde(skip)]
    #[builder(default = "1")]
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    timeout: u8,
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
    fn wrk_args(&self, benchmark: &Benchmark, url: &Url, lua_script: &Path) -> Result<Vec<String>> {
        Ok(vec![
            "-t".to_string(),
            benchmark.threads().to_string(),
            "-c".to_string(),
            benchmark.connections().to_string(),
            "-d".to_string(),
            format!("{}s", benchmark.duration().as_secs()),
            "--timeout".to_string(),
            format!("{}s", self.timeout()),
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
        let url = Url::parse(self.url())?;
        let mut script_file = NamedTempFile::new()?;
        LuaScript::render(
            &mut script_file,
            self.user_script().as_ref(),
            url.path(),
            self.method(),
            self.headers(),
            self.body(),
        )?;
        for benchmark in benchmarks {
            let mut run = match Command::new("wrk")
                .args(self.wrk_args(benchmark, &url, script_file.path())?)
                .output()
            {
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
            self.benchmarks_mut().push(run);
        }
        script_file.keep()?;
        self.dump(date)?;
        Ok(())
    }

    pub fn bench_exponential(&mut self, duration: Option<Duration>) -> Result<()> {
        self.bench(&BenchmarkBuilder::exponential(duration))?;
        Ok(())
    }

    fn dump(&self, date: DateTime<Utc>) -> Result<()> {
        let filename = format!("result.{}.json", date.format(DATE_FORMAT));
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
                } else {
                    return Err(WrkError::History(
                        "Unable to load history with a single measurement".to_string(),
                    ));
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

    pub fn all_benchmarks(&self) -> Benchmarks {
        let mut history = self.benchmarks_history().clone();
        history.append(&mut self.benchmarks().clone());
        history
    }

    pub fn variance(&mut self, period: HistoryPeriod) -> Result<Variance> {
        self.load(period, false)?;
        let new = self.best()?;
        let old = self.historical_best()?;
        Ok(Variance::new(new, old))
    }

    pub fn plot(&self, title: &str, output: &Path, benchmarks: &Benchmarks) -> Result<()> {
        Gnuplot::new(title, output).plot(benchmarks)
    }
}

// #[cfg(test)]
// mod tests {
//     use std::{thread, time::Duration};

//     use super::*;
//     use crate::benchmark::BenchmarkBuilder;

//     #[test]
//     fn benchmark() {
//         let mut wrk = WrkBuilder::default()
//             .url("http://localhost:13734/some".to_string())
//             .build()
//             .unwrap();
//         // wrk.bench_exponential(Some(Duration::from_secs(30))).unwrap();
//         // println!("{}", wrk.variance(HistoryPeriod::Last).unwrap());
//         wrk.bench(&vec![BenchmarkBuilder::default()
//             .duration(Duration::from_secs(5))
//             .build()
//             .unwrap()])
//             .unwrap();
//         wrk.load(HistoryPeriod::Day, false).unwrap();
//         // wrk.plot("Wrk Weeeeeee", Path::new("./some.png"), &wrk.all_benchmarks())
//         // .unwrap();
//     }
// }

#[cfg(test)]
mod tests {
    use std::{net::SocketAddr, thread, time::Duration};

    use super::*;
    use crate::benchmark::BenchmarkBuilder;
    use axum::{
        http::StatusCode,
        response::IntoResponse,
        routing::{get, post},
        Json, Router,
    };
    use http::Request;
    use hyper::Body;

    async fn server() {
        let app = Router::new().route("/", get(|| async { "Hello, world!" }));

        let addr = SocketAddr::from(([127, 0, 0, 1], 13734));
        println!("server listening on {}", addr);
        axum::Server::bind(&addr).serve(app.into_make_service()).await.unwrap();
    }

    #[tokio::test]
    async fn benchmark() {
        tokio::spawn(server());
        tokio::time::sleep(Duration::from_secs(1)).await;
        let client = hyper::Client::new();

        let response = client
            .request(
                Request::builder()
                    .uri("http://127.0.0.1:13734/")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();

        let mut wrk = WrkBuilder::default()
            .url("http://127.0.0.1:13734".to_string())
            .build()
            .unwrap();
        // wrk.bench_exponential(Some(Duration::from_secs(30))).unwrap();
        wrk.bench(&vec![BenchmarkBuilder::default()
            .duration(Duration::from_secs(5))
            .build()
            .unwrap()])
            .unwrap();
        // println!("{}", wrk.variance(HistoryPeriod::Hour).unwrap());
        // wrk.load(HistoryPeriod::Day, false).unwrap();
        // wrk.plot("Wrk Weeeeeee", Path::new("./some.png"), &wrk.all_benchmarks())
        // .unwrap();
    }
}
