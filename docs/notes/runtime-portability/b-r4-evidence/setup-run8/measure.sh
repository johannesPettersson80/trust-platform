#!/usr/bin/env bash
set -uo pipefail
source /home/johannes/.cache/trust-portability-br4-setup-run8/environment.sh
[[ -s "$B_EVIDENCE/preparation-finished.txt" && -s "$B_EVIDENCE/review-approved.txt" ]] || exit 90
[[ $(cat "$B_EVIDENCE/review-approved.txt") == $(sha256sum "$B_EVIDENCE/formatted-source-manifest.json" | cut -d' ' -f1) ]] || exit 93
[[ ! -e "$B_EVIDENCE/measurement-started.txt" ]] || exit 91
date -u +%FT%TZ > "$B_EVIDENCE/measurement-started.txt"
B_EVIDENCE="$B_EVIDENCE/measurement"
mkdir -p "$B_EVIDENCE"
printf 'step\texit\n' > "$B_EVIDENCE/ledger.tsv"
if [[ -e firmware/trust-nucleo-f401re/trust-nucleo-f401re.map ]]; then
 mv firmware/trust-nucleo-f401re/trust-nucleo-f401re.map "$B_EVIDENCE/preexisting.map"
fi
bash scripts/with_cargo_target_lease.sh "$CARGO_TARGET_DIR" cargo xtask portability build-firmware "$B_EVIDENCE/build-config.json" > "$B_EVIDENCE/link.txt" 2>&1
result=$?
printf 'canonical-firmware-link\t%s\n' "$result" >> "$B_EVIDENCE/ledger.tsv"
map_result=1
if [[ -s firmware/trust-nucleo-f401re/trust-nucleo-f401re.map ]]; then
 cp firmware/trust-nucleo-f401re/trust-nucleo-f401re.map "$B_EVIDENCE/firmware.map"
 "$CARGO_TARGET_DIR/debug/xtask" portability footprint "$B_EVIDENCE/firmware.map" "$B_SETUP/baseline.map" "$B_EVIDENCE/footprint.json" > "$B_EVIDENCE/footprint.txt" 2>&1
 map_result=$?
 printf 'footprint\t%s\n' "$map_result" >> "$B_EVIDENCE/ledger.tsv"
 sha256sum "$B_EVIDENCE/firmware.map" > "$B_EVIDENCE/map.sha256"
else
 printf 'footprint\tUNRUN-no-map\n' >> "$B_EVIDENCE/ledger.tsv"
fi
if (( result == 0 )); then
 cp "$CARGO_TARGET_DIR/thumbv7em-none-eabihf/release/trust-nucleo-f401re" "$B_EVIDENCE/firmware.elf"
fi
date -u +%FT%TZ > "$B_EVIDENCE/measurement-finished.txt"
if (( map_result != 0 )); then exit 94; fi
exit "$result"
