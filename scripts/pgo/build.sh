#!/usr/bin/env bash
# Builds a profile-guided release binary into target/pgo/release/cstyle.
#
# Usage: scripts/pgo/build.sh [DIR...]
#
# The profile comes from formatting C and C++ sources under several option
# sets. Without DIR arguments the sources are the projects pinned in
# corpus.txt, fetched once into $CSTYLE_PGO_CACHE, or else
# $XDG_CACHE_HOME/cstyle/pgo-corpus, or else ~/.cache/cstyle/pgo-corpus.
#
# A profile fits only the code and compiler it was made with, so each run
# makes it afresh. The optimized build takes much longer than a plain one.
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
root=$(cd "$here/../.." && pwd)
cd "$root"

note() { echo "build-pgo: $*" >&2; }
die() {
    note "$*"
    exit 1
}

sysroot=$(rustc --print sysroot)
profdata=$(find "$sysroot/lib/rustlib" -name llvm-profdata -type f 2>/dev/null | head -n 1)
[ -n "$profdata" ] || die "llvm-profdata not found; install it with: rustup component add llvm-tools"
jobs=$(getconf _NPROCESSORS_ONLN 2>/dev/null || echo 4)

corpus=()
if [ "$#" -gt 0 ]; then
    corpus=("$@")
else
    command -v git > /dev/null || die "git is needed to fetch the training corpus"
    cache=${CSTYLE_PGO_CACHE:-${XDG_CACHE_HOME:-$HOME/.cache}/cstyle/pgo-corpus}
    mkdir -p "$cache"
    while read -r name url tag; do
        case "$name" in '' | '#'*) continue ;; esac
        dir="$cache/$name-$tag"
        if [ ! -d "$dir" ]; then
            note "fetching $name $tag"
            # A clone lands under its final name only once complete.
            partial=$(mktemp -d "$cache/.fetch.XXXXXX")
            if ! git -c advice.detachedHead=false clone --quiet --depth 1 --branch "$tag" "$url" "$partial"; then
                rm -rf "$partial"
                die "cannot fetch $url at $tag"
            fi
            mv "$partial" "$dir"
        fi
        corpus+=("$dir")
    done < "$here/corpus.txt"
fi
for dir in "${corpus[@]}"; do
    [ -d "$dir" ] || die "no such directory: $dir"
done

profiles="$root/target/pgo-profile"
rm -rf "$profiles"
mkdir -p "$profiles"
sources="$profiles/sources"
find "${corpus[@]}" -type f \( -name '*.c' -o -name '*.h' -o -name '*.cc' -o -name '*.cpp' \
    -o -name '*.cxx' -o -name '*.hh' -o -name '*.hpp' -o -name '*.hxx' -o -name '*.m' \) \
    -print0 > "$sources"
count=$(tr -cd '\0' < "$sources" | wc -c | tr -d ' ')
[ "$count" -gt 0 ] || die "no C or C++ sources under: ${corpus[*]}"

note "building the instrumented binary"
RUSTFLAGS="${RUSTFLAGS:+$RUSTFLAGS }-Cprofile-generate=$profiles" \
    cargo build --release --target-dir "$root/target/pgo-instrumented"
trainer="$root/target/pgo-instrumented/release/cstyle"

# The project's own options and the main styles, with options that steer
# splitting, padding and pointer alignment.
option_sets=(
    "--options=.cstylerc"
    "--style=kr"
    "--style=allman"
    "--style=gnu"
    "--style=java"
    "--style=whitesmith --indent-switches"
    "--style=google --max-code-length=80"
    "--style=linux --pad-oper --unpad-paren --align-pointer=type"
)
export LLVM_PROFILE_FILE="$profiles/cstyle-%4m.profraw"
for options in "${option_sets[@]}"; do
    note "training on $count files with $options"
    # Each file goes through stdin and stdout, so the corpus stays as it is.
    xargs -0 -n 1 -P "$jobs" sh -c \
        '"$0" --options=none --project=none $1 < "$2" > /dev/null 2>&1 || true' \
        "$trainer" "$options" < "$sources"
done
"$profdata" merge -o "$profiles/cstyle.profdata" "$profiles"/*.profraw

note "building the optimized binary"
RUSTFLAGS="${RUSTFLAGS:+$RUSTFLAGS }-Cprofile-use=$profiles/cstyle.profdata" \
    cargo build --release --target-dir "$root/target/pgo"
note "built target/pgo/release/cstyle"
