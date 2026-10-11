#!/usr/bin/env bash
set -uo pipefail
source /home/johannes/.cache/trust-portability-br4-setup-run5/environment.sh
[[ -s "$B_EVIDENCE/preparation-finished.txt" && -s "$B_EVIDENCE/review-approved.txt" ]] || exit 93
[[ $(cat "$B_EVIDENCE/review-approved.txt") == $(sha256sum "$B_EVIDENCE/formatted-source-manifest.json" | cut -d' ' -f1) ]] || exit 93
run_step firmware-link required bash "$B_SETUP/measure.sh"
result=$?
if [[ -s "$B_EVIDENCE/measurement/firmware.elf" ]]; then
 run_step inspect required "$CARGO_TARGET_DIR/debug/xtask" portability inspect "$B_EVIDENCE/measurement/firmware.elf" "$B_EVIDENCE/elf-report.json"
 inspection=$?
else
 unrun inspect 'No linked candidate'
 inspection=1
fi
date -u +%FT%TZ > "$B_EVIDENCE/measurement-stage-finished.txt"
(( result == 0 && inspection == 0 ))
