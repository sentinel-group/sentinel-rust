use super::{constant::*, ConfigEntity};
use crate::{base::ResourceType, logging, utils, Error, Result};
use directories::UserDirs;
use lazy_static::lazy_static;
use serde_yaml;
use std::env;
use std::fs::File;
use std::io::prelude::*;
use std::path::Path;
use std::sync::RwLock;

lazy_static! {
    static ref GLOBAL_CONFIG: RwLock<ConfigEntity> = RwLock::new(ConfigEntity::new());
}

pub fn reset_global_config(entity: ConfigEntity) {
    let mut cfg = GLOBAL_CONFIG.write().unwrap();
    *cfg = entity;
}

// init_config_with_yaml loads general configuration from the YAML file under provided path.
pub fn init_config_with_yaml(config_path: &mut String) -> Result<()> {
    // Initialize general config and logging module.
    apply_yaml_config_file(config_path)?;
    override_config_from_env_and_init_log()?;
    Ok(())
}

// apply_yaml_config_file loads general configuration from the given YAML file.
fn apply_yaml_config_file(config_path: &mut String) -> Result<()> {
    // Priority: system environment > YAML file > default config
    if utils::is_blank(&config_path) {
        // If the config file path is absent, Sentinel will try to resolve it from the system env.
        *config_path = env::var(CONF_FILE_PATH_ENV_KEY).unwrap_or(CONFIG_FILENAME.into());
    }
    // First Sentinel will try to load config from the given file.
    // If the path is empty (not set), Sentinel will use the default config.
    load_global_config_from_yaml_file(&config_path)?;
    Ok(())
}

fn load_global_config_from_yaml_file(path_str: &String) -> Result<()> {
    let path = Path::new(path_str);
    if path_str == CONFIG_FILENAME && path.exists() {
        //use default globalCfg.
        return Ok(());
    }
    if !path.exists() {
        return Err(Error::msg(
            "Sentinel YAML configuration file does not exist!",
        ));
    }
    let mut file = File::open(path)?;
    let mut content = String::new();
    file.read_to_string(&mut content)?;
    let entity: ConfigEntity = serde_yaml::from_str(&content)?;
    entity.check()?;
    logging::info!(
        "[Config] Resolving Sentinel config from file, file {}",
        path_str
    );
    reset_global_config(entity);
    Ok(())
}

pub fn override_config_from_env_and_init_log() -> Result<()> {
    // Then Sentinel will try to get fundamental config items from system environment.
    // If present, the value in system env will override the value in config file.
    override_items_from_system_env()?;

    let config_logger = logger();
    logging::logger_init(config_logger);
    logging::info!("[Config] App name resolved, appName {}", app_name());
    logging::info!(
        "[Config] Print effective global config, globalConfig {}",
        GLOBAL_CONFIG.read().unwrap()
    );

    Ok(())
}

fn override_items_from_system_env() -> Result<()> {
    let mut cfg = GLOBAL_CONFIG.write().unwrap();
    let app_name = env::var(APP_NAME_ENV_KEY).unwrap_or(DEFAULT_APP_NAME.into());
    let app_type: ResourceType = env::var(APP_TYPE_ENV_KEY)
        .unwrap_or(format!("{}", DEFAULT_APP_TYPE))
        .parse::<u8>()
        .unwrap_or(DEFAULT_APP_TYPE)
        .into();

    if !utils::is_blank(&app_name) {
        cfg.config.app.app_name = app_name;
    }
    cfg.config.app.app_type = app_type;
    cfg.check()?;
    Ok(())
}

#[inline]
pub fn app_name() -> String {
    let cfg = GLOBAL_CONFIG.read().unwrap();
    cfg.app_name().clone()
}

#[inline]
pub fn app_type() -> ResourceType {
    let cfg = GLOBAL_CONFIG.read().unwrap();
    cfg.app_type().clone()
}

#[inline]
pub fn logger() -> logging::Logger {
    let cfg = GLOBAL_CONFIG.read().unwrap();
    cfg.logger().clone()
}

#[inline]
pub fn metric_log_flush_interval_sec() -> u32 {
    let cfg = GLOBAL_CONFIG.read().unwrap();
    cfg.metric_log_flush_interval_sec()
}

#[inline]
pub fn metric_log_single_file_max_size() -> u64 {
    let cfg = GLOBAL_CONFIG.read().unwrap();
    cfg.metric_log_single_file_max_size()
}

#[inline]
pub fn metric_log_max_file_amount() -> u32 {
    let cfg = GLOBAL_CONFIG.read().unwrap();
    cfg.metric_log_max_file_amount()
}

#[inline]
pub fn system_stat_collect_interval_ms() -> u32 {
    let cfg = GLOBAL_CONFIG.read().unwrap();
    cfg.system_stat_collect_interval_ms()
}

#[inline]
pub fn load_stat_collec_interval_ms() -> u32 {
    let cfg = GLOBAL_CONFIG.read().unwrap();
    cfg.load_stat_collec_interval_ms()
}

#[inline]
pub fn cpu_stat_collec_interval_ms() -> u32 {
    let cfg = GLOBAL_CONFIG.read().unwrap();
    cfg.cpu_stat_collec_interval_ms()
}

#[inline]
pub fn memory_stat_collec_interval_ms() -> u32 {
    let cfg = GLOBAL_CONFIG.read().unwrap();
    cfg.memory_stat_collec_interval_ms()
}

#[inline]
pub fn use_cache_time() -> bool {
    let cfg = GLOBAL_CONFIG.read().unwrap();
    cfg.use_cache_time()
}

#[inline]
pub fn global_stat_interval_ms_total() -> u32 {
    let cfg = GLOBAL_CONFIG.read().unwrap();
    cfg.global_stat_interval_ms_total()
}

#[inline]
pub fn global_stat_sample_count_total() -> u32 {
    let cfg = GLOBAL_CONFIG.read().unwrap();
    cfg.global_stat_sample_count_total()
}

#[inline]
pub fn global_stat_bucket_length_ms() -> u32 {
    let cfg = GLOBAL_CONFIG.read().unwrap();
    cfg.global_stat_interval_ms_total() / cfg.global_stat_sample_count_total()
}

#[inline]
pub fn metric_stat_interval_ms() -> u32 {
    let cfg = GLOBAL_CONFIG.read().unwrap();
    cfg.metric_stat_interval_ms()
}

#[inline]
pub fn metric_stat_sample_count() -> u32 {
    let cfg = GLOBAL_CONFIG.read().unwrap();
    cfg.metric_stat_sample_count()
}
