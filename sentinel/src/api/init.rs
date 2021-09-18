//! Initialization func initialize the Sentinel's runtime environment, including:
//! 1. override global config, from manually config or yaml file or env variable
//! 2. initialize global logger
//! 3. initiate core component async task, including: metric log, system statistic...

use super::{config, config::ConfigEntity};
use crate::{log::metric, system_metric, utils, Error, Result};
use serde_yaml;

/// `init_default` initializes Sentinel using the configuration from system
/// environment and the default value.
#[inline]
pub fn init_default() -> Result<()> {
    init_sentinel(&mut String::new())
}

/// `init_with_config` initializes Sentinel using given config.
#[inline]
pub fn init_with_config(config_entity: ConfigEntity) -> Result<()> {
    config_entity.check()?;
    config::reset_global_config(config_entity);
    config::override_config_from_env_and_init_log()?;
    init_core_compoents()
}

/// Init loads Sentinel general configuration from the given YAML file
/// and initializes Sentinel.
#[inline]
pub fn init_with_config_file(config_path: &mut String) -> Result<()> {
    init_sentinel(config_path)
}

#[inline]
fn init_sentinel(config_path: &mut String) -> Result<()> {
    // Initialize general config and logging module.
    if config_path.len() > 0 {
        config::init_config_with_yaml(config_path)?;
    }
    init_core_compoents()
}

// `init_core_compoents` init core components with global config
#[inline]
fn init_core_compoents() -> Result<()> {
    if config::metric_log_flush_interval_sec() > 0 {
        metric::init_task()?;
    }

    let system_interval = config::system_stat_collect_interval_ms();
    let mut load_interval = system_interval;
    let mut cpu_interval = system_interval;
    let mut mem_interval = system_interval;

    if config::load_stat_collec_interval_ms() > 0 {
        load_interval = config::load_stat_collec_interval_ms();
    }
    if config::cpu_stat_collec_interval_ms() > 0 {
        cpu_interval = config::cpu_stat_collec_interval_ms();
    }
    if config::memory_stat_collec_interval_ms() > 0 {
        mem_interval = config::memory_stat_collec_interval_ms();
    }

    if load_interval > 0 {
        system_metric::init_load_collector(load_interval);
    }
    if cpu_interval > 0 {
        system_metric::init_cpu_collector(cpu_interval);
    }
    if mem_interval > 0 {
        system_metric::init_memory_collector(mem_interval);
    }

    if config::use_cache_time() {
        utils::start_time_ticker();
    }
    Ok(())
}
