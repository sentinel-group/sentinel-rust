use crate::base::ResourceType;

// default app settings
pub const SENTINEL_VERSION: &str = "v1";
pub const DEFAULT_APP_NAME: &str = "unknown_service";
pub const DEFAULT_APP_TYPE: u8 = ResourceType::Common as _;
pub const APP_NAME_ENV_KEY: &str = "SENTINEL_APP_NAME";
pub const APP_TYPE_ENV_KEY: &str = "SENTINEL_APP_TYPE";
pub const CONF_FILE_PATH_ENV_KEY: &str = "SENTINEL_CONFIG_FILE_PATH";
pub const CONFIG_FILENAME: &str = "USE_DEFAULT_CONFIGURATION";

// default metric log settings
pub const FLUSH_INTERVAL_SEC: u32 = 1;
pub const SINGLE_FILE_MAX_SIZE: u64 = 100; // 1024 * 1024 * 50;
pub const MAX_FILE_AMOUNT: usize = 2; //8;
pub const EXPORTER_ADDR: &str = "127.0.0.1:9091";
pub const EXPORTER_METRICS_PATH: &str = "/metrics";

// default statistic settings
pub const SYSTEM_INTERVAL_MS: u32 = 1000;
pub const LOAD_INTERVAL_MS: u32 = 1000;
pub const CPU_INTERVAL_MS: u32 = 1000;
pub const MEMORY_INTERVAL_MS: u32 = 150;
pub const WARM_UP_COLD_FACTOR: u32 = 3;

// default log settings
pub const DEFAULT_LOG_LEVEL: &str = "warn";
pub const LOG_CONFIG_FILE: &str = "testdata/config/log4rs.yaml";
pub const LOG_METRICS_DIR: &str = "logs/sentinel/";
