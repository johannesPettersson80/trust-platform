#!/usr/bin/env bash
set -euo pipefail

dest="${OPEN_OT_REF_DIR:-../open-ot-ref}"
repo="${OPEN_OT_REF_REPOSITORY:-https://github.com/johannesPettersson80/open-ot-experiments.git}"
ref="${OPEN_OT_REF_REF:-cf6135830e7dbb01626e35f6804ed2429b127baa}"

if [[ -f "${dest}/Cargo.toml" ]]; then
  echo "OpenOT reference already available at ${dest}"
  exit 0
fi

mkdir -p "$(dirname "${dest}")"
tmp="${dest}.tmp.$$"
trap 'rm -rf "${tmp}"' EXIT

# A transient answer of GitHub's fails a clone that passes a moment later: one 403 failed main's
# Documentation job on 18 November 2026 (specification 22, "CI reliability"). The clone is tried up
# to three times, after pauses of 15 and 30 seconds, each time into an empty folder.
clone() {
  local attempt
  for attempt in 1 2 3; do
    rm -rf "${tmp}"
    if git clone --depth 1 "$@" "${repo}" "${tmp}"; then
      return 0
    fi
    if [[ ${attempt} -lt 3 ]]; then
      echo "The clone of ${repo} failed; trying again in $((attempt * 15)) seconds" >&2
      sleep $((attempt * 15))
    fi
  done
  return 1
}

if [[ "${ref}" =~ ^[0-9a-fA-F]{40}$ ]]; then
  clone
  git -C "${tmp}" fetch --depth 1 origin "${ref}" || true
  git -C "${tmp}" checkout --detach "${ref}"
else
  clone --branch "${ref}"
fi

mv "${tmp}" "${dest}"
echo "Checked out OpenOT reference ${ref} at ${dest}"
