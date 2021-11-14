mod aggregator;
mod reader;
mod searcher;
mod writer;

pub use aggregator::*;
pub use reader::*;
pub use searcher::*;
pub use writer::*;

use crate::{base::MetricItem, Error, Result};
use lazy_static::lazy_static;
use regex::Regex;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

// METRIC_FILENAME_SUFFIX represents the suffix of the metric file.
static METRIC_FILENAME_SUFFIX: &'static str = "metrics.log";
// METRIC_IDX_SUFFIX represents the suffix of the metric index file.
static METRIC_IDX_SUFFIX: &'static str = ".idx";
// FILE_LOCK_SUFFIX represents the suffix of the lock file.
static FILE_LOCK_SUFFIX: &'static str = ".lck";
// FILE_PID_PREFIX represents the pid flag of filename.
static FILE_PID_PREFIX: &'static str = "pid";

static METRIC_FILE_PATTERN: &'static str = r"\.[0-9]{4}-[0-9]{2}-[0-9]{2}(\.[0-9]*)?";

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
        begin_time_ms: u64,
        end_time_ms: u64,
        resource: &String,
    ) -> Result<MetricItem>;
    fn find_from_time_with_max_lines(begin_time_ms: u64, max_lines: u32) -> Result<MetricItem>;
}

// Generate the metric file name from the service name.
fn form_metric_filename(service_name: &str, with_pid: bool) -> PathBuf {
    let dot = ".";
    let separator = "-";
    let mut filename = String::new();
    if service_name.contains(dot) {
        filename = service_name.replace(dot, separator);
    }
    let mut filename = format!("{}{}{}", service_name, separator, METRIC_FILENAME_SUFFIX);
    if with_pid {
        let pid = std::process::id();
        filename = format!("{}.pid{}", filename, pid);
    }
    PathBuf::from(filename)
}

// Generate the metric index filename from the metric log filename.
fn form_metric_idx_filename(metric_filename: &Path) -> PathBuf {
    metric_filename.join(METRIC_IDX_SUFFIX)
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
    file_pattern: &PathBuf,
    predicate: fn(&str, &str) -> bool,
) -> Result<Vec<PathBuf>> {
    let dir = fs::read_dir(base_dir)?;
    let mut arr = Vec::new();
    for f in dir {
        let f = f?.path();
        if f.is_dir() {
            continue;
        }
        let name = f.to_str().unwrap();
        if predicate(name, file_pattern.to_str().unwrap())
            && !name.ends_with(METRIC_IDX_SUFFIX)
            && !name.ends_with(FILE_LOCK_SUFFIX)
        {
            // Put the absolute path into the slice.
            arr.push(Path::new(base_dir).join(name));
        }
    }
    if arr.len() > 1 {
        arr.sort_by(filename_comparator);
    }
    Ok(arr)
}

/// List metrics files
/// baseDir: the directory of metrics files
/// filePattern: metric file pattern
fn list_metric_files(base_dir: &PathBuf, file_pattern: &PathBuf) -> Result<Vec<PathBuf>> {
    return list_metric_files_conditional(base_dir, file_pattern, filename_matches);
}

fn filename_comparator(file1: &PathBuf, file2: &PathBuf) -> Ordering {
    let name1 = file1.file_name().unwrap().to_str().unwrap();
    let name2 = file2.file_name().unwrap().to_str().unwrap();
    let a1 = name1.split(".").collect::<Vec<&str>>();
    let a2 = name2.split(".").collect::<Vec<&str>>();
    let mut date_str1 = a1[2];
    let mut date_str2 = a2[2];

    // in case of file name contains pid, skip it, like Sentinel-Admin-metrics.log.pid22568.2018-12-24
    if a1[2].starts_with(FILE_PID_PREFIX) {
        date_str1 = a1[3];
        date_str2 = a2[3];
    }

    // compare date first
    if date_str1 != date_str2 {
        return date_str1.cmp(&date_str2);
    }

    // same date, compare the file number
    match name1.len().cmp(&name2.len()) {
        Ordering::Equal => name1.cmp(&name2),
        Ordering::Less => Ordering::Less,
        Ordering::Greater => Ordering::Greater,
    }
}
