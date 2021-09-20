use sentinel_macros::flow;
use sentinel_rs;
use sentinel_rs::utils::sleep_for_ms;
use std::sync::Arc;

/// a "hello-world" example on small code snippets with Sentinel attributes macros
fn main() {
    // Init sentienl configurations
    sentinel_rs::init_default().unwrap_or_else(|err| sentinel_rs::logging::error!("{:?}", err));

    let mut handlers = Vec::new();
    let db = Arc::new(DB {});
    for _ in 0..20 {
        handlers.push({
            let db_ptr = Arc::clone(&db);
            std::thread::spawn(move || {
                loop {
                    let val = db_ptr.query().unwrap_or_else(|_| {
                        // blocked
                        sleep_for_ms(10);
                        return 0u32;
                    });
                    println!("The value in DB is: {:?}", val);
                }
            })
        });
    }
    for h in handlers {
        h.join().expect("Couldn't join on the associated thread");
    }
}

struct DB {}

impl DB {
    #[flow(threshold = 10.0)]
    pub fn query(&self) -> u32 {
        sleep_for_ms(10);
        1u32
    }
}
