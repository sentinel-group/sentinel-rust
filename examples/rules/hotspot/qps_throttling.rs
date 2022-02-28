use sentinel_core::base::{MetricEvent, ReadStat, ResourceType};
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
                "[HotSpot Concurrency] pass: {:?}, block: {:?}, complete: {:?}",
                node.qps(MetricEvent::Pass),
                node.qps(MetricEvent::Block),
                node.qps(MetricEvent::Complete)
            );
            sleep_for_ms(100);
        }
    }));
    for h in handlers {
        h.join().expect("Couldn't join on the associated thread");
    }
}

#[hotspot(
    threshold = 100,
    metric_type = "QPS",
    control_strategy = "Throttling",
    max_queueing_time_ms = 5,
    duration_in_sec = 1,
    param_index = 0,
    args = r#"vec!["task".into()]"#
)]
fn task() {
    println!("{}: passed", sentinel_core::utils::curr_time_millis());
    sleep_for_ms(10);
}
