#!/usr/bin/env bash
# Builds a development toolchain folder for compiled programs (specification 44 §4.6): the pinned
# rustc and rust-lld of the current rustup toolchain, and the interface and program library crates
# built for each target with the module flags of §4.5. The build finds the folder at
# TRUST_TOOLCHAIN. `trust-runtime toolchain bundle <folder> <archive>` turns the folder into the
# toolchain bundle a release ships (§4.6).
#
# Usage: scripts/native/build_toolchain_libraries.sh <folder> [<triple>...]
#   The triples default to every module target of specification 44 §4.4, as a bundle holds them.
set -euo pipefail
out=${1:?usage: build_toolchain_libraries.sh <folder> [<triple>...]}
shift
if [ $# -gt 0 ]; then
  triples=("$@")
else
  triples=(riscv32imac-unknown-none-elf thumbv8m.main-none-eabihf x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu)
fi
root=$(cd "$(dirname "$0")/../.." && pwd)
cd "$root"
sysroot=$(rustc --print sysroot)
host=$(rustc -vV | sed -n 's/^host: //p')
mkdir -p "$out/lib"
# The compiler, linked where the build looks for it. Not on Windows: rustc.exe runs only beside its
# own libraries, so a Windows computer takes the bundle `trust-runtime toolchain bundle` writes from
# this folder and `trust-runtime toolchain install --from` installs (§4.6).
case $host in
  *-windows-*) ;;
  *-apple-darwin)
    # macOS's rust-lld loads LLVM from ../lib beside its own file, so it is copied, not linked,
    # next to the compiler's libraries, as the bundle places them; a cleared environment, as an
    # isolated run has (protocol annex E8.1), then still finds them.
    mkdir -p "$out/bin"
    ln -sfn "$sysroot/bin/rustc" "$out/bin/rustc"
    rm -f "$out/bin/rust-lld"
    cp "$sysroot/lib/rustlib/$host/bin/rust-lld" "$out/bin/rust-lld"
    for library in "$sysroot"/lib/*.dylib; do ln -sfn "$library" "$out/lib/$(basename "$library")"; done
    ;;
  *)
    mkdir -p "$out/bin"
    ln -sfn "$sysroot/bin/rustc" "$out/bin/rustc"
    ln -sfn "$sysroot/lib/rustlib/$host/bin/rust-lld" "$out/bin/rust-lld"
    ;;
esac
# A program whose project has a Rust crate is built by the compiler's cargo against the SDK's
# crates (specification 48 §9.3, RU-12): the folder names this checkout's crates.
case $host in
  *-windows-*) ;;
  *)
    ln -sfn "$sysroot/bin/cargo" "$out/bin/cargo"
    ln -sfn "$root/crates" "$out/sdk"
    # The SDK's procedural macros, built here with the toolchain's own compiler, which the build
    # gives rustc as an external crate so that it compiles and links nothing for the computer
    # (specification 48 RU-74).
    cargo build --quiet --release -p trust-plc-macros --target-dir "$out/build-macros"
    case $host in
      *-apple-darwin) library=libtrust_plc_macros.dylib ;;
      *) library=libtrust_plc_macros.so ;;
    esac
    mkdir -p "$out/macros"
    rm -f "$out/macros/$library"
    cp "$out/build-macros/release/$library" "$out/macros/$library"
    ;;
esac
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
  flags="-C relocation-model=pic -C panic=abort -C embed-bitcode=yes -C opt-level=2 -C codegen-units=1 -C relro-level=off"
  # The AArch64 module reserves x18, which macOS reserves and may overwrite when it switches
  # threads, and makes its atomic operations its own instructions, as the compiler's outline atomics
  # call the C library's getauxval, which a freestanding module cannot (§4.4). Link-time
  # optimization keeps each function's own target features, so the libraries are built with the
  # module's: a library function that kept a value in x18 computed a wrong value or crashed at
  # random on macOS.
  case $triple in
    aarch64-*) flags="$flags -C target-feature=+reserve-x18,-outline-atomics" ;;
  esac
  RUSTFLAGS="$flags" \
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
# The crates.io crates of the libraries above that have a build script, today libm, for the build of
# a program whose project has a Rust crate (specification 48 RU-74): `vendor/` holds a copy of each,
# whose manifest gains a `links` key, and `build-scripts.toml`, the output each script printed above
# for each module target, as cargo's build script overrides of that key. The build takes each crate
# from its copy and gives cargo the overrides, so it compiles and runs no build script on the
# computer. Not on Windows, whose folder has no cargo (above).
# The values of the directive $1 in the build script output $2, as TOML strings.
directive() {
  sed -n "s/^cargo::\{0,1\}$1=//p" "$2" | sed 's/\\/\\\\/g; s/"/\\"/g; s/.*/"&"/' | paste -s -d, -
}
# The variables the build script output $1 sets, as the entries of a TOML table.
environment() {
  sed -n 's/^cargo::\{0,1\}rustc-env=//p' "$1" \
    | sed 's/\\/\\\\/g; s/"/\\"/g; s/^\([^=]*\)=\(.*\)$/\1 = "\2"/' | paste -s -d, -
}
case $host in
  *-windows-*) ;;
  *)
    rm -rf "${out:?}/vendor"
    mkdir -p "$out/vendor"
    overrides="$out/vendor/build-scripts.toml"
    echo "# The output of the SDK's build scripts for each module target (specification 48 RU-74)." > "$overrides"
    for crate in libm; do
      version=$(cargo pkgid -p "$crate" | sed 's/.*[#@]//')
      source=$(ls -d "${CARGO_HOME:-$HOME/.cargo}/registry/src/"*/"$crate-$version" | head -1)
      if grep -q '^links' "$source/Cargo.toml"; then
        echo "$crate names a native library with a links key of its own" >&2
        exit 1
      fi
      cp -R "$source" "$out/vendor/$crate"
      awk -v links="trust_$crate" '{ print } /^\[package\]$/ { print "links = \"" links "\"" }' \
        "$source/Cargo.toml" > "$out/vendor/$crate/Cargo.toml"
      for triple in "${triples[@]}"; do
        output=$(ls -t "$out/build/$triple/release/build/$crate"-*/output | head -1)
        # A module links no library of the computer's (specification 44 §4.5).
        if grep -Eq '^cargo::?rustc-(link|flags|cdylib)' "$output"; then
          echo "$crate's build script links a library for $triple" >&2
          exit 1
        fi
        {
          echo "[target.$triple.trust_$crate]"
          echo "rustc-cfg = [$(directive rustc-cfg "$output")]"
          echo "rustc-check-cfg = [$(directive rustc-check-cfg "$output")]"
          echo "rustc-env = {$(environment "$output")}"
        } >> "$overrides"
      done
    done
    ;;
esac
