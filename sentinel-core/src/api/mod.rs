//! mod `api` provides the topmost fundamental APIs for users using sentinel-core.
//! Users must initialize Sentinel before loading Sentinel rules. Sentinel support three ways to perform initialization:
//!
//!  1. `init_default()`, using default config to initialize.
//!  2. `init_with_config(config_entity: config::Entity)`, using customized config Entity to initialize.
//!  3. `init_with_config_file(config_path: String)`, using yaml file to initialize.
//! For the examples, visit the [Sentinel repository](https://github.com/sentinel-group/sentinel-rust)

mod base;
mod init;
mod slot_chain;

pub use base::*;
pub use init::*;
pub use slot_chain::*;
