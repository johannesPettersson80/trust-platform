#!/usr/bin/env bash
set -euo pipefail

if [[ "$(uname -s)" == "Linux" ]] && command -v mold >/dev/null 2>&1; then
  case " ${RUSTFLAGS:-} " in
    *" -C link-arg=-fuse-ld="*) ;;
    *)
      export RUSTFLAGS="${RUSTFLAGS:-} -C link-arg=-fuse-ld=mold"
      echo "cargo_test_fast_link: using mold for Rust test linking" >&2
      ;;
  esac
fi

exec cargo "$@"
