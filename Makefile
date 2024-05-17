
.PHONY: build rebuild clean upgrade format

build:
	cargo build --release
clean:
	cargo clean
	rm Cargo.lock
rebuild: clean build
upgrade:
	cargo upgrade && cargo update
format:
	cargo +nightly-gnu fmt