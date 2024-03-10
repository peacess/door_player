
.PHONY: build clean upgrade format

build:
	cargo build --release
clean:
	cargo clean
upgrade:
	cargo upgrade --incompatible
format:
	cargo +nightly-gnu fmt