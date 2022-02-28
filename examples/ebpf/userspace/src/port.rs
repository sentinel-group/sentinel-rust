use futures::stream::StreamExt;
use probes::port::PortEvent;
use redbpf::{load::Loader, xdp, Array};
use sentinel_core::{base, flow, EntryBuilder};
use std::sync::Arc;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

fn probe_code() -> &'static [u8] {
    include_bytes!(concat!(
        env!("BPF_DIR"),
        "/target/bpf/programs/port/port.elf"
    ))
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> std::result::Result<(), String> {
    // initialize sentinel
    sentinel_core::init_default().unwrap_or_else(|err| sentinel_core::logging::error!("{:?}", err));
    flow::load_rules(vec![Arc::new(flow::Rule {
        resource: "port:8000".into(),
        threshold: 1.0,
        calculate_strategy: flow::CalculateStrategy::Direct,
        control_strategy: flow::ControlStrategy::Reject,
        ..Default::default()
    })]);
    // initialize tracing
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::WARN)
        .finish();
    tracing::subscriber::set_global_default(subscriber).unwrap();

    // load xdp program
    let xdp_mode = xdp::Flags::SkbMode;
    let interfaces: Vec<String> = vec!["lo".to_string()];

    let mut loaded = Loader::load(probe_code()).map_err(|err| format!("{:?}", err))?;

    for interface in &interfaces {
        println!(
            "Attach block_port on interface: {} with mode {:?}",
            interface, xdp_mode
        );
        for prog in loaded.xdps_mut() {
            prog.attach_xdp(interface, xdp_mode)
                .map_err(|err| format!("{:?}", err))?;
        }
    }

    // start listening for port events
    let _ = tokio::spawn(async move {
        while let Some((map_name, events)) = loaded.events.next().await {
            let port_blocked_map = loaded.map("port_blocked").expect("port_blocked not found");
            let port_blocked =
                Array::<bool>::new(port_blocked_map).expect("error creating Array in userspace");
            for event in events {
                match map_name.as_str() {
                    "port_events" => {
                        let event = unsafe { std::ptr::read(event.as_ptr() as *const PortEvent) };
                        let res_name = format!("port:{}", event.port);
                        let entry_builder = EntryBuilder::new(res_name.clone())
                            .with_traffic_type(base::TrafficType::Inbound);
                        if let Ok(entry) = entry_builder.build() {
                            port_blocked
                                .set(event.port as u32, false)
                                .expect("error setting port_blocked, index out of bound");
                            if event.port < 10000 {
                                println!(
                                    "{} at {} passed",
                                    res_name,
                                    sentinel_core::utils::curr_time_millis()
                                );
                            }
                            entry.exit()
                        } else {
                            port_blocked
                                .set(event.port as u32, true)
                                .expect("error setting port_blocked, index out of bound");
                            println!(
                                "{} at {} blocked",
                                res_name,
                                sentinel_core::utils::curr_time_millis()
                            );
                        }
                    }
                    _ => panic!("unexpected event"),
                }
            }
        }
    })
    .await;

    Ok(())
}
