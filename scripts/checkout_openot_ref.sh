#!/usr/bin/env bash
set -euo pipefail

dest="${OPEN_OT_REF_DIR:-../open-ot-ref}"
repo="${OPEN_OT_REF_REPOSITORY:-https://github.com/johannesPettersson80/open-ot-experiments.git}"
ref="${OPEN_OT_REF_REF:-137f0e765f085c262651f479be35298b836ac891}"

if [[ -f "${dest}/Cargo.toml" ]]; then
  echo "OpenOT reference already available at ${dest}"
  exit 0
fi

mkdir -p "$(dirname "${dest}")"
tmp="${dest}.tmp.$$"
trap 'rm -rf "${tmp}"' EXIT

if [[ "${ref}" =~ ^[0-9a-fA-F]{40}$ ]]; then
  git clone --depth 1 "${repo}" "${tmp}"
  git -C "${tmp}" fetch --depth 1 origin "${ref}" || true
  git -C "${tmp}" checkout --detach "${ref}"
else
  git clone --depth 1 --branch "${ref}" "${repo}" "${tmp}"
fi

mv "${tmp}" "${dest}"
echo "Checked out OpenOT reference ${ref} at ${dest}"
