use sentinel_macros::flow;

use sentinel_core::utils::sleep_for_ms;

/// an example on `flow::CalculateStrategy::MemoryAdaptive`
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
    for h in handlers {
        h.join().expect("Couldn't join on the associated thread");
    }
}

#[flow(
    traffic_type = "Inbound",
    calculate_strategy = "MemoryAdaptive",
    mem_low_water_mark = 128,
    mem_high_water_mark = 512,
    low_mem_usage_threshold = 5,
    high_mem_usage_threshold = 1
)]
fn task() {
    println!("{}: passed", sentinel_core::utils::curr_time_millis());
    sleep_for_ms(10);
}
