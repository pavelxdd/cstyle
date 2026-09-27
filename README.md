# CStyle

`cstyle` is a standalone formatter for C, C++, and Objective-C source code.

It formats files in place, works with stdin/stdout, and reads formatter options
from config files. The command-line interface is compatible with the option
syntax used by AStyle, so existing configs can often be reused.

## Install

```sh
just install
```

## Library

```rust
use cstyle::{api::Formatter, config::FormatOptions};

let formatter = Formatter::with_options(FormatOptions::default());
let output = formatter.format("int main(){return 0;}\n");
```

Use `api::format` for one-shot text formatting and `api::format_bytes` when the
input encoding and line endings must be preserved.

## Development

Project commands are `just` recipes; `just --list` shows all of them.

```sh
just test          # nextest suite; extra arguments go to nextest
just lint          # clippy on all targets
just perf-bounded  # release-mode bounded-runtime suite
just check         # full release gate
```

The release gate treats Rust, clippy, and rustdoc warnings as errors.

## Usage

```sh
cstyle [OPTION] [FILE]...
cstyle < input.c > output.c
```

Run `cstyle --help` for the current option list.

## Configuration

Put defaults in `.cstylerc`. Set `suffix=none` to disable backup files, or
`suffix=.bak` to select a backup suffix. An explicit command-line suffix option
overrides the config file.

For projects that already have one, `.astylerc` is also read as a fallback.
Environment defaults are read from `CSTYLE_OPTIONS` and `CSTYLE_PROJECT_OPTIONS`,
with `ARTISTIC_STYLE_OPTIONS` and `ARTISTIC_STYLE_PROJECT_OPTIONS` as fallback
names.

## Compatibility

`cstyle` targets AStyle-compatible option syntax and formatting behavior while
remaining a standalone Rust implementation. For deterministic comparisons, pass
`--options=none --project=none` to disable user and project configuration.
