#!/usr/bin/env bash
set -euo pipefail

dest="${OPEN_OT_REF_DIR:-../open-ot-ref}"
repo="${OPEN_OT_REF_REPOSITORY:-https://github.com/johannesPettersson80/open-ot-experiments.git}"
ref="${OPEN_OT_REF_REF:-main}"

if [[ -f "${dest}/Cargo.toml" ]]; then
  echo "OpenOT reference already available at ${dest}"
  exit 0
fi

mkdir -p "$(dirname "${dest}")"
git clone --depth 1 --branch "${ref}" "${repo}" "${dest}"
echo "Checked out OpenOT reference ${ref} at ${dest}"
