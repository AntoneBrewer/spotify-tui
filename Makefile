.PHONY: help build build-release build-windows clean test fmt clippy check install

# Default target
help:
	@echo "Spotify TUI - Build Commands"
	@echo ""
	@echo "Available targets:"
	@echo "  make build           - Build debug binary"
	@echo "  make build-release   - Build optimized release binary"
	@echo "  make build-windows   - Build Windows executable (.exe)"
	@echo "  make test            - Run tests"
	@echo "  make fmt             - Format code"
	@echo "  make clippy          - Run clippy linter"
	@echo "  make check           - Run cargo check"
	@echo "  make clean           - Clean build artifacts"
	@echo "  make install         - Install binary to system"

# Build debug binary
build:
	cargo build

# Build release binary
build-release:
	cargo build --release

# Build Windows executable
build-windows:
	@echo "Building Windows executable..."
	@echo "Note: This requires the Windows toolchain. See BUILD_WINDOWS.md for details."
	@rustup target add x86_64-pc-windows-gnu 2>/dev/null || true
	cargo build --release --target x86_64-pc-windows-gnu
	@echo ""
	@echo "✓ Windows executable built at: target/x86_64-pc-windows-gnu/release/spt.exe"

# Run tests
test:
	cargo test

# Format code
fmt:
	cargo fmt

# Run clippy
clippy:
	cargo clippy -- -D warnings

# Check if code compiles
check:
	cargo check

# Clean build artifacts
clean:
	cargo clean

# Install to system
install:
	cargo install --path .
