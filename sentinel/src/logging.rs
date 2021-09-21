use crate::config::DEFAULT_LOG_LEVEL;
use env_logger;
use lazy_static::lazy_static;
pub use log::{debug, error, info, trace, warn};
use log4rs;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Once;

// todo: may conflict with the logger used by users

// todo: use feature attr for conditional compiling
// currently, it simply loads all of supported loggers

lazy_static! {
    static ref LOG_FILE_NAME: String = String::from("sentinel-record.log");
    pub static ref FREQUENT_ERROR_ONCE: Once = Once::new();
}

/// supported loggers with user-defined settings
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Logger {
    None,
    // a simple console logger and its logging level
    EnvLogger(String),
    // a configurable logger and its configuration file path
    Log4rs(String),
}

pub fn logger_init(logger: Logger) {
    match logger {
        Logger::None => {
            // user must explicitly disable the logger by Logger::None
        }
        Logger::EnvLogger(level) => {
            env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(level))
                .init()
        }
        Logger::Log4rs(ref file_path) => {
            let path = Path::new(file_path);
            if path.exists() {
                log4rs::init_file(path, Default::default())
                    .unwrap_or_else(|_| default_logger_init());
            } else {
                default_logger_init();
            }
        }
    }
}

#[inline]
fn default_logger_init() {
    logger_init(Logger::EnvLogger(DEFAULT_LOG_LEVEL.into()));
    info!("Current logger is the default one. If this is unexpected, check your configuration.");
}
