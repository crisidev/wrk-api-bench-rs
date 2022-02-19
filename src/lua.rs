use std::{
    env,
    fs::File,
    io::{BufReader, Read, Write},
    path::{Path, PathBuf},
};

use rslua::lexer::Lexer;
use tempfile::NamedTempFile;

use crate::{Headers, Result, WrkError};

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

pub struct LuaScript {}

impl LuaScript {
    fn lua_script_from_config(&self, uri: &str, method: &str, headers: &Headers, body: &str) -> Result<PathBuf> {
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
            method,
            body,
            self.lua_headers(headers)?,
            method,
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

    fn lua_headers(&self, headers: &Headers) -> Result<String> {
        let mut result = String::new();
        for (k, v) in headers {
            result += &format!(r#"wrk.headers["{}"] = "{}"\n"#, k, v);
        }
        Ok(result)
    }

    pub fn render(
        user_script: Option<&PathBuf>,
        uri: &str,
        method: &str,
        headers: &Headers,
        body: &str,
    ) -> Result<PathBuf> {
        let this = Self {};
        match user_script {
            Some(lua_script) => {
                if !lua_script.exists() {
                    error!(
                        "Wrk Lua file {} not found in {}",
                        env::current_dir().expect("unable to get current directory").display(),
                        lua_script.display()
                    );
                    Err(WrkError::Lua("Wrk Lua file not found".to_string()))
                } else {
                    Ok(this.lua_script_from_user(&lua_script)?)
                }
            }
            None => Ok(this.lua_script_from_config(uri, method, headers, body)?),
        }
    }
}
