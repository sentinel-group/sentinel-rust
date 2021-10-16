use sentinel_macros::flow;
use sentinel_rs::config::ConfigEntity;
use sentinel_rs::utils::sleep_for_ms;

fn main() {
    // Init sentienl configurations from a ConfigEntity
    let mut config = ConfigEntity::default();
    config.config.app.app_name = String::from("ConfigEntityApp");
    sentinel_rs::init_with_config(config)
        .unwrap_or_else(|err| sentinel_rs::logging::error!("{:?}", err));

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

#[flow(threshold = 10.0)]
fn task() {
    println!("{}: passed", sentinel_rs::utils::curr_time_millis());
    sleep_for_ms(10);
}
