.PHONY: build

all: build

build:
	cargo build -r --target x86_64-unknown-linux-musl && \
    cargo build -r --target x86_64-apple-darwin && \
    cargo build -r --target x86_64-pc-windows-gnu && \
    make migrate

migrate-mac:
	cp target/x86_64-apple-darwin/release/iptv-checker-rs ./iptv-checker-rs-x86_64-apple-darwin

migrate-win:
	cp target/x86_64-pc-windows-gnu/release/iptv-checker-rs.exe ./iptv-checker-rs-x86_64-pc-windows-gnu.exe

migrate-linux:
	cp target/x86_64-unknown-linux-musl/release/iptv-checker-rs ./iptv-checker-rs-x86_64-unknown-linux-musl

migrate:
	make migrate-mac && make migrate-win && make migrate-linux