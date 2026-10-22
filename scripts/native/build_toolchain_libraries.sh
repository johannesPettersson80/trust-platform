#!/usr/bin/env bash
# Builds a development toolchain folder for compiled programs (specification 44 §4.6): the pinned
# rustc and rust-lld of the current rustup toolchain, and the interface and program library crates
# built for each target with the module flags of §4.5. The build finds the folder at
# TRUST_TOOLCHAIN. A release ships the same folder as the toolchain bundle.
#
# Usage: scripts/native/build_toolchain_libraries.sh <folder> [<triple>...]
#   The triples default to x86_64-unknown-linux-gnu and riscv32imac-unknown-none-elf.
set -euo pipefail
out=${1:?usage: build_toolchain_libraries.sh <folder> [<triple>...]}
shift
if [ $# -gt 0 ]; then
  triples=("$@")
else
  triples=(x86_64-unknown-linux-gnu riscv32imac-unknown-none-elf)
fi
root=$(cd "$(dirname "$0")/../.." && pwd)
cd "$root"
sysroot=$(rustc --print sysroot)
host=$(rustc -vV | sed -n 's/^host: //p')
mkdir -p "$out/bin" "$out/lib"
ln -sfn "$sysroot/bin/rustc" "$out/bin/rustc"
ln -sfn "$sysroot/lib/rustlib/$host/bin/rust-lld" "$out/bin/rust-lld"
for triple in "${triples[@]}"; do
  build="$out/build"
  # The module flags of specification 44 §4.5 that the libraries share with the module:
  # position-independent code, abort on panic, and bitcode for the module's link-time optimization.
  RUSTFLAGS="-C relocation-model=pic -C panic=abort -C embed-bitcode=yes -C opt-level=2 -C codegen-units=1" \
    cargo build --quiet --release -p trust-plc-lib --target "$triple" --target-dir "$build"
  libraries="$out/lib/$triple"
  mkdir -p "$libraries"
  rm -f "$libraries"/*.rlib
  cp "$build/$triple/release/deps/"libtrust_plc_abi-*.rlib \
     "$build/$triple/release/deps/"libtrust_plc_lib-*.rlib \
     "$build/$triple/release/deps/"liblibm-*.rlib "$libraries/"
  echo "$triple: $(ls "$libraries" | tr '\n' ' ')"
done
