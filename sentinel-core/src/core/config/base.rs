use super::{constant::*, ConfigEntity};
use crate::{base::ResourceType, logging, utils, Error, Result};
use serde_yaml;
use std::cell::RefCell;
use std::env;
use std::fs::File;
use std::io::prelude::*;
use std::path::Path;

thread_local! {
    static GLOBAL_CONFIG : RefCell<ConfigEntity> = RefCell::new(ConfigEntity::new());
}

pub fn reset_global_config(entity: ConfigEntity) {
    GLOBAL_CONFIG.with(|c| {
        *c.borrow_mut() = entity;
    });
}

// init_config_with_yaml loads general configuration from the YAML file under provided path.
pub fn init_config_with_yaml(config_path: &mut String) -> Result<()> {
    // Initialize general config and logging module.
    apply_yaml_config_file(config_path)?;
    override_items_from_system_env()?;
    #[cfg(any(feature = "env_logger", feature = "log4rs"))]
    init_log()?;
    Ok(())
}

// apply_yaml_config_file loads general configuration from the given YAML file.
fn apply_yaml_config_file(config_path: &mut String) -> Result<()> {
    // Priority: system environment > YAML file > default config
    if utils::is_blank(config_path) {
        // If the config file path is absent, Sentinel will try to resolve it from the system env.
        *config_path = env::var(CONF_FILE_PATH_ENV_KEY).unwrap_or_else(|_| CONFIG_FILENAME.into());
    }
    // First Sentinel will try to load config from the given file.
    // If the path is empty (not set), Sentinel will use the default config.
    load_global_config_from_yaml_file(config_path)?;
    Ok(())
}

fn load_global_config_from_yaml_file(path_str: &String) -> Result<()> {
    let path = Path::new(path_str);
    if path_str == CONFIG_FILENAME {
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

fn override_items_from_system_env() -> Result<()> {
    let app_name = env::var(APP_NAME_ENV_KEY).unwrap_or_else(|_| DEFAULT_APP_NAME.into());
    let app_type: ResourceType = env::var(APP_TYPE_ENV_KEY)
        .unwrap_or(format!("{}", DEFAULT_APP_TYPE))
        .parse::<u8>()
        .unwrap_or(DEFAULT_APP_TYPE)
        .into();

    GLOBAL_CONFIG
        .try_with(|c| -> Result<()> {
            let mut cfg = c.borrow_mut();
            if !utils::is_blank(&app_name) {
                cfg.config.app.app_name = app_name;
            }
            cfg.config.app.app_type = app_type;
            cfg.check()?;
            Ok(())
        })
        .unwrap()?;
    Ok(())
}

#[cfg(any(feature = "env_logger", feature = "log4rs"))]
pub fn init_log() -> Result<()> {
    // Then Sentinel will try to get fundamental config items from system environment.
    // If present, the value in system env will override the value in config file.
    logging::logger_init(log_config_file());

    logging::info!("[Config] App name resolved, appName {}", app_name());
    GLOBAL_CONFIG
        .try_with(|c| {
            logging::info!(
                "[Config] Print effective global config, globalConfig {:?}",
                c.borrow()
            );
        })
        .unwrap();

    Ok(())
}

#[inline]
pub fn log_config_file() -> Option<String> {
    GLOBAL_CONFIG
        .try_with(|c| c.borrow().config.log.config_file.clone())
        .ok()
}

#[inline]
pub fn log_metrc_dir() -> String {
    GLOBAL_CONFIG
        .try_with(|c| c.borrow().config.log.metric.dir.clone())
        .unwrap()
}

#[inline]
pub fn log_metrc_pid() -> bool {
    GLOBAL_CONFIG
        .try_with(|c| c.borrow().config.log.metric.use_pid)
        .unwrap()
}

#[inline]
pub fn app_name() -> String {
    GLOBAL_CONFIG
        .try_with(|c| c.borrow().config.app.app_name.clone())
        .unwrap()
}

#[inline]
pub fn app_type() -> ResourceType {
    GLOBAL_CONFIG
        .try_with(|c| c.borrow().config.app.app_type)
        .unwrap()
}

#[inline]
pub fn exporter_addr() -> String {
    GLOBAL_CONFIG
        .try_with(|c| c.borrow().config.log.exporter.addr.clone())
        .unwrap()
}

#[inline]
pub fn exporter_metrics_path() -> String {
    GLOBAL_CONFIG
        .try_with(|c| c.borrow().config.log.exporter.metrics_path.clone())
        .unwrap()
}

#[inline]
pub fn metric_log_flush_interval_sec() -> u32 {
    GLOBAL_CONFIG
        .try_with(|c| c.borrow().config.log.metric.flush_interval_sec)
        .unwrap()
}

#[inline]
pub fn metric_log_single_file_max_size() -> u64 {
    GLOBAL_CONFIG
        .try_with(|c| c.borrow().config.log.metric.single_file_max_size)
        .unwrap()
}

#[inline]
pub fn metric_log_max_file_amount() -> usize {
    GLOBAL_CONFIG
        .try_with(|c| c.borrow().config.log.metric.max_file_count)
        .unwrap()
}

#[inline]
pub fn system_stat_collect_interval_ms() -> u32 {
    GLOBAL_CONFIG
        .try_with(|c| c.borrow().config.stat.system.system_interval_ms)
        .unwrap()
}

#[inline]
pub fn load_stat_collec_interval_ms() -> u32 {
    GLOBAL_CONFIG
        .try_with(|c| c.borrow().config.stat.system.load_interval_ms)
        .unwrap()
}

#[inline]
pub fn cpu_stat_collec_interval_ms() -> u32 {
    GLOBAL_CONFIG
        .try_with(|c| c.borrow().config.stat.system.cpu_interval_ms)
        .unwrap()
}

#[inline]
pub fn memory_stat_collec_interval_ms() -> u32 {
    GLOBAL_CONFIG
        .try_with(|c| c.borrow().config.stat.system.memory_interval_ms)
        .unwrap()
}

#[inline]
pub fn use_cache_time() -> bool {
    GLOBAL_CONFIG
        .try_with(|c| c.borrow().config.use_cache_time)
        .unwrap()
}

#[inline]
pub fn global_stat_interval_ms_total() -> u32 {
    GLOBAL_CONFIG
        .try_with(|c| c.borrow().config.stat.interval_ms_total)
        .unwrap()
}

#[inline]
pub fn global_stat_sample_count_total() -> u32 {
    GLOBAL_CONFIG
        .try_with(|c| c.borrow().config.stat.sample_count_total)
        .unwrap()
}

#[inline]
pub fn global_stat_bucket_length_ms() -> u32 {
    GLOBAL_CONFIG
        .try_with(|_c| global_stat_interval_ms_total() / global_stat_sample_count_total())
        .unwrap()
}

#[inline]
pub fn metric_stat_interval_ms() -> u32 {
    GLOBAL_CONFIG
        .try_with(|c| c.borrow().config.stat.interval_ms)
        .unwrap()
}

#[inline]
pub fn metric_stat_sample_count() -> u32 {
    GLOBAL_CONFIG
        .try_with(|c| c.borrow().config.stat.sample_count)
        .unwrap()
}
