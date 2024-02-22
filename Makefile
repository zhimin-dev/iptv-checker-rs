.PHONY: build

all: build

build:
	make init-cargo && \
	cargo build -r --target x86_64-unknown-linux-musl && \
    cargo build -r --target x86_64-apple-darwin && \
    cargo build -r --target x86_64-pc-windows-gnu && \
    cargo build -r --target aarch64-unknown-linux-gnu && \
    make migrate

init-cargo:
	mkdir .cargo && cp cargo_config.toml .cargo/config.toml

migrate-mac:
	cp target/x86_64-apple-darwin/release/iptv-checker-rs ./iptv-checker-rs-x86_64-apple-darwin

migrate-win:
	cp target/x86_64-pc-windows-gnu/release/iptv-checker-rs.exe ./iptv-checker-rs-x86_64-pc-windows-gnu.exe

migrate-linux:
	cp target/x86_64-unknown-linux-musl/release/iptv-checker-rs ./iptv-checker-rs-x86_64-unknown-linux-musl

migrate-linux-arm:
	cp target/aarch64-unknown-linux-gnu/release/iptv-checker-rs ./iptv-checker-rs-aarch64-unknown-linux-gnu

migrate:
	make migrate-mac && make migrate-win && make migrate-linux && make migrate-linux-arm