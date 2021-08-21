use crate::base::ResourceType;

// default app settings
pub const SENTINEL_VERSION: &str = "v1";
pub const DEFAULT_APP_NAME: &str = "unknown_service";
pub const DEFAULT_APP_TYPE: u8 = ResourceType::Common as _;
pub const APP_NAME_ENV_KEY: &str = "SENTINEL_APP_NAME";
pub const APP_TYPE_ENV_KEY: &str = "SENTINEL_APP_TYPE";
pub const CONF_FILE_PATH_ENV_KEY: &str = "SENTINEL_CONFIG_FILE_PATH";
pub const CONFIG_FILENAME: &str = "sentinel.yml";

// default metric log settings
pub const FLUSH_INTERVAL_SEC: u32 = 1;
pub const SINGLE_FILE_MAX_SIZE: u64 = 1024 * 1024 * 50;
pub const MAX_FILE_AMOUNT: u32 = 8;

// default statistic settings
pub const SYSTEM_INTERVAL_MS: u32 = 1000;
pub const LOAD_INTERVAL_MS: u32 = 1000;
pub const CPU_INTERVAL_MS: u32 = 1000;
pub const MEMORY_INTERVAL_MS: u32 = 150;
pub const WARM_UP_COLD_FACTOR: u32 = 3;
