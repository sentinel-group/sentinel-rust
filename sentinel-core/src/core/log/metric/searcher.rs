use byteorder::{BigEndian, ReadBytesExt};

use super::*;
use crate::{logging, Error, Result};
use std::fs::File;
use std::io::{Cursor, Read, Seek, SeekFrom};
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

impl MetricSearcher for DefaultMetricSearcher {
    fn find_by_time_and_resource(
        &self,
        begin_time_ms: u64,
        end_time_ms: u64,
        resource: &str,
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
                resource.to_owned(),
            )
        })
    }

    fn find_from_time_with_max_lines(
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
    ) -> Result<u64> {
        let idx_filename = form_metric_idx_filename(filename);
        let begin_sec = begin_time_ms / 1000;
        let mut file = File::open(&idx_filename)?;

        let mut cached_pos = self.cached_pos.lock().unwrap();

        // Seek to the last position
        file.seek(last_pos)?;

        let mut index_data = Vec::new();
        file.read_to_end(&mut index_data)?;

        let mut offset = 0;
        let mut sec = 0;

        let mut reader = Cursor::new(index_data);
        while let Ok(sec_be) = ReadBytesExt::read_u64::<BigEndian>(&mut reader) {
            sec = sec_be;
            let offset_be = ReadBytesExt::read_u64::<BigEndian>(&mut reader)?;
            offset = offset_be;
            if sec >= begin_sec {
                break;
            }
        }

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
