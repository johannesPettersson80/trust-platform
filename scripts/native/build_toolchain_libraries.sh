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
# crates (specification 48 §9.3, RU-12): `sdk/` holds this checkout's SDK crates as `cargo package`
# writes them for crates.io, so that the build takes them outside this workspace (RU-74).
case $host in
  *-windows-*) ;;
  *)
    ln -sfn "$sysroot/bin/cargo" "$out/bin/cargo"
    # The crates of the lock, which the steps below read offline: on a fresh computer, as the
    # release's, this script runs cargo first.
    cargo fetch --quiet --locked
    rm -rf "${out:?}/sdk" "${out:?}/build-package"
    cargo package --quiet --offline --no-verify --allow-dirty -p trust-plc-abi -p trust-plc-lib \
      -p trust-plc-mem -p trust-plc-decl -p trust-plc-macros -p trust-plc --target-dir "$out/build-package"
    mkdir -p "$out/sdk"
    for package in "$out/build-package/package/"*.crate; do
      tar -xzf "$package" -C "$out/sdk"
    done
    # Each crate's folder by its name alone: trust-plc-0.41.2 becomes trust-plc.
    for folder in "$out/sdk/"*-[0-9]*; do
      mv "$folder" "${folder%-[0-9]*}"
    done
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
# The crates.io crates the SDK's crates are built from, the attributes' own among them, for the
# build of a program whose project has a Rust crate (specification 48 RU-74): `vendor/` holds a copy
# of each, which the build takes instead of a registry's, so that its cargo, whose home is its own
# (RU-75), needs no registry. A crate whose build script ran for the module targets above, today
# libm, gains a `links` key in its copy's manifest, and `build-scripts.toml` holds the output the
# script printed for each module target, as cargo's build script overrides of that key: the build
# gives cargo the overrides, so it compiles and runs no build script on the computer. Not on
# Windows, whose folder has no cargo (above).
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
    # `name vX.Y.Z` of each crates.io crate of the SDK's crate graph: a path crate names its folder,
    # and an empty line parts the trees of the two packages.
    cargo tree --offline -p trust-plc -p trust-plc-mem -e normal,build --prefix none --format '{p}' \
      | grep -v '(/' | sed 's/ (.*)$//' | awk 'NF == 2' | sort -u > "$out/build/registry-crates.txt"
    while read -r crate version; do
      source=$(ls -d "${CARGO_HOME:-$HOME/.cargo}/registry/src/"*/"$crate-${version#v}" | head -1)
      cp -R "$source" "$out/vendor/$crate"
      # Only a crate whose build script ran for the module targets has its output among their builds.
      ls "$out/build/${triples[0]}/release/build/$crate"-*/output > /dev/null 2>&1 || continue
      if grep -q '^links' "$source/Cargo.toml"; then
        echo "$crate names a native library with a links key of its own" >&2
        exit 1
      fi
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
    done < "$out/build/registry-crates.txt"
    # The SDK's procedural macros, built with the toolchain's own compiler from the SDK's crates
    # and the crates they are built from as the bundle carries them, which the build gives rustc as
    # an external crate so that it compiles and links nothing for the computer (RU-74): a package
    # of the folder's own depends on trust-plc-macros, its graph patched to `sdk/` and `vendor/`,
    # so that no registry is reached. The bundle's compiler loads the library, so a Linux bundle's
    # must load on every Linux the packages support, glibc 2.28 or newer (specification 24): the
    # release names in TRUST_MACROS_IMAGE a container image of this computer's architecture with
    # that glibc and a C compiler, in which the library is built. Cargo builds a procedural macro
    # for the computer it runs on, so each platform builds its own.
    case $host in
      *-apple-darwin) library=libtrust_plc_macros.dylib ;;
      *) library=libtrust_plc_macros.so ;;
    esac
    rm -rf "${out:?}/build-macros"
    mkdir -p "$out/build-macros/source"
    : > "$out/build-macros/source/lib.rs"
    {
      printf '[package]\nname = "trust-macros-build"\nversion = "0.0.0"\nedition = "2024"\npublish = false\n\n'
      printf '[lib]\npath = "lib.rs"\n\n[dependencies]\n'
      printf 'trust-plc-macros = { path = "../../sdk/trust-plc-macros" }\n\n[patch.crates-io]\n'
      for crate in "$out/sdk/"*/ "$out/vendor/"*/; do
        crate=${crate%/}
        printf '%s = { path = "../../%s/%s" }\n' "$(basename "$crate")" \
          "$(basename "$(dirname "$crate")")" "$(basename "$crate")"
      done
      printf '\n[workspace]\n'
    } > "$out/build-macros/source/Cargo.toml"
    if [ -n "${TRUST_MACROS_IMAGE:-}" ]; then
      docker run --rm --user "$(id -u):$(id -g)" --volume "$(cd "$out" && pwd):/toolchain" \
        --volume "$sysroot:/rust:ro" --workdir /toolchain/build-macros/source \
        --env RUSTC=/rust/bin/rustc --env HOME=/tmp --env CARGO_HOME=/tmp/cargo "$TRUST_MACROS_IMAGE" \
        /rust/bin/cargo build --quiet --release --offline -p trust-plc-macros --target-dir ../target
    else
      (cd "$out/build-macros/source" && RUSTC="$sysroot/bin/rustc" "$sysroot/bin/cargo" build \
        --quiet --release --offline -p trust-plc-macros --target-dir ../target)
    fi
    mkdir -p "$out/macros"
    rm -f "$out/macros/$library"
    cp "$out/build-macros/target/release/$library" "$out/macros/$library"
    ;;
esac
