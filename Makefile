.PHONY: proto script test fmt clippy check help

# Default target: show help
help:
	@echo "Verryte Developer Makefile"
	@echo ""
	@echo "Usage:"
	@echo "  make proto      - Run the interactive TTY game (Wuthering Terminal)"
	@echo "  make script     - Run the automated script smoke test"
	@echo "  make test       - Run all workspace unit tests"
	@echo "  make fmt        - Check and format all workspace files"
	@echo "  make clippy     - Run clippy linting checks"
	@echo "  make check      - Verify that the workspace compiles cleanly"
	@echo ""

# Run the TTY game
proto:
	cargo run -p wuthering-terminal --bin wuthering-terminal

# Run the script smoke test
script:
	cargo run -p wuthering-terminal --bin wuthering-terminal-script -- "inspect:4,4 confirm inspect:4,5 confirm"

# Run tests
test:
	cargo test

# Format code
fmt:
	cargo fmt

# Lint code
clippy:
	cargo clippy --all-targets

# Compile check
check:
	cargo check
