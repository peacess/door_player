
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
	cargo +nightly fmt

cp_bin:
	cp -f target/release/door_player ${HOME}/bin/door_player
cp_dll:
	mkdir -p bin
	cp -f target/release/door_player.exe ./bin/
	cp -f ${FFMPEG_DIR}/bin/avformat-61.dll ./bin/
	cp -f ${FFMPEG_DIR}/bin/avutil-59.dll ./bin/
	cp -f ${FFMPEG_DIR}/bin/pkgconf-5.dll ./bin/
	cp -f ${FFMPEG_DIR}/bin/swresample-5.dll ./bin/
	cp -f ${FFMPEG_DIR}/bin/swscale-8.dll ./bin/
	cp -f ${FFMPEG_DIR}/bin/avcodec-61.dll  ./bin/
	cp -f ${FFMPEG_DIR}/bin/avdevice-61.dll ./bin/
	cp -f ${FFMPEG_DIR}/bin/avfilter-10.dll ./bin/
