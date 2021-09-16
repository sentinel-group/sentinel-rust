use sentinel_macros::flow;
use sentinel_rs;
use tokio::time::{sleep, Duration};

/// an example on async functions
#[tokio::main]
async fn main() {
    // Init sentienl configurations
    sentinel_rs::init_default().unwrap_or_else(|err| sentinel_rs::logging::error!("{:?}", err));

    let mut handlers = Vec::new();
    for _ in 0..20 {
        handlers.push(tokio::spawn(async move {
            loop {
                task().await.unwrap_or_else(|_| {
                    sentinel_rs::utils::sleep_for_ms(100);
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
    println!("{}: {}", sentinel_rs::utils::curr_time_millis(), "passed");
    sleep(Duration::from_millis(100)).await;
}
