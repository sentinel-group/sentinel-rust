SRC_FILES := $(shell find . -name '*.rs' -print)

check:
	cargo check

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
	cargo test -- --ignored --test-threads=1

unit_parallel:
	cargo test

.PHONY: clean clippy doc fmt unit unit_single unit_parallel check
