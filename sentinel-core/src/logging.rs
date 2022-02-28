use cfg_if::cfg_if;
use lazy_static::lazy_static;
pub use log::{debug, error, info, trace, warn};
use std::sync::Once;

lazy_static! {
    pub static ref FREQUENT_ERROR_ONCE: Once = Once::new();
}

cfg_if! {
    if #[cfg(feature = "logger_env")] {
        use env_logger;
        use crate::config::DEFAULT_LOG_LEVEL;
        fn init_env_logger() {
            env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(DEFAULT_LOG_LEVEL))
                .init();
        }
        pub fn logger_init(_: Option<String>) {
            init_env_logger();
        }
    }
    else if #[cfg(feature = "logger_log4rs")] {
        use log4rs;
        use std::path::Path;
        fn init_log4rs(file_name: Option<String>) {
            let file_name = file_name.expect("Must provide a configuration file for log4rs crate");
            let path = Path::new(&file_name);
            if path.exists() {
                log4rs::init_file(path, Default::default()).unwrap();
            }
        }
        pub fn logger_init(file_name: Option<String>) {
            init_log4rs(file_name);
        }
    }else{
        pub fn logger_init(_: Option<String>) {}
    }
}
