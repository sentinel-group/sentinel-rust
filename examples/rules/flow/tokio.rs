use sentinel_macros::flow;

use tokio::time::{sleep, Duration};

/// an example on async functions
#[tokio::main]
async fn main() {
    // Init sentienl configurations
    sentinel_core::init_default().unwrap_or_else(|err| sentinel_core::logging::error!("{:?}", err));

    let mut handlers = Vec::new();
    for _ in 0..20 {
        handlers.push(tokio::spawn(async move {
            loop {
                task().await.unwrap_or_else(|_| {
                    sentinel_core::utils::sleep_for_ms(100);
                });
            }
        }));
    }
    for h in handlers {
        h.await.expect("Couldn't join on the associated thread");
    }
}

#[flow(threshold = 10.0)]
async fn task() {
    println!("{}: passed", sentinel_core::utils::curr_time_millis());
    sleep(Duration::from_millis(100)).await;
}
