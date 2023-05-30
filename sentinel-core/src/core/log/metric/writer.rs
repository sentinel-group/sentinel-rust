use super::*;
use crate::{config, logging, utils, Error, Result};
use std::fs::{DirBuilder, File};
use std::io::{Seek, SeekFrom};
use std::{io::Write, sync::RwLock};

#[derive(Default)]
pub struct DefaultMetricLogWriter {
    base_dir: PathBuf,
    base_filename: PathBuf,
    max_single_size: u64,
    max_file_amount: usize,
    latest_op_sec: u64,
    cur_metric_file: Option<RwLock<File>>,
    cur_metric_idx_file: Option<RwLock<File>>,
}

impl MetricLogWriter for DefaultMetricLogWriter {
    fn write(&mut self, ts: u64, items: &mut Vec<MetricItem>) -> Result<()> {
        if items.is_empty() {
            return Ok(());
        }
        if ts == 0 {
            return Err(Error::msg(format!("Invalid timestamp: {}", ts)));
        }
        if self.cur_metric_file.is_none() || self.cur_metric_idx_file.is_none() {
            return Err(Error::msg("file handle not initialized".to_string()));
        }
        // Update all metric items to the given timestamp.
        for item in items.iter_mut() {
            item.timestamp = ts
        }

        let time_sec = ts / 1000;
        if time_sec < self.latest_op_sec {
            // ignore
            return Ok(());
        }
        if time_sec > self.latest_op_sec {
            let pos = self
                .cur_metric_file
                .as_ref()
                .unwrap()
                .write()
                .unwrap()
                .seek(SeekFrom::Current(0))?;
            self.write_index(time_sec, pos)?;
            if self.is_new_day(self.latest_op_sec, time_sec) {
                self.roll_to_next_file(ts)?;
            }
        }
        // Write and flush
        self.write_items_and_flush(items)?;
        self.roll_file_size_exceeded(ts)?;
        if time_sec > self.latest_op_sec {
            // Update the latest time_sec.
            self.latest_op_sec = time_sec;
        }

        Ok(())
    }
}

impl DefaultMetricLogWriter {
    fn write_items_and_flush(&self, items: &Vec<MetricItem>) -> Result<()> {
        let mut metric_out = self.cur_metric_file.as_ref().unwrap().write().unwrap();
        for item in items {
            // Append the LF line separator.
            let s = item.to_string() + "\n";
            metric_out.write_all(s.as_ref())?;
        }
        metric_out.flush()?;
        Ok(())
    }

    /// Check whether the file size is exceeded `config::SINGLE_FILE_MAX_SIZE`.
    /// If so, roll to the next file.
    fn roll_file_size_exceeded(&mut self, time: u64) -> Result<()> {
        if self.cur_metric_file.is_none() {
            return Ok(());
        }
        let file_len = self
            .cur_metric_file
            .as_ref()
            .unwrap()
            .read()
            .unwrap()
            .metadata()?
            .len();
        if file_len >= self.max_single_size {
            return self.roll_to_next_file(time);
        }
        Ok(())
    }

    /// Close last file and open a new one.
    fn roll_to_next_file(&mut self, time: u64) -> Result<()> {
        // pay attention, if the computation name of the next file is failed,
        // the old file won't be closed and metric logs would be append to the old file.
        // And it may also lead to failure when deleting deprecated metric logs, since it also depnds on this .
        let new_filename = self.next_file_name_of_time(time)?;
        self.close_cur_and_new_file(new_filename)
    }

    fn write_index(&self, time: u64, offset: u64) -> Result<()> {
        // Use BigEndian here to keep consistent with DataOutputStream in Java.
        let mut idx_out = self.cur_metric_idx_file.as_ref().unwrap().write().unwrap();
        idx_out.write_all(&time.to_be_bytes())?;
        idx_out.write_all(&offset.to_be_bytes())?;
        idx_out.flush()?;
        Ok(())
    }

    /// Remove the outdated metric log files and corresponding index files,
    /// incase that log files accumulate exceedng the `config::MAX_FILE_AMOUNT`.
    fn remove_deprecated_files(&self) -> Result<()> {
        let files = list_metric_files(&self.base_dir, &self.base_filename)?;
        if files.len() >= self.max_file_amount {
            let amount_to_remove = files.len() - self.max_file_amount + 1;
            for filename in files.iter().take(amount_to_remove) {
                let idx_filename = form_metric_idx_filename(filename.to_str().unwrap());
                match fs::remove_file(filename) {
                    Ok(_) => {
                        logging::info!("[MetricWriter] Metric log file removed in DefaultMetricLogWriter.remove_deprecated_files(), filename: {:?}", filename);
                    }
                    Err(err) => {
                        logging::error!("Failed to remove metric log file in DefaultMetricLogWriter::remove_deprecated_files(), filename: {:?}, error: {:?}", filename, err);
                    }
                }
                match fs::remove_file(idx_filename) {
                    Ok(_) => {
                        logging::info!("[MetricWriter] Metric index file removed in DefaultMetricLogWriter.remove_deprecated_files(), filename: {:?}", filename);
                    }
                    Err(err) => {
                        logging::error!("Failed to remove metric index log file in DefaultMetricLogWriter::remove_deprecated_files(), filename: {:?}, error: {:?}", filename, err);
                    }
                }
            }
        }
        Ok(())
    }

    /// Compute the next file name of the given time. Find the lastest file with the same prefix pattern and add increase the order.
    /// And never use `fmt::Debug` to print the file name (either `String/&str` or `PathBuf/&Path`), since it will contain `\"`.
    fn next_file_name_of_time(&self, time: u64) -> Result<String> {
        let date_str = utils::format_date(time);
        let file_pattern = self.base_filename.to_str().unwrap().to_owned() + "." + &date_str;
        let list = list_metric_files_conditional(
            &self.base_dir,
            &PathBuf::from(&file_pattern),
            |filename: &str, p: &str| -> bool { filename.contains(p) },
        )?;
        if list.is_empty() {
            return Ok(self.base_dir.to_str().unwrap().to_owned() + &file_pattern);
        }
        // Find files with the same prefix pattern, have to add the order to separate files.
        let last = &list[list.len() - 1];
        let mut n = 0;
        let items = last.to_str().unwrap().split('.').collect::<Vec<&str>>();
        if !items.is_empty() {
            n = str::parse::<u32>(items[items.len() - 1]).unwrap_or(0);
        }
        return Ok(format!(
            "{}{}.{}",
            self.base_dir.to_str().unwrap().to_owned(),
            file_pattern,
            n + 1
        ));
    }

    fn close_cur_and_new_file(&mut self, filename: String) -> Result<()> {
        self.remove_deprecated_files()?;

        if self.cur_metric_file.is_some() {
            self.cur_metric_file.take();
        }
        if self.cur_metric_idx_file.is_some() {
            self.cur_metric_idx_file.take();
        }
        // Create new metric log file, whether it exists or not.
        let mf = fs::File::create(&filename)?;
        logging::info!(
            "[MetricWriter] New metric log file created, filename {:?}",
            filename
        );

        let idx_file = form_metric_idx_filename(&filename);
        let mif = fs::File::create(&idx_file)?;
        logging::info!(
            "[MetricWriter] New metric log index file created, idx_file {:?}",
            idx_file
        );

        self.cur_metric_file = Some(RwLock::new(mf));
        self.cur_metric_idx_file = Some(RwLock::new(mif));

        Ok(())
    }

    fn initialize(&mut self) -> Result<()> {
        // Create the dir if not exists.
        DirBuilder::new().recursive(true).create(&self.base_dir)?;
        if self.cur_metric_file.is_some() {
            return Ok(());
        }
        let ts = utils::curr_time_millis();
        self.roll_to_next_file(ts)?;
        self.latest_op_sec = ts / 1000;
        Ok(())
    }

    fn is_new_day(&self, last_sec: u64, sec: u64) -> bool {
        sec / 86400 > last_sec / 86400
    }

    fn new_of_app(
        max_single_size: u64,
        max_file_amount: usize,
        app_name: String,
    ) -> Result<DefaultMetricLogWriter> {
        if max_single_size == 0 || max_file_amount == 0 {
            return Err(Error::msg("invalid max_size or max_file_amount"));
        }
        let base_dir = PathBuf::from(config::log_metrc_dir());
        let base_filename = form_metric_filename(&app_name, config::log_metrc_pid()).into();

        let mut writer = DefaultMetricLogWriter {
            base_dir,
            base_filename,
            max_single_size,
            max_file_amount,
            latest_op_sec: 0,
            ..Default::default()
        };
        writer.initialize()?;
        Ok(writer)
    }

    pub fn new(max_size: u64, max_file_amount: usize) -> Result<DefaultMetricLogWriter> {
        Self::new_of_app(max_size, max_file_amount, config::app_name())
    }
}
