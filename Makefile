
.PHONY: build rebuild clean upgrade format cp cp_linux cp_windows cp_macos

NAME := $(shell cargo metadata --no-deps --format-version=1 | jq -r ".packages[0].name")
VERSION := $(shell cargo metadata --no-deps --format-version=1 | jq -r ".packages[0].version")

ifeq ($(OS),Windows_NT)
	cp_cmd = cp_windows
	zip_cmd = zip_windows
	FFMPEG_DIR:=$(CURDIR)/vcpkg_installed/x64-windows
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
	PKG_CONFIG_PATH="$(CURDIR)/vcpkg_installed/x64-windows/lib/pkgconfig" \
	FFMPEG_DIR="$(CURDIR)/vcpkg_installed/x64-windows" \
	cargo build --release
release: build
	cp -f target/release/door_player ${HOME}/bin/door_player/
clean:
	cargo clean
	rm -rf Cargo.lock bin out
rebuild: clean build
upgrade:
	cargo upgrade --incompatible
format:
	cargo +nightly fmt
install:
	cargo install cargo-update cargo-edit
cp:
	$(MAKE) $(cp_cmd)
cp_linux: build
	cp -f target/release/door_player ${HOME}/bin/door_player
cp_windows:
	mkdir -p bin
	rm -rf bin/*
	cp -f target/release/door_player.exe ./bin/
	cp -f ${FFMPEG_DIR}/bin/avformat-63.dll ./bin/
	cp -f ${FFMPEG_DIR}/bin/avutil-61.dll ./bin/
	# cp -f ${FFMPEG_DIR}/bin/pkgconf-5.dll ./bin/
	cp -f ${FFMPEG_DIR}/bin/swresample-7.dll ./bin/
	cp -f ${FFMPEG_DIR}/bin/swscale-10.dll ./bin/
	cp -f ${FFMPEG_DIR}/bin/avcodec-63.dll  ./bin/
	cp -f ${FFMPEG_DIR}/bin/avdevice-63.dll ./bin/
	cp -f ${FFMPEG_DIR}/bin/avfilter-12.dll ./bin/
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
install_vcpkg:
	vcpkg.exe install --x-manifest-root=.
