TEST = (cargo test)


clippy:
	cargo clippy --all-targets --all-features

doc:
	cargo doc

clean:
	cargo clean

fmt:
	cargo fmt

unit: unit_single unit_parallel

unit_single:
	cargo test rule_manager -- --test-threads=1

unit_parallel:
	cargo test -- --skip rule_manager

api:
	cargo test api::flow -- --nocapture

.PHONY: clean clippy doc fmt unit unit_single unit_parallel api
