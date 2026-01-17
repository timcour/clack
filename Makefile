.PHONY: clack build test deps all clean

# Default target
clack:
	cargo build --release
	@echo "Binary built: target/release/clack"

build:
	cargo build

test:
	cargo test --all-features

deps:
	@echo "Installing Rust toolchain if needed..."
	@command -v rustc >/dev/null 2>&1 || { \
		echo "Rust not found. Please install from https://rustup.rs/"; \
		exit 1; \
	}
	@echo "Rust toolchain is installed"
	@rustc --version
	@cargo --version

all: deps clack test

clean:
	cargo clean
