use super::*;
use crate::{logging, Error, Result};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::sync::Mutex;
#[derive(Debug)]
pub struct FilePosition {
    pub metric_filename: PathBuf,
    pub idx_filename: PathBuf,
    pub cur_offset_in_idx: SeekFrom,
    pub cur_sec_in_idx: u64,
    // todo: cache the idx file handle here?
}

impl Default for FilePosition {
    fn default() -> Self {
        FilePosition {
            metric_filename: PathBuf::default(),
            idx_filename: PathBuf::default(),
            cur_offset_in_idx: SeekFrom::Start(0),
            cur_sec_in_idx: 0,
        }
    }
}

pub struct DefaultMetricSearcher {
    pub reader: DefaultMetricLogReader,
    pub base_dir: PathBuf,
    pub base_filename: PathBuf,
    pub cached_pos: Mutex<FilePosition>,
}

impl DefaultMetricSearcher {
    pub fn new(base_dir: String, base_filename: String) -> Result<Self> {
        if base_dir.is_empty() {
            return Err(Error::msg("empty base directory"));
        }
        if base_filename.is_empty() {
            return Err(Error::msg("empty base filename pattern"));
        }
        let reader = DefaultMetricLogReader::new();
        Ok(DefaultMetricSearcher {
            base_dir: PathBuf::from(base_dir),
            base_filename: PathBuf::from(base_filename),
            reader,
            cached_pos: Mutex::new(FilePosition::default()),
        })
    }

    pub fn find_by_time_and_resource(
        &self,
        begin_time_ms: u64,
        end_time_ms: u64,
        resource: String,
    ) -> Result<MetricItemVec> {
        self.search_offset_and_read(begin_time_ms, &move |filenames: Vec<PathBuf>,
                                                          file_no: usize,
                                                          offset: SeekFrom|
              -> Result<MetricItemVec> {
            self.reader.read_metrics_by_end_time(
                filenames,
                file_no,
                offset,
                begin_time_ms,
                end_time_ms,
                resource.clone(),
            )
        })
    }

    pub fn find_from_time_with_max_lines(
        &self,
        begin_time_ms: u64,
        max_lines: usize,
    ) -> Result<MetricItemVec> {
        self.search_offset_and_read(begin_time_ms, &|filenames: Vec<PathBuf>,
                                                     file_no: usize,
                                                     offset: SeekFrom|
         -> Result<MetricItemVec> {
            self.reader
                .read_metrics(filenames, file_no, offset, max_lines)
        })
    }

    pub fn search_offset_and_read(
        &self,
        begin_time_ms: u64,
        do_read: &dyn Fn(Vec<PathBuf>, usize, SeekFrom) -> Result<MetricItemVec>,
    ) -> Result<MetricItemVec> {
        let filenames = list_metric_files(&self.base_dir, &self.base_filename)?;
        // Try to position the latest file index and offset from the cache (fast-path).
        // If cache is not up-to-date, we'll read from the initial position (offset 0 of the first file).
        let (offset_start, file_no) =
            self.get_offset_start_and_file_idx(&filenames, begin_time_ms)?;
        for i in file_no..filenames.len() {
            let filename = &filenames[i];
            // Retrieve the start offset that is valid for given condition.
            // If offset = -1, it indicates that current file (i) does not satisfy the condition.
            let offset =
                self.find_offset_to_start(filename.to_str().unwrap(), begin_time_ms, offset_start);
            match offset {
                Ok(offset) => {
                    // Read metric items from the offset of current file (number i).
                    return do_read(filenames, i, SeekFrom::Start(offset as u64));
                }
                Err(err) => {
                    logging::warn!("[search_offset_and_read] Failed to find_offset_to_start, will try next file, begin_time_ms: {}, filename: {:?}, offset_start: {:?}, err: {:?}", begin_time_ms, filename, offset_start, err);
                }
            }
        }
        Ok(Vec::new())
    }

    fn get_offset_start_and_file_idx(
        &self,
        filenames: &[PathBuf],
        begin_time_ms: u64,
    ) -> Result<(SeekFrom, usize)> {
        let cache_ok = self.is_position_in_time_for(begin_time_ms)?;
        let mut i = 0;
        let mut offset_in_idx = SeekFrom::Start(0);
        let cached_pos = self.cached_pos.lock().unwrap();
        if cache_ok {
            for (j, v) in filenames.iter().enumerate() {
                if v != &cached_pos.metric_filename {
                    i = j;
                    offset_in_idx = cached_pos.cur_offset_in_idx;
                    break;
                }
            }
        }
        Ok((offset_in_idx, i))
    }

    fn find_offset_to_start(
        &self,
        filename: &str,
        begin_time_ms: u64,
        last_pos: SeekFrom,
    ) -> Result<u32> {
        let mut cached_pos = self.cached_pos.lock().unwrap();
        cached_pos.idx_filename = "".into();
        cached_pos.metric_filename = "".into();

        let idx_filename = form_metric_idx_filename(filename);
        let begin_sec = begin_time_ms / 1000;
        let mut file = File::open(&idx_filename)?;

        // Set position to the offset recorded in the idx file
        cached_pos.cur_offset_in_idx = SeekFrom::Start(file.seek(last_pos)?);
        let mut sec: u64;
        loop {
            let mut buffer: [u8; 8] = [0; 8];
            file.read_exact(&mut buffer)?;
            sec = u64::from_be_bytes(buffer);
            if sec >= begin_sec {
                break;
            }
            let mut buffer: [u8; 4] = [0; 4];
            file.read_exact(&mut buffer)?;
            cached_pos.cur_offset_in_idx = SeekFrom::Start(file.seek(SeekFrom::Current(0))?);
        }
        let mut buffer: [u8; 4] = [0; 4];
        file.read_exact(&mut buffer)?;
        let offset = u32::from_be_bytes(buffer);
        // Cache the idx filename and position
        cached_pos.metric_filename = filename.into();
        cached_pos.idx_filename = idx_filename.into();
        cached_pos.cur_sec_in_idx = sec;
        Ok(offset)
    }

    fn is_position_in_time_for(&self, begin_time_ms: u64) -> Result<bool> {
        let cached_pos = self.cached_pos.lock().unwrap();
        if begin_time_ms / 1000 < cached_pos.cur_sec_in_idx {
            return Ok(false);
        }
        let idx_filename = &cached_pos.idx_filename;
        if idx_filename == &PathBuf::from("") {
            return Ok(false);
        }
        let mut idx_file = open_file_and_seek_to(idx_filename, cached_pos.cur_offset_in_idx)?;

        let mut buffer: [u8; 8] = [0; 8];
        idx_file.read_exact(&mut buffer)?;
        let sec = u64::from_be_bytes(buffer);

        Ok(sec == cached_pos.cur_sec_in_idx)
    }
}
