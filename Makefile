SRC_FILES := $(shell find . -path '*/target' -prune -o -name '*.rs' -print)
KERNEL_VERSION?=$(shell ls /lib/modules | grep generic | head -1)

check:
	cargo check --all-features 

clippy:
	cargo clippy --all-targets --all-features 

doc: clean
	cargo doc --lib --no-deps --all-features --document-private-items 

clean:
	cargo clean

fmt:
	@rustfmt --edition 2021 $(SRC_FILES)

unit: unit_single unit_parallel

unit_single:
	cargo test -- --ignored --test-threads=1 --nocapture

unit_parallel:
	cargo test -- --nocapture

ebpf_port:
	cd examples/ebpf/probes && KERNEL_VERSION=$(KERNEL_VERSION) cargo bpf build port --target-dir=../target
	cd examples/ebpf/userspace && KERNEL_VERSION=$(KERNEL_VERSION) BPF_DIR=$(shell pwd)/examples/ebpf cargo build --example port --target-dir=../target
	sudo ip link set dev lo xdpgeneric off
	sudo examples/ebpf/target/x86_64-unknown-linux-gnu/debug/examples/port


.PHONY: clean clippy doc fmt unit unit_single unit_parallel check ebpf
