#!/usr/bin/env bash
set -uo pipefail
source /home/johannes/.cache/trust-portability-br3-setup-run3/environment.sh
[[ ! -e "$B_EVIDENCE/validation-started.txt" ]] || exit 91
[[ -s "$B_EVIDENCE/review-approved.txt" ]] || exit 92
date -u +%FT%TZ > "$B_EVIDENCE/validation-started.txt"
printf 'step\tclass\tstatus\texit\tstart\tend\n' > "$B_EVIDENCE/ledger.tsv"
run_step canonical-parity required sha256sum -c "$B_SETUP/canonical.sha256" || exit 93
run_step firmware-link required bash "$B_SETUP/measure.sh"
link_status=$?
if (( link_status == 0 )); then
 run_step inspect required "$CARGO_TARGET_DIR/debug/xtask" portability inspect "$B_EVIDENCE/measurement/firmware.elf" "$B_EVIDENCE/elf-report.json"
 inspection_status=$?
else
 unrun inspect 'Corrected image did not link'
 inspection_status=1
fi
date -u +%FT%TZ > "$B_EVIDENCE/builder-finished.txt"
(( link_status == 0 && inspection_status == 0 ))
