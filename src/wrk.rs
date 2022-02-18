use std::{
    collections::HashMap,
    env,
    fs::{self, File},
    io::{BufReader, BufWriter, Read, Write},
    path::{Path, PathBuf},
    process::Command,
};

use getset::{Getters, MutGetters, Setters};
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;
use time::{OffsetDateTime, format_description};

use crate::{config::WrkConfig, error::WrkError, result::WrkResult, Result};

const LUA_DEFAULT_DONE_FUNCTION: &str = r#"
# The done() function is called at the end of wrk execution
# and allows us to produce a well formed JSON output, prefixed
# by the string "JSON" which allows us to parse the wrk output
# easily.
done = function(summary, latency, requests)
    local errors = summary.errors.connect
        + summary.errors.read
        + summary.errors.write
        + summary.errors.status
        + summary.errors.timeout
    io.write("JSON")
    io.write(string.format(
        [[{{
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
}}
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

#[derive(Debug, Clone, Serialize, Deserialize, Getters, Setters, MutGetters, Builder)]
pub struct Wrk {
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    url: String,
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    data: HashMap<WrkConfig, WrkResult>,
    #[builder(default = "self.default_storage_dir()")]
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    storage_dir: PathBuf,
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    lua_script: Option<PathBuf>,
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    headers: HashMap<String, String>,
    #[builder(default = "self.default_method()")]
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    method: String,
    #[builder(default = "self.default_body()")]
    #[getset(get = "pub", set = "pub", get_mut = "pub")]
    body: String,
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
}

impl Wrk {
    fn lua_script_from_config(&self, uri: &str) -> Result<PathBuf> {
        let request = format!(
            r#"
            # The request() function is called by wrk on all requests
            # and allow us to configure things like headers, method, body, etc..
            request = function()"
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
        Ok(tmpfile.path().to_path_buf())
    }

    fn lua_script_from_user(&self, lua_script: &Path) -> Result<PathBuf> {
        let file = File::open(lua_script)?;
        let mut reader = BufReader::new(file);
        let mut buffer = String::new();
        reader.read_to_string(&mut buffer)?;
        let buffer = buffer + LUA_DEFAULT_DONE_FUNCTION;
        let mut tmpfile = NamedTempFile::new()?;
        tmpfile.write_all(buffer.as_bytes())?;
        Ok(tmpfile.path().to_path_buf())
    }

    fn lua_headers(&self) -> Result<String> {
        let mut result = String::new();
        for (k, v) in self.headers().iter() {
            result += &format!(r#"wrk.headers["{}"] = "{}"\n"#, k, v);
        }
        Ok(result)
    }

    fn lua_find_script(&self, uri: &str) -> Result<PathBuf> {
        match self.lua_script() {
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

    pub fn bench(&mut self, config: WrkConfig, max_error_percentage: u16) -> Result<()> {
        if !self.storage_dir().exists() {
            fs::create_dir(self.storage_dir()).unwrap_or_else(|e| {
                error!(
                    "Unable to create storage dir {}: {}. Statistics calculation could be impaired",
                    self.storage_dir().display(),
                    e
                );
            });
        }
        let lua_script = self.lua_find_script(config.uri())?;
        let args = vec![
            "-t".to_string(),
            config.threads().to_string(),
            "-d".to_string(),
            config.connections().to_string(),
            "-d".to_string(),
            format!("{}s", config.duration().as_secs()),
            "-s".to_string(),
            lua_script.to_string_lossy().to_string(),
            format!("{}/{}", self.url(), config.uri()),
        ];
        let run = match Command::new("wrk").args(args).output() {
            Ok(wrk) => {
                let output = String::from_utf8_lossy(&wrk.stdout);
                let error = String::from_utf8_lossy(&wrk.stderr);
                if wrk.status.success() {
                    debug!("Wrk execution succeded:\n{}", output);
                    let wrk_json = output.split("JSON").nth(1).unwrap_or("{}");
                    match serde_json::from_str::<WrkResult>(wrk_json) {
                        Ok(mut run) => {
                            let error_percentage = run.errors() / 100 * run.requests();
                            if error_percentage < max_error_percentage.into() {
                                *run.success_mut() = true;
                            } else {
                                error!(
                                    "Errors percentage is {}%, which is more than {}%",
                                    error_percentage, max_error_percentage
                                );
                            }
                            run
                        }
                        Err(e) => {
                            error!("Wrk JSON result deserialize failed: {}", e);
                            WrkResult::fail(e.to_string())
                        }
                    }
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
        self.data_mut().insert(config, run);
        Ok(())
    }

    fn dump(&self) -> Result<()> {
        let now = OffsetDateTime::now_utc(); 
        let format = format_description::parse("%Y-%m-%d.%H:%M:%S")?;
        let filename = format!("result.{}.json", now.format(&format)?);
        let file = File::open(self.storage_dir().join(filename))?;
        let writer = BufWriter::new(file);
        serde_json::to_writer(writer, &self.data())?;
        Ok(())
    }

    fn load(&self) -> Result<HashMap<WrkConfig, WrkResult>> {
        let file = File::open(self.storage_dir.join(".smithy-bench/old.json"))?;
        let mut reader = BufReader::new(file);
        Ok(serde_json::from_reader(&mut reader)?)
    }

    fn best(&self) -> Result<WrkResult> {
        let best = self.data().iter().filter(|v| *v.1.success()).max_by(|a, b| {
            a.1.requests_sec()
                .cmp(b.1.requests_sec())
                .then(a.1.successes().cmp(b.1.successes()))
                .then(a.1.requests().cmp(b.1.requests()))
                .then(a.1.transfer_mb().cmp(b.1.transfer_mb()))
        });
        best.map(|x| x.1.clone())
            .ok_or(WrkError::Stats("Unable to calculate best run in set".to_string()))
    }

    fn compare(&self) -> Result<()> {
        let best = self.best()?;
        let old = self.load()?;
        Ok(())
    }
}
