use super::constant::*;
use crate::{
    base::{check_validity_for_reuse_statistic, constant::*, ResourceType},
    Error, Result,
};
use serde::{Deserialize, Serialize};
use serde_json;
use std::fmt;

#[derive(Serialize, Deserialize, Debug)]
pub struct AppConfig {
    // app_name represents the name of current running service.
    pub app_name: String,
    // app_type indicates the resource_type of the service (e.g. web service, API gateway).
    pub app_type: ResourceType,
}

impl Default for AppConfig {
    fn default() -> Self {
        AppConfig {
            app_name: DEFAULT_APP_NAME.into(),
            app_type: DEFAULT_APP_TYPE.into(),
        }
    }
}

// LogMetricConfig represents the configuration items of the metric log.
#[derive(Serialize, Deserialize, Debug)]
pub struct LogMetricConfig {
    pub use_pid: bool,
    pub dir: String,
    pub single_file_max_size: u64,
    pub max_file_count: usize,
    pub flush_interval_sec: u32,
}

impl Default for LogMetricConfig {
    fn default() -> Self {
        LogMetricConfig {
            use_pid: true,
            dir: LOG_METRICS_DIR.into(),
            single_file_max_size: SINGLE_FILE_MAX_SIZE,
            max_file_count: MAX_FILE_AMOUNT,
            flush_interval_sec: FLUSH_INTERVAL_SEC,
        }
    }
}

// ExporterConfig represents exporter settings
#[derive(Serialize, Deserialize, Debug)]
pub struct ExporterConfig {
    pub addr: String,
    pub metrics_path: String,
}

impl Default for ExporterConfig {
    fn default() -> Self {
        ExporterConfig {
            addr: EXPORTER_ADDR.into(),
            metrics_path: EXPORTER_METRICS_PATH.into(),
        }
    }
}

// LogConfig represent the configuration of logging in Sentinel.
#[derive(Serialize, Deserialize, Debug)]
pub struct LogConfig {
    // metric represents the configuration items of the metric log.
    pub metric: LogMetricConfig,
    pub exporter: ExporterConfig,
    pub config_file: String,
}

impl Default for LogConfig {
    fn default() -> Self {
        LogConfig {
            metric: LogMetricConfig::default(),
            exporter: ExporterConfig::default(),
            config_file: LOG_CONFIG_FILE.into(),
        }
    }
}

// SystemStatConfig represents the configuration items of system statistic collector
#[derive(Serialize, Deserialize, Debug)]
pub struct SystemStatConfig {
    // interval_ms represents the collecting interval of the system metrics collector.
    pub system_interval_ms: u32,
    // load_interval_ms represents the collecting interval of the system load collector.
    pub load_interval_ms: u32,
    // cpu_interval_ms represents the collecting interval of the system cpu usage collector.
    pub cpu_interval_ms: u32,
    // memory_interval_ms represents the collecting interval of the system memory usage collector.
    pub memory_interval_ms: u32,
}

impl Default for SystemStatConfig {
    fn default() -> Self {
        SystemStatConfig {
            system_interval_ms: SYSTEM_INTERVAL_MS,
            load_interval_ms: LOAD_INTERVAL_MS,
            cpu_interval_ms: CPU_INTERVAL_MS,
            memory_interval_ms: MEMORY_INTERVAL_MS,
        }
    }
}

// StatConfig represents configuration items related to statistics.
#[derive(Serialize, Deserialize, Debug)]
pub struct StatConfig {
    // sample_count_total and interval_ms_total is the per resource's global default statistic sliding window config
    pub sample_count_total: u32,
    pub interval_ms_total: u32,
    // sample_count and interval_ms is the per resource's default readonly metric statistic
    // This default readonly metric statistic must be reusable based on global statistic.
    pub sample_count: u32,
    pub interval_ms: u32,
    pub system: SystemStatConfig,
}

impl Default for StatConfig {
    fn default() -> Self {
        StatConfig {
            sample_count_total: DEFAULT_SAMPLE_COUNT_TOTAL,
            interval_ms_total: DEFAULT_INTERVAL_MS_TOTAL,
            sample_count: DEFAULT_SAMPLE_COUNT,
            interval_ms: DEFAULT_INTERVAL_MS,
            system: SystemStatConfig::default(),
        }
    }
}

// SentinelConfig represent the general configuration of Sentinel.
#[derive(Serialize, Deserialize, Debug)]
pub struct SentinelConfig {
    pub app: AppConfig,
    pub log: LogConfig,
    pub stat: StatConfig,
    // use_cache_time indicates whether to cache time(ms), it is false by default
    pub use_cache_time: bool,
}

impl Default for SentinelConfig {
    fn default() -> Self {
        SentinelConfig {
            use_cache_time: true,
            app: AppConfig::default(),
            log: LogConfig::default(),
            stat: StatConfig::default(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ConfigEntity {
    pub version: String,
    pub config: SentinelConfig,
}

impl Default for ConfigEntity {
    fn default() -> Self {
        ConfigEntity {
            version: SENTINEL_VERSION.into(),
            config: SentinelConfig::default(),
        }
    }
}

impl ConfigEntity {
    pub fn new() -> Self {
        ConfigEntity::default()
    }

    pub fn check(&self) -> Result<()> {
        if self.version.is_empty() {
            return Err(Error::msg("empty version"));
        }
        if self.config.app.app_name.is_empty() {
            return Err(Error::msg("empty app name"));
        }
        if self.config.log.metric.max_file_count == 0 {
            return Err(Error::msg(
                "illegal metric log configuration: max_file_count < 0",
            ));
        }
        if self.config.log.metric.single_file_max_size == 0 {
            return Err(Error::msg(
                "illegal metric log configuration: single_file_max_size < 0",
            ));
        }
        check_validity_for_reuse_statistic(
            self.config.stat.sample_count,
            self.config.stat.interval_ms,
            self.config.stat.sample_count_total,
            self.config.stat.interval_ms_total,
        )?;
        Ok(())
    }
}

impl fmt::Display for ConfigEntity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let fmtted = serde_json::to_string_pretty(self).unwrap();
        write!(f, "{}", fmtted)
    }
}
