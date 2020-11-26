version := $(shell grep -Po '(\d.\d.\d)' Cargo.toml | head -n 1) #FIXME fragile

.PHONY: release
release:
	cargo build --release

.PHONY: debug
debug:
	cargo build

.PHONY: clean
clean:
	cargo clean

.PHONY: appimage
appimage: release
	scripts/build_appimage.sh $(shell pwd)/target/release/tag $(version) $(shell pwd)/dist

.PHONY: appimage-debug
appimage-debug: debug
	scripts/build_appimage.sh $(shell pwd)/target/debug/tag $(version) $(shell pwd)/dist