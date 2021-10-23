use crate::config::DEFAULT_LOG_LEVEL;
#[cfg(feature = "logger_env")]
use env_logger;
use lazy_static::lazy_static;
pub use log::{debug, error, info, trace, warn};
#[cfg(feature = "logger_log4rs")]
use log4rs;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Once;

lazy_static! {
    pub static ref FREQUENT_ERROR_ONCE: Once = Once::new();
}

pub fn logger_init(file_name: Option<String>) {
    #[cfg(feature = "logger_env")]
    init_env_logger();

    #[cfg(feature = "logger_log4rs")]
    init_log4rs(file_name);
}

#[cfg(feature = "logger_env")]
fn init_env_logger() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(DEFAULT_LOG_LEVEL))
        .init();
}

#[cfg(feature = "logger_log4rs")]
fn init_log4rs(file_name: Option<String>) {
    let file_name = file_name.expect("Must provide a configuration file for log4rs crate");
    let path = Path::new(&file_name);
    if path.exists() {
        log4rs::init_file(path, Default::default()).unwrap();
    }
}
