use env_logger;
pub use log;

pub fn new_console_logger() {
    env_logger::init()
}
