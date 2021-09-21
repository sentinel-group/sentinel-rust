use super::{config, constant::*};
use crate::{
    base::{check_validity_for_reuse_statistic, constant::*, ResourceType},
    logging::Logger,
    Error, Result,
};
use serde::{Deserialize, Serialize};
use serde_json;
use std::fmt;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Debug)]
pub(super) struct AppConfig {
    // app_name represents the name of current running service.
    pub(super) app_name: String,
    // app_type indicates the resource_type of the service (e.g. web service, API gateway).
    pub(super) app_type: ResourceType,
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
pub(super) struct LogMetricConfig {
    pub(super) single_file_max_size: u64,
    pub(super) max_file_count: u32,
    pub(super) flush_interval_sec: u32,
}

impl Default for LogMetricConfig {
    fn default() -> Self {
        LogMetricConfig {
            single_file_max_size: SINGLE_FILE_MAX_SIZE,
            max_file_count: MAX_FILE_AMOUNT,
            flush_interval_sec: FLUSH_INTERVAL_SEC,
        }
    }
}

// LogConfig represent the configuration of logging in Sentinel.
#[derive(Serialize, Deserialize, Debug)]
pub(super) struct LogConfig {
    // logger indicates that using logger to replace default logging.
    pub(super) logger: Logger,
    // metric represents the configuration items of the metric log.
    pub(super) metric: LogMetricConfig,
}

impl Default for LogConfig {
    fn default() -> Self {
        LogConfig {
            logger: Logger::EnvLogger(DEFAULT_LOG_LEVEL.into()),
            metric: LogMetricConfig::default(),
        }
    }
}

// SystemStatConfig represents the configuration items of system statistic collector
#[derive(Serialize, Deserialize, Debug)]
pub(super) struct SystemStatConfig {
    // interval_ms represents the collecting interval of the system metrics collector.
    pub(super) system_interval_ms: u32,
    // load_interval_ms represents the collecting interval of the system load collector.
    pub(super) load_interval_ms: u32,
    // cpu_interval_ms represents the collecting interval of the system cpu usage collector.
    pub(super) cpu_interval_ms: u32,
    // memory_interval_ms represents the collecting interval of the system memory usage collector.
    pub(super) memory_interval_ms: u32,
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
pub(super) struct StatConfig {
    // sample_count_total and interval_ms_total is the per resource's global default statistic sliding window config
    pub(super) sample_count_total: u32,
    pub(super) interval_ms_total: u32,
    // sample_count and interval_ms is the per resource's default readonly metric statistic
    // This default readonly metric statistic must be reusable based on global statistic.
    pub(super) sample_count: u32,
    pub(super) interval_ms: u32,
    pub(super) system: SystemStatConfig,
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
pub(super) struct SentinelConfig {
    pub(super) app: AppConfig,
    pub(super) log: LogConfig,
    pub(super) stat: StatConfig,
    // use_cache_time indicates whether to cache time(ms), it is false by default
    pub(super) use_cache_time: bool,
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
    pub(super) version: String,
    pub(super) config: SentinelConfig,
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
        if self.version.len() == 0 {
            return Err(Error::msg("empty version"));
        }
        if self.config.app.app_name.len() == 0 {
            return Err(Error::msg("empty app name"));
        }
        if self.config.log.metric.max_file_count <= 0 {
            return Err(Error::msg(
                "illegal metric log configuration: max_file_count < 0",
            ));
        }
        if self.config.log.metric.single_file_max_size <= 0 {
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

    pub fn app_name(&self) -> &String {
        &self.config.app.app_name
    }

    pub fn app_type(&self) -> &ResourceType {
        &self.config.app.app_type
    }

    pub fn logger(&self) -> &Logger {
        &self.config.log.logger
    }

    pub fn metric_log_flush_interval_sec(&self) -> u32 {
        self.config.log.metric.flush_interval_sec
    }

    pub fn metric_log_single_file_max_size(&self) -> u64 {
        self.config.log.metric.single_file_max_size
    }

    pub fn metric_log_max_file_amount(&self) -> u32 {
        self.config.log.metric.max_file_count
    }

    pub fn system_stat_collect_interval_ms(&self) -> u32 {
        self.config.stat.system.system_interval_ms
    }

    pub fn load_stat_collec_interval_ms(&self) -> u32 {
        self.config.stat.system.load_interval_ms
    }

    pub fn cpu_stat_collec_interval_ms(&self) -> u32 {
        self.config.stat.system.cpu_interval_ms
    }

    pub fn memory_stat_collec_interval_ms(&self) -> u32 {
        self.config.stat.system.memory_interval_ms
    }

    pub fn use_cache_time(&self) -> bool {
        self.config.use_cache_time
    }

    pub fn global_stat_interval_ms_total(&self) -> u32 {
        self.config.stat.interval_ms_total
    }

    pub fn global_stat_sample_count_total(&self) -> u32 {
        self.config.stat.sample_count_total
    }

    pub fn metric_stat_interval_ms(&self) -> u32 {
        self.config.stat.interval_ms
    }

    pub fn metric_stat_sample_count(&self) -> u32 {
        self.config.stat.sample_count
    }
}

impl fmt::Display for ConfigEntity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let fmtted = serde_json::to_string_pretty(self).unwrap();
        write!(f, "{}", fmtted)
    }
}
