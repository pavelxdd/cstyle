# Run `just --list` to see all available recipes.
# Reference: https://just.systems/man/en/

set shell := ["bash", "-euo", "pipefail", "-c"]

nextest := "cargo nextest --config-file nextest.toml run"

# List project commands
default:
    @just --list --unsorted

# Build the debug binary and library
[group('build')]
build:
    cargo build

# Build optimized release artifacts
[group('build')]
build-release:
    cargo build --release

# Build a profile-guided release binary into target/pgo/ (slow); trains on a fetched corpus unless DIRs are given
[group('build')]
build-pgo *dirs:
    scripts/pgo/build.sh {{dirs}}

# Install cstyle from the working tree
[group('build')]
install:
    cargo install --path .

# Install the profile-guided binary from target/pgo (run build-pgo first)
[group('build')]
install-pgo:
    #!/usr/bin/env bash
    set -euo pipefail
    bin=target/pgo/release/cstyle
    [ -x "$bin" ] || { echo "install-pgo: $bin is missing; run just build-pgo first" >&2; exit 1; }
    install -m 755 "$bin" "${CARGO_HOME:-$HOME/.cargo}/bin/cstyle"

# Remove Cargo build artifacts
[group('build')]
clean:
    cargo clean

# Format Rust sources
[group('quality')]
fmt:
    cargo fmt

# Check Rust source formatting
[group('quality')]
fmt-check:
    cargo fmt --check

# Run clippy on all targets with warnings denied
[group('quality')]
lint:
    cargo clippy --all-targets -- -Dwarnings

# Build API documentation with warnings denied
[group('quality')]
doc:
    RUSTDOCFLAGS="${RUSTDOCFLAGS:+$RUSTDOCFLAGS }-Dwarnings" cargo doc --no-deps

# Run the test suite; arguments are passed to nextest
[group('test')]
test *args:
    CARGO_TARGET_DIR=target/test {{nextest}} {{args}}

# Run one integration-test target (format, cli, perf_bounded); further arguments go to nextest
[group('test')]
test-target target *args:
    CARGO_TARGET_DIR=target/test {{nextest}} --test {{target}} {{args}}

# Run the single-threaded bounded-runtime suite in release mode
[group('test')]
perf-bounded *args:
    CARGO_TARGET_DIR=target/test {{nextest}} --release --profile release-perf --test perf_bounded {{args}}

# Create the release package archive
[group('release')]
package:
    CARGO_TARGET_DIR=target/package-build cargo package

# Verify packaging from a dirty working tree
[group('release')]
package-check:
    CARGO_TARGET_DIR=target/package-check cargo package --allow-dirty

# Run the complete release gate with warnings denied
[group('release')]
check: fmt-check lint
    #!/usr/bin/env bash
    set -euo pipefail
    export CARGO_TARGET_DIR=target/check CARGO_INCREMENTAL=0
    export RUSTFLAGS="${RUSTFLAGS:+$RUSTFLAGS }-Dwarnings"
    export RUSTDOCFLAGS="${RUSTDOCFLAGS:+$RUSTDOCFLAGS }-Dwarnings"
    cargo build
    {{nextest}}
    cargo build --release
    cargo doc --no-deps
    CARGO_TARGET_DIR=target/package-check cargo package --allow-dirty
