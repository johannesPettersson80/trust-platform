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
  # position-independent code, abort on panic, bitcode for the module's link-time optimization,
  # and no RELRO, so that the memory functions are called directly, not through a global offset
  # table (the flag travels in the bitcode).
  # A computer's module links no C library, so it takes the memory functions from
  # trust-plc-mem; a chip's target builtins have them.
  packages=(-p trust-plc-lib)
  crates=(trust_plc_abi trust_plc_lib libm ryu)
  case $triple in
    *-none-*) ;;
    *) packages+=(-p trust-plc-mem); crates+=(trust_plc_mem) ;;
  esac
  RUSTFLAGS="-C relocation-model=pic -C panic=abort -C embed-bitcode=yes -C opt-level=2 -C codegen-units=1 -C relro-level=off" \
    cargo build --quiet --release "${packages[@]}" --target "$triple" --target-dir "$build"
  libraries="$out/lib/$triple"
  mkdir -p "$libraries"
  rm -f "$libraries"/*.rlib
  # The newest build of each crate the library links, so that an older build left in the
  # target folder is no second candidate.
  for crate in "${crates[@]}"; do
    newest=$(ls -t "$build/$triple/release/deps/"lib"$crate"-*.rlib | head -1)
    cp "$newest" "$libraries/"
  done
  # The program library the module links, which the image's header names by a hash and the
  # kernel of the same release admits (specification 44 §5.2, §10.2).
  version=$(cargo pkgid -p trust-plc-lib | sed 's/.*[#@]//')
  echo "trust-plc-lib $version" > "$libraries/LIBRARY"
  echo "$triple: $(ls "$libraries" | tr '\n' ' ')"
done
