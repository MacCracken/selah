.PHONY: check build test clippy fmt fmt-check deny doc clean

check:
	cargo check --all-features

build:
	cargo build --all-features

test:
	cargo test --all-features

clippy:
	cargo clippy --all-features -- -D warnings

fmt:
	cargo fmt

fmt-check:
	cargo fmt -- --check

deny:
	cargo deny check

doc:
	cargo doc --all-features --no-deps

clean:
	cargo clean
