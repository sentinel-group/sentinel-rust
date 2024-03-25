mod aggregator;
mod reader;
mod searcher;
mod writer;

pub use aggregator::*;
// `reader` is utilized in `searcher` for metric deserialization.
pub use reader::*;
// Currently searcher haven't been utilized. But it intends to provide search ability for metric with index files generated in `writer`.
pub use searcher::*;
pub use writer::*;

use crate::{base::MetricItem, Result};
use lazy_static::lazy_static;
use regex::Regex;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

// METRIC_FILENAME_SUFFIX represents the suffix of the metric file.
static METRIC_FILENAME_SUFFIX: &str = "metrics.log";
// METRIC_IDX_SUFFIX represents the suffix of the metric index file.
static METRIC_IDX_SUFFIX: &str = ".idx";
// FILE_LOCK_SUFFIX represents the suffix of the lock file.
static FILE_LOCK_SUFFIX: &str = ".lck";
// FILE_PID_PREFIX represents the pid flag of filename.
static FILE_PID_PREFIX: &str = "pid";

static METRIC_FILE_PATTERN: &str = r"\.[0-9]{4}-[0-9]{2}-[0-9]{2}(\.[0-9]*)?";

type MetricItemVec = Vec<MetricItem>;
type MetricTimeMap = HashMap<u64, MetricItemVec>;

lazy_static! {
    static ref METRIC_FILE_REGEX: Regex = Regex::new(METRIC_FILE_PATTERN).unwrap();
}

// MetricLogWriter writes and flushes metric items to current metric log.
pub trait MetricLogWriter {
    fn write(&mut self, ts: u64, items: &mut Vec<MetricItem>) -> Result<()>;
}

// MetricSearcher searches metric items from the metric log file under given condition.
pub trait MetricSearcher {
    fn find_by_time_and_resource(
        &self,
        begin_time_ms: u64,
        end_time_ms: u64,
        resource: &str,
    ) -> Result<MetricItemVec>;
    fn find_from_time_with_max_lines(
        &self,
        begin_time_ms: u64,
        max_lines: usize,
    ) -> Result<MetricItemVec>;
}

// Generate the metric file name from the service name.
fn form_metric_filename(service_name: &str, with_pid: bool) -> String {
    let separator = "-";
    let mut filename = if service_name.contains('.') {
        service_name.replace('.', separator)
    } else {
        service_name.to_string()
    };

    filename.push_str(&format!("{}{}", separator, METRIC_FILENAME_SUFFIX));

    if with_pid {
        let pid = std::process::id();
        filename.push_str(&format!(".pid{}", pid));
    }

    filename
}

// Generate the metric index filename from the metric log filename.
fn form_metric_idx_filename(metric_filename: &str) -> String {
    format!("{}{}", metric_filename, METRIC_IDX_SUFFIX)
}

fn filename_matches(filename: &str, base_filename: &str) -> bool {
    if !filename.starts_with(base_filename) {
        return false;
    }
    let part = &filename[base_filename.len()..];
    // part is like: ".yyyy-MM-dd.number", eg. ".2018-12-24.11"
    METRIC_FILE_REGEX.is_match(part)
}

fn list_metric_files_conditional(
    base_dir: &PathBuf,
    file_pattern: &Path,
    predicate: fn(&str, &str) -> bool,
) -> Result<Vec<PathBuf>> {
    let dir = fs::read_dir(base_dir)?;
    let mut arr = Vec::new();
    for f in dir {
        let f = f?.path();
        match f.file_name() {
            Some(name) => {
                if let Some(name) = name.to_str() {
                    if predicate(name, file_pattern.to_str().unwrap())
                        && !name.ends_with(METRIC_IDX_SUFFIX)
                        && !name.ends_with(FILE_LOCK_SUFFIX)
                    {
                        // Put the absolute path into the slice.
                        arr.push(Path::new(base_dir).join(name));
                    }
                }
            }
            None => continue,
        }
    }
    if arr.len() > 1 {
        arr.sort_by(filename_comparator);
    }
    Ok(arr)
}

/// List metrics files according to `base_dir` (the directory of metrics files) and
/// `file_pattern` (metric file pattern).
fn list_metric_files(base_dir: &PathBuf, file_pattern: &Path) -> Result<Vec<PathBuf>> {
    list_metric_files_conditional(base_dir, file_pattern, filename_matches)
}

/// Sort the metric files by their date time.
/// This function is used to remove the deprecated files, or create a new file in order.
#[allow(clippy::ptr_arg)]
fn filename_comparator(file1: &PathBuf, file2: &PathBuf) -> Ordering {
    let name1 = file1.file_name().unwrap().to_str().unwrap();
    let name2 = file2.file_name().unwrap().to_str().unwrap();
    let a1 = name1.split('.').collect::<Vec<&str>>();
    let a2 = name2.split('.').collect::<Vec<&str>>();
    let mut date_str1 = a1[2];
    let mut date_str2 = a2[2];

    // in case of file name contains pid, skip it, like Sentinel-Admin-metrics.log.pid22568.2018-12-24
    if a1[2].starts_with(FILE_PID_PREFIX) {
        date_str1 = a1[3];
        date_str2 = a2[3];
    }

    // compare date first
    if date_str1 != date_str2 {
        return date_str1.cmp(date_str2);
    }

    // same date, compare the file number
    name1.cmp(name2)
}

#[cfg(test)]
mod tests {
    use std::{
        fs::File,
        path::{Path, PathBuf},
    };

    use tempfile::tempdir;

    use crate::log::metric::{
        filename_comparator, filename_matches, form_metric_filename, list_metric_files,
    };

    #[test]
    fn test_form_metric_filename() {
        let app_name1 = "foo-test";
        let app_name2 = "foo.test";
        let mf1 = form_metric_filename(app_name1, false);
        let mf2 = form_metric_filename(app_name2, false);
        assert_eq!("foo-test-metrics.log", mf1);
        assert_eq!(mf1, mf2);

        let mf1_pid = form_metric_filename(app_name2, true);
        assert!(mf1_pid.ends_with(&std::process::id().to_string()));
    }

    #[test]
    fn test_filename_matches() {
        let test_cases = vec![
            (
                "~/logs/csp/app1-metric.log.2018-12-24.1111",
                "~/logs/csp/app1-metric.log",
                true,
            ),
            (
                "~/logs/csp/app1-metric.log-2018-12-24.1111",
                "~/logs/csp/app1-metric.log",
                false,
            ),
            (
                "~/logs/csp/app2-metric.log.2018-12-24.1111",
                "~/logs/csp/app1-metric.log",
                false,
            ),
        ];

        for (filename, base_filename, expected) in test_cases {
            assert_eq!(filename_matches(filename, base_filename), expected);
        }
    }

    #[test]
    fn test_filename_comparator_no_pid() {
        let mut arr = vec![
            PathBuf::from("metrics.log.2018-03-06"),
            PathBuf::from("metrics.log.2018-03-07"),
            PathBuf::from("metrics.log.2018-03-07.51"),
            PathBuf::from("metrics.log.2018-03-07.10"),
            PathBuf::from("metrics.log.2018-03-06.100"),
        ];
        arr.sort_by(filename_comparator);

        let expected = vec![
            PathBuf::from("metrics.log.2018-03-06"),
            PathBuf::from("metrics.log.2018-03-06.100"),
            PathBuf::from("metrics.log.2018-03-07"),
            PathBuf::from("metrics.log.2018-03-07.10"),
            PathBuf::from("metrics.log.2018-03-07.51"),
        ];

        assert_eq!(expected, arr);
    }

    #[test]
    fn test_filename_comparator_with_pid() {
        let mut arr = vec![
            PathBuf::from("metrics.log.pid21879.2018-03-06"),
            PathBuf::from("metrics.log.pid21879.2018-03-07"),
            PathBuf::from("metrics.log.pid21879.2018-03-07.51"),
            PathBuf::from("metrics.log.pid21879.2018-03-07.10"),
            PathBuf::from("metrics.log.pid21879.2018-03-06.100"),
        ];
        arr.sort_by(filename_comparator);

        let expected = vec![
            PathBuf::from("metrics.log.pid21879.2018-03-06"),
            PathBuf::from("metrics.log.pid21879.2018-03-06.100"),
            PathBuf::from("metrics.log.pid21879.2018-03-07"),
            PathBuf::from("metrics.log.pid21879.2018-03-07.10"),
            PathBuf::from("metrics.log.pid21879.2018-03-07.51"),
        ];

        assert_eq!(expected, arr);
    }

    #[test]
    fn test_list_metric_files() {
        let temp_dir = tempdir().expect("Failed to create temporary directory");

        let base_dir = temp_dir.path().to_path_buf();
        let file_pattern = Path::new("app1-metrics.log");

        // Create temporary files and directories for testing
        let paths = [
            "app1-metrics.log.2020-02-14",
            "app1-metrics.log.2020-02-14.12",
            "app1-metrics.log.2020-02-14.32",
            "app1-metrics.log.2020-02-15",
            "app1-metrics.log.2020-02-16",
            "app1-metrics.log.2020-02-16.100",
            "app2-metrics.log.2020-02-14",
        ];

        for path in paths.iter() {
            let file_path = temp_dir.path().join(path);
            File::create(&file_path).expect("Failed to create test file");
        }

        let result =
            list_metric_files(&base_dir, file_pattern).expect("Failed to list metric files");

        // Assert that the number of files found matches the expected count
        assert_eq!(result.len(), 6);
    }
}
