
.PHONY: build rebuild clean upgrade format

build:
	cargo build --release
release: build
	cp -f target/release/door_player ${HOME}/bin/
clean:
	cargo clean
	rm Cargo.lock
rebuild: clean build
upgrade:
	cargo upgrade && cargo update
format:
	cargo +nightly-gnu fmt