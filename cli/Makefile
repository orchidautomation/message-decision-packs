PREFIX ?= $(HOME)/.local/bin

.PHONY: build test install-local

build:
	cargo build --release

test:
	cargo test
	cargo run -- --help >/dev/null

install-local: build
	mkdir -p $(PREFIX)
	cp target/release/mdp $(PREFIX)/mdp
	chmod +x $(PREFIX)/mdp
