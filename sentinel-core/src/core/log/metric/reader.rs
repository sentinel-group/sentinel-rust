use super::*;
use crate::{base, logging};
use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};

const MAX_ITEM_AMOUNT: usize = 100000;

pub trait MetricLogReader {
    fn read_metrics(
        &self,
        name_list: Vec<PathBuf>,
        file_no: usize,
        start_offset: SeekFrom,
        max_lines: usize,
    ) -> Result<MetricItemVec>;

    fn read_metrics_by_end_time(
        &self,
        name_list: Vec<PathBuf>,
        file_no: usize,
        start_offset: SeekFrom,
        begin_ms: u64,
        end_ms: u64,
        resource: String,
    ) -> Result<MetricItemVec>;
}

// Not thread-safe itself, but guarded by the outside MetricSearcher.
#[derive(Default)]
pub struct DefaultMetricLogReader {}

impl DefaultMetricLogReader {
    pub fn new() -> Self {
        DefaultMetricLogReader {}
    }

    fn read_metrics_in_one_file(
        &self,
        filename: &PathBuf,
        offset: SeekFrom,
        max_lines: usize,
        last_sec: u64,
        prev_size: usize,
    ) -> Result<(MetricItemVec, bool)> {
        let file = open_file_and_seek_to(filename, offset)?;
        let mut buf_reader = BufReader::new(file);
        let mut items = Vec::with_capacity(1024);
        let mut last_sec = last_sec;
        loop {
            let mut line = String::new();
            let count = buf_reader.read_line(&mut line)?;
            if count == 0 {
                let should_continue = (prev_size + items.len()) < max_lines;
                return Ok((Vec::new(), should_continue));
            }
            let item = base::MetricItem::from_string(&line);

            match item {
                Ok(item) => {
                    let ts_sec = item.timestamp / 1000;
                    if prev_size + items.len() >= max_lines && ts_sec != last_sec {
                        return Ok((items, false));
                    }
                    items.push(item);
                    last_sec = ts_sec;
                }
                Err(err) => {
                    logging::error!("DefaultMetricLogReader::read_metrics_in_one_file: {:?} Failed to convert to MetricItem. Error: {:?}.", line,err);
                    continue;
                }
            }
        }
    }

    fn read_metrics_one_file_by_end_time(
        &self,
        filename: &PathBuf,
        offset: SeekFrom,
        begin_ms: u64,
        end_ms: u64,
        resource: &String,
        prev_size: usize,
    ) -> Result<(MetricItemVec, bool)> {
        let begin_sec = begin_ms / 1000;
        let end_sec = end_ms / 1000;
        let file = open_file_and_seek_to(filename, offset)?;

        let mut buf_reader = BufReader::new(file);
        let mut items = Vec::with_capacity(1024);
        loop {
            let mut line = String::new();
            let count = buf_reader.read_line(&mut line)?;
            if count == 0 {
                return Ok((Vec::new(), true));
            }
            let item = base::MetricItem::from_string(&line);
            match item {
                Ok(item) => {
                    let ts_sec = item.timestamp / 1000;
                    // current_second should in [begin_sec, end_sec]
                    if ts_sec < begin_sec || ts_sec > end_sec {
                        return Ok((items, false));
                    }

                    // empty resource name indicates "fetch all"
                    if resource.is_empty() || resource == &item.resource {
                        items.push(item);
                    }

                    if prev_size + items.len() >= MAX_ITEM_AMOUNT {
                        return Ok((items, false));
                    }
                }
                Err(err) => {
                    logging::error!("DefaultMetricLogReader::read_metrics_one_file_by_end_time: {:?} Failed to convert to MetricItem. Error: {:?}.", line,err);
                    continue;
                }
            }
        }
    }
}

impl MetricLogReader for DefaultMetricLogReader {
    fn read_metrics(
        &self,
        name_list: Vec<PathBuf>,
        file_no: usize,
        start_offset: SeekFrom,
        max_lines: usize,
    ) -> Result<MetricItemVec> {
        if name_list.is_empty() {
            return Ok(Vec::new());
        }
        let mut file_no = file_no;
        // start_offset: the offset of the first file to read
        let (mut items, should_continue) =
            self.read_metrics_in_one_file(&name_list[file_no], start_offset, max_lines, 0, 0)?;
        if !should_continue {
            return Ok(items);
        }
        file_no += 1;
        // Continue reading until the size or time does not satisfy the condition
        loop {
            if file_no >= name_list.len() || items.len() >= max_lines {
                // No files to read.
                break;
            }
            let (arr, should_continue) = self.read_metrics_in_one_file(
                &name_list[file_no],
                SeekFrom::Start(0),
                max_lines,
                get_latest_second(&items),
                items.len(),
            )?;
            items.extend_from_slice(&arr);
            if !should_continue {
                break;
            }
            file_no += 1;
        }
        Ok(items)
    }

    fn read_metrics_by_end_time(
        &self,
        name_list: Vec<PathBuf>,
        file_no: usize,
        start_offset: SeekFrom,
        begin_ms: u64,
        end_ms: u64,
        resource: String,
    ) -> Result<MetricItemVec> {
        if name_list.is_empty() {
            return Ok(Vec::new());
        }
        let mut file_no = file_no;
        // start_offset: the offset of the first file to read
        let (mut items, should_continue) = self.read_metrics_one_file_by_end_time(
            &name_list[file_no],
            start_offset,
            begin_ms,
            end_ms,
            &resource,
            0,
        )?;
        if !should_continue {
            return Ok(items);
        }
        file_no += 1;
        // Continue reading until the size or time does not satisfy the condition
        loop {
            if file_no >= name_list.len() {
                // No files to read.
                break;
            }
            let (arr, should_continue) = self.read_metrics_one_file_by_end_time(
                &name_list[file_no],
                SeekFrom::Start(0),
                begin_ms,
                end_ms,
                &resource,
                items.len(),
            )?;
            items.extend_from_slice(&arr);
            if !should_continue {
                break;
            }
            file_no += 1;
        }
        Ok(items)
    }
}

fn get_latest_second(items: &MetricItemVec) -> u64 {
    if items.is_empty() {
        return 0;
    }
    items[items.len() - 1].timestamp / 1000
}

pub fn open_file_and_seek_to(filename: &PathBuf, offset: SeekFrom) -> Result<File> {
    let mut file = File::open(filename)?;
    // Set position to the offset recorded in the idx file
    file.seek(offset)?;
    Ok(file)
}
