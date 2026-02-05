
.PHONY: build rebuild clean upgrade format cp cp_linux cp_windows cp_macos

NAME := $(shell cargo metadata --no-deps --format-version=1 | jq -r ".packages[0].name")
VERSION := $(shell cargo metadata --no-deps --format-version=1 | jq -r ".packages[0].version")

ifeq ($(OS),Windows_NT)
	cp_cmd = cp_windows
	zip_cmd = zip_windows
else ifeq ($(shell uname -s),Linux)
	cp_cmd = cp_linux
	zip_cmd = zip_linux
else ifeq ($(shell uname -s),Darwin)
	cp_cmd = cp_linux
	zip_cmd = zip_linux
else
	$(error Unknown operating system. Please update the Makefile.)
endif

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
	cargo +nightly fmt
install:
	cargo install cargo-update cargo-edit
cp:
	$(MAKE) $(cp_cmd)
cp_linux:
	cp -f target/release/door_player ${HOME}/bin/door_player
cp_windows:
	mkdir -p bin
	rm -rf bin/*
	cp -f target/release/door_player.exe ./bin/
	cp -f ${FFMPEG_DIR}/bin/avformat-62.dll ./bin/
	cp -f ${FFMPEG_DIR}/bin/avutil-60.dll ./bin/
	# cp -f ${FFMPEG_DIR}/bin/pkgconf-5.dll ./bin/
	cp -f ${FFMPEG_DIR}/bin/swresample-6.dll ./bin/
	cp -f ${FFMPEG_DIR}/bin/swscale-9.dll ./bin/
	cp -f ${FFMPEG_DIR}/bin/avcodec-62.dll  ./bin/
	cp -f ${FFMPEG_DIR}/bin/avdevice-62.dll ./bin/
	cp -f ${FFMPEG_DIR}/bin/avfilter-11.dll ./bin/
cp_macos:

zip:
	$(MAKE) $(zip_cmd)
zip_windows: build cp_windows
	mkdir -p out
	rm -rf out/*
	zip out/$(NAME)-$(VERSION).zip bin/*
zip_linux: build cp_linux
	mkdir -p out
	rm -rf out/*
	zip out/$(NAME)-$(VERSION).zip bin/*
tool_windows:
	# install choco
	choco install zip jq -y
