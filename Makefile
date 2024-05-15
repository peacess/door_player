
.PHONY: build rebuild clean upgrade format

build:
	cargo build --release
clean:
	cargo clean
rebuild: clean build
upgrade:
	cargo upgrade --incompatible
format:
	cargo +nightly-gnu fmt