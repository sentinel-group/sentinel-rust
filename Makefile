check:
	cargo check

clippy:
	cargo clippy --all-targets --all-features

doc: clean
	cargo doc --no-deps 

clean:
	cargo clean

fmt:
	cargo fmt

unit: unit_single unit_parallel

unit_single:
	cargo test -- --ignored --test-threads=1

unit_parallel:
	cargo test

.PHONY: clean clippy doc fmt unit unit_single unit_parallel check
