SRC_FILES := $(shell find . -path '*/target' -prune -o -name '*.rs' -print)
KERNEL_VERSION?=$(shell ls /lib/modules | grep generic | head -1)

check:
	cargo check --all-features

clippy:
	cargo clippy --all-targets

doc: clean
	cargo doc --no-deps 

clean:
	cargo clean

fmt:
	@rustfmt --edition 2018 $(SRC_FILES)

unit: unit_single unit_parallel

unit_single:
	cargo test --workspace --exclude sentinel-envoy-module --exclude ebpf-probes --exclude ebpf-userspace -- --ignored --test-threads=1 --nocapture

unit_parallel:
	cargo test --workspace --exclude sentinel-envoy-module --exclude ebpf-probes --exclude ebpf-userspace -- --nocapture

envoy:
	cargo build --target wasm32-unknown-unknown --release -p sentinel-envoy-module
	cp target/wasm32-unknown-unknown/release/sentinel_envoy_module.wasm examples/proxy/envoy/docker/sentinel_envoy_module.wasm
	cd examples/proxy/envoy && docker-compose up --build

ebpf_port:
	cd examples/ebpf/probes && KERNEL_VERSION=$(KERNEL_VERSION) cargo bpf build port --target-dir=../target
	cd examples/ebpf/userspace && KERNEL_VERSION=$(KERNEL_VERSION) BPF_DIR=$(shell pwd)/examples/ebpf cargo build --example port --target-dir=../target
	sudo ip link set dev lo xdpgeneric off
	sudo examples/ebpf/target/x86_64-unknown-linux-gnu/debug/examples/port


.PHONY: clean clippy doc fmt unit unit_single unit_parallel check envoy ebpf
