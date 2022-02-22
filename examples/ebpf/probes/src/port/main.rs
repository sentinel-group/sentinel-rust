#![no_std]
#![no_main]
use ebpf_probes::port::PortEvent;
use redbpf_probes::maps::Array;
use redbpf_probes::xdp::prelude::*;

program!(0xFFFFFFFE, "GPL");

#[map]
static mut port_events: PerfMap<PortEvent> = PerfMap::with_max_entries(1024);
#[map]
static mut port_blocked: Array<bool> = Array::with_max_entries(1 << 16);

#[xdp]
pub fn block_port(ctx: XdpContext) -> XdpResult {
    if let Ok(transport) = ctx.transport() {
        let port = transport.dest();
        let event = MapData::new(PortEvent { port });
        unsafe { port_events.insert(&ctx, &event) };
        // the mmapped memory port_blocked not sync between kernel and userspace
        let blocked = unsafe { port_blocked.get(port as u32) };
        if let Some(&blocked) = blocked {
            if blocked {
                return Ok(XdpAction::Drop);
            }
        }
    }
    Ok(XdpAction::Pass)
}
