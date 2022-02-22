#![no_std]
#![no_main]
use ebpf_probes::port::PortEvent;
use redbpf_probes::helpers::bpf_trace_printk;
use redbpf_probes::xdp::prelude::*;

program!(0xFFFFFFFE, "GPL");

#[map]
static mut port_events: PerfMap<PortEvent> = PerfMap::with_max_entries(1024);

#[xdp]
pub fn block_port(ctx: XdpContext) -> XdpResult {
    if let Ok(transport) = ctx.transport() {
        bpf_trace_printk(b"Got transport\0");
        let event = MapData::new(PortEvent {
            port: transport.dest(),
            reject: false,
        });
        unsafe { port_events.insert(&ctx, &event) };
    }
    Ok(XdpAction::Pass)
}
