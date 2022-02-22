# eBPF with Sentinel

See Makefile in the root directory. Run `make ebpf_port`.

Use `ip link show dev lo` to check whether the XDP program is attached to `lo` device.

Use `sudo ip link set dev lo xdpgeneric off` to remove XDP program from the `lo` device.