use sentinel_macros::flow;
use sentinel_rs;
use sentinel_rs::utils::sleep_for_ms;

/// an example on `flow::ControlStrategy::Throttling`
fn main() {
    // Init sentienl configurations
    sentinel_rs::init_default().unwrap_or_else(|err| sentinel_rs::logging::error!("{:?}", err));

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

#[flow(control_strategy = "Throttling", threshold = 10.0)]
fn task() {
    println!("{}: {}", sentinel_rs::utils::curr_time_millis(), "passed");
    sleep_for_ms(10);
}
