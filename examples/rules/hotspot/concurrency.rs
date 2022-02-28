use sentinel_core::base::{ConcurrencyStat, ResourceType};
use sentinel_core::stat::get_or_create_resource_node;
use sentinel_core::utils::sleep_for_ms;
use sentinel_macros::hotspot;

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
                "[HotSpot Concurrency] currentConcurrency: {:?}",
                node.current_concurrency()
            );
            sleep_for_ms(100);
        }
    }));
    for h in handlers {
        h.join().expect("Couldn't join on the associated thread");
    }
}

#[hotspot(
    threshold = 3,
    metric_type = "Concurrency",
    param_index = 0,
    args = r#"vec!["task".into()]"#
)]
fn task() {
    println!("{}: passed", sentinel_core::utils::curr_time_millis());
    sleep_for_ms(10);
}
