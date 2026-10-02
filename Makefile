.PHONY: build run test clippy fmt clean

build:
	cargo build --release

run:
	cargo run --release

test:
	cargo test

clippy:
	cargo clippy --all-targets -- -D warnings

fmt:
	cargo fmt

clean:
	cargo clean