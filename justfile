# Run `just --list` to see all available recipes.
# Reference: https://just.systems/man/en/

set shell := ["bash", "-lc"]

test_target := "target/test"

# List supported project commands
default:
    @just --list

# Build the debug binary and library
build:
    cargo build

# Build optimized release artifacts
build-release:
    cargo build --release

# Format Rust sources
fmt:
    cargo fmt

# Check Rust source formatting
fmt-check:
    cargo fmt --check

# Build API documentation with warnings denied
doc:
    RUSTDOCFLAGS="${RUSTDOCFLAGS:+$RUSTDOCFLAGS }-Dwarnings" cargo doc --no-deps

# Create the release package archive
package:
    CARGO_TARGET_DIR=target/package-build cargo package

# Verify packaging from a dirty working tree
package-check:
    CARGO_TARGET_DIR=target/package-check cargo package --allow-dirty

# Remove Cargo build artifacts
clean:
    cargo clean

# Install cstyle from the working tree
install:
    cargo install --path .

# Run the test suite, optionally filtered by nextest arguments
test *filters:
    CARGO_TARGET_DIR={{test_target}} cargo nextest --config-file nextest.toml run {{filters}}

# Run tests matching one name filter
test-one filter *args:
    CARGO_TARGET_DIR={{test_target}} cargo nextest --config-file nextest.toml run {{filter}} {{args}}

# Run one integration-test target
test-file target *filters:
    CARGO_TARGET_DIR={{test_target}} cargo nextest --config-file nextest.toml run --test {{target}} {{filters}}

# Run the single-threaded bounded-runtime suite
perf-bounded *filters:
    CARGO_TARGET_DIR={{test_target}} cargo nextest --config-file nextest.toml run --release --profile release-perf --test perf_bounded {{filters}}

# Run the complete release gate
check: fmt-check
    CARGO_TARGET_DIR=target/check CARGO_INCREMENTAL=0 RUSTFLAGS="${RUSTFLAGS:+$RUSTFLAGS }-Dwarnings" cargo build
    CARGO_TARGET_DIR=target/check CARGO_INCREMENTAL=0 RUSTFLAGS="${RUSTFLAGS:+$RUSTFLAGS }-Dwarnings" cargo nextest --config-file nextest.toml run
    CARGO_TARGET_DIR=target/check CARGO_INCREMENTAL=0 RUSTFLAGS="${RUSTFLAGS:+$RUSTFLAGS }-Dwarnings" cargo build --release
    CARGO_TARGET_DIR=target/check RUSTDOCFLAGS="${RUSTDOCFLAGS:+$RUSTDOCFLAGS }-Dwarnings" cargo doc --no-deps
    CARGO_TARGET_DIR=target/package-check CARGO_INCREMENTAL=0 RUSTFLAGS="${RUSTFLAGS:+$RUSTFLAGS }-Dwarnings" cargo package --allow-dirty
