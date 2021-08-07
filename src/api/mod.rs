//! mod `api` provides the topmost fundamental APIs for users using sentinel-rs.
//! Users must initialize Sentinel before loading Sentinel rules. Sentinel support three ways to perform initialization:
//!
//!  1. `init_default()`, using default config to initialize.
//!  2. `init_with_config(config_entity: config::Entity)`, using customized config Entity to initialize.
//!  3. `init_with_config_file(config_path: String)`, using yaml file to initialize.
//!
//! Here is the example code to use Sentinel:
//!
//! ```
//! use rand;
//! use sentinel;
//! use sentinel::{core::base, EntryBuilder};
//!
//! #[test]
//! fn entry() {
//!     // Init sentienl configurations
//!     sentinel::init_default().unwrap_or_else(|err| sentinel::logging::error!("{:?}", err));
//!     // Load sentinel rules
//!     /*
//!     sentinel::flow::load_rules(
//!     sentinel::flow::Rule::new("some-test", sentinel::base::ResourceType::QPS, 10, sentinel::base::TrafficType::Reject)
//!         .unwrap_or_else(|err| sentinel::logging::error!("{:?}", err)),
//!     );
//!     */
//!     let mut handlers = Vec::new();
//!     for _ in 0..10 {
//!         handlers.push(std::thread::spawn(|| {
//!             loop {
//!                 let entry_builder = EntryBuilder::new("some-test".into())
//!                     .with_traffic_type(base::TrafficType::Inbound);
//!                 if let Ok(entry) = entry_builder.build() {
//!                     // Passed, wrap the logic here.
//!                     println!("{}: {}", sentinel::utils::curr_time_millis(), "passed");
//!                     std::thread::sleep(std::time::Duration::from_millis(
//!                         rand::random::<u64>() % 10,
//!                     ));
//!                     // Be sure the entry is exited finally.
//!                     entry.exit()
//!                 } else {
//!                     // Blocked. We could get the block reason from the BlockError.
//!                     std::thread::sleep(std::time::Duration::from_millis(
//!                         rand::random::<u64>() % 10,
//!                     ));
//!                 }
//!             }
//!         }));
//!     }
//!     for h in handlers {
//!         h.join().expect("Couldn't join on the associated thread");
//!     }
//! }
//!
//!
//! ```
//!

pub mod api;
pub mod init;
pub mod slot_chain;

pub use api::*;
pub use init::*;
pub use slot_chain::*;

pub use crate::core::config;
