use futures::stream::StreamExt;
use probes::port::PortEvent;
use redbpf::{load::Loader, xdp};
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
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::WARN)
        .finish();
    tracing::subscriber::set_global_default(subscriber).unwrap();

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

    let _ = tokio::spawn(async move {
        while let Some((map_name, events)) = loaded.events.next().await {
            for event in events {
                match map_name.as_str() {
                    "port_events" => {
                        let event = unsafe { std::ptr::read(event.as_ptr() as *const PortEvent) };
                        println!("port number {}", event.port);
                    }
                    _ => panic!("unexpected event"),
                }
            }
        }
    })
    .await;

    Ok(())
}
