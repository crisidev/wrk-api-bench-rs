use std::{
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use tempfile::NamedTempFile;

use crate::{wrk::Benchmarks, Result, WrkError};

#[derive(Debug, Clone)]
pub struct Gnuplot {
    title: String,
    output: PathBuf,
}

impl Gnuplot {
    pub fn new(title: &str, output: &Path) -> Self {
        Self {
            title: title.to_string(),
            output: output.to_path_buf(),
        }
    }

    pub fn plot(&self, benchmarks: &Benchmarks) -> Result<()> {
        if benchmarks.len() < 2 {
            return Err(WrkError::Plot(format!(
                "There are {} availble datapoints. Unable to plot history with less than 2 datapoints",
                benchmarks.len()
            )));
        }
        let dates: Vec<_> = benchmarks
            .iter()
            .map(|b| b.date().format("%Y-%m-%d-%H:%M:%S").to_string())
            .collect();
        let serie: Vec<_> = benchmarks.iter().map(|b| *b.requests_sec() as u64).collect();
        let min_x = dates.iter().min().unwrap();
        let max_x = dates.iter().max().unwrap();
        let min_y = *serie.iter().min().unwrap_or(&0) as f64;
        let min_y = (min_y - (min_y * 0.15)) as u64;
        let max_y = *serie.iter().max().unwrap_or(&1000) as f64;
        let max_y = (max_y + (max_y * 0.15)) as u64;
        let mut data_file = NamedTempFile::new()?;
        for (i, b) in benchmarks.iter().enumerate() {
            data_file.write_all(format!("{} {}\n", dates[i], b.requests_sec()).as_bytes())?;
        }
        let gnuplot = format!(
            r#"set xdata time
set timefmt "%Y-%m-%d-%H:%M:%S"
set format x "%m/%y/%d %H:%M:%S"
set xrange ["{}":"{}"]
set yrange [{}:{}]
set key off
set xtics rotate by -45
set title "{}"
set terminal png
set output "{}"
plot "{}" using 1:2 with linespoints linetype 6 linewidth 2"#,
            min_x,
            max_x,
            min_y,
            max_y,
            self.title,
            self.output.display(),
            data_file.path().display()
        );
        let mut child = Command::new("gnuplot").stdin(Stdio::piped()).spawn()?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(gnuplot.as_ref())?;
        }
        let status = child.wait()?;
        if status.success() {
            Ok(())
        } else {
            let err = WrkError::Plot(format!(
                "Error plotting file {} which is kept for debug",
                data_file.path().display()
            ));
            data_file.keep()?;
            Err(err)
        }
    }
}
