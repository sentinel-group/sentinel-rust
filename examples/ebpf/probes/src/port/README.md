# eBPF Kernel Space Probes

These example programs are based on [redbpf](https://github.com/foniod/redbpf). Use eBPF map to exchange data between userspace Sentinel and the kernel space XDP program.

- port: XDP program emits PerfMap for each port. Cooperate with userspace port program.

## Attentions

- This example is tested on [WSL2](https://github.com/microsoft/WSL2-Linux-Kernel/releases/tag/linux-msft-wsl-5.10.74.3). In WSL2 with older kernels, the BTF is not activated. If you work on WSL2, it is recommended to upgrade your kernel to `5.10.74.3` at least. 

- The `redbpf` on WSL2 need explicit `KERNEL_VERSION` during building. The reason is the same as [issues](https://github.com/foniod/redbpf/issues/179) in docker images.

- Run `sudo cargo bpf load -i lo target/bpf/programs/xxx/xxx.elf` to load eBPF probes for test. For privilege problem on `cargo-bpf`, visit this [issue](https://github.com/foniod/redbpf/issues/288). In fact, you do not need to use `cargo-bpf` to load bpf image. You can simply write a userspace program and load the image in code. Then you only need to run `sudo` on complied userspace program, instead of the `sudo cargo bpf` command.

