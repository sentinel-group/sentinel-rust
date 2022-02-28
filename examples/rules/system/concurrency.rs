#![allow(clippy::needless_update)]
use sentinel_core::base::{ConcurrencyStat, ResourceType};
use sentinel_core::stat::get_or_create_resource_node;
use sentinel_core::utils::sleep_for_ms;
use sentinel_macros::system;

/// a "hello-world" example on small code snippets with Sentinel attributes macros
fn main() {
    // Init sentienl configurations
    sentinel_core::init_default().unwrap_or_else(|err| sentinel_core::logging::error!("{:?}", err));

    let mut handlers = Vec::new();
    for _ in 0..20 {
        handlers.push(std::thread::spawn(move || {
            loop {
                task().unwrap_or_else(|_| {
                    // blocked
                    sleep_for_ms(10);
                });
            }
        }));
    }
    handlers.push(std::thread::spawn(|| {
        let node = get_or_create_resource_node(&"task".into(), &ResourceType::Common);
        loop {
            println!(
                "[System Concurrency] currentConcurrency: {:?}",
                node.current_concurrency()
            );
            sleep_for_ms(100);
        }
    }));
    for h in handlers {
        h.join().expect("Couldn't join on the associated thread");
    }
}

#[system(
    traffic_type = "Inbound",
    threshold = 3.0,
    metric_type = "Concurrency",
    adaptive_strategy = "NoAdaptive"
)]
fn task() {
    println!("{}: passed", sentinel_core::utils::curr_time_millis());
    sleep_for_ms(10);
}
