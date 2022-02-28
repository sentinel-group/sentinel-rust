use sentinel_macros::flow;

use sentinel_core::utils::sleep_for_ms;

fn main() {
    // Init sentienl configurations from yaml file
    let yaml_name = String::from("testdata/config/sentinel.yaml");
    sentinel_core::init_with_config_file(yaml_name)
        .unwrap_or_else(|err| sentinel_core::logging::error!("{:?}", err));

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
    println!("{}: passed", sentinel_core::utils::curr_time_millis());
    sleep_for_ms(10);
}
