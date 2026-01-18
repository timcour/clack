.PHONY: clack build test deps all clean install uninstall

PREFIX ?= /usr/local
BINDIR ?= $(PREFIX)/bin
MANDIR ?= $(PREFIX)/share/man/man1
MANPAGE ?= man/clack.1

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

install: clack
	@echo "Installing clack to $(BINDIR) and man page to $(MANDIR) (run with sudo)"
	install -d "$(BINDIR)" "$(MANDIR)"
	install -m 755 target/release/clack "$(BINDIR)/clack"
	install -m 644 "$(MANPAGE)" "$(MANDIR)/clack.1"

uninstall:
	@echo "Removing clack from $(BINDIR) and man page from $(MANDIR) (run with sudo)"
	rm -f "$(BINDIR)/clack" "$(MANDIR)/clack.1"
