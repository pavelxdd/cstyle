# CStyle

`cstyle` is a standalone formatter for C, C++, and Objective-C source code.

It formats files in place, works with stdin/stdout, and reads formatter options
from config files. The command-line interface is compatible with the option
syntax used by AStyle, so existing configs can often be reused.

## Install

```sh
just install      # cargo install from the working tree
just install-pgo  # copy the binary from `just build-pgo`
```

## Usage

```sh
cstyle [OPTION] [FILE]...
cstyle < input.c > output.c
```

Run `cstyle --help` for the current option list.

## Configuration

Put defaults in `.cstylerc`; `.astylerc` is read as a fallback. Environment
defaults come from `CSTYLE_OPTIONS` and `CSTYLE_PROJECT_OPTIONS`, with the
`ARTISTIC_STYLE_*` names as fallback.

## Library

```rust
use cstyle::{api::Formatter, config::FormatOptions};

let formatter = Formatter::with_options(FormatOptions::default());
let output = formatter.format("int main(){return 0;}\n");
```

## Development

Project commands are `just` recipes; `just --list` shows all of them.
