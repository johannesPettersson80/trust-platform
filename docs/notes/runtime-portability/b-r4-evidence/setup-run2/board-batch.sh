#!/usr/bin/env bash
set -euo pipefail
E=/home/johannes/projects/.artifacts/runtime-portability-b/b-r4/run2/board
SERIAL=0671FF575755846687183960
PORT=/dev/serial/by-id/usb-STMicroelectronics_STM32_STLink_0671FF575755846687183960-if02
cd "$E"
if test -e board-started.txt; then echo 'Refusing automatic repeat'; exit 1; fi
test -s firmware.elf && test -s application.bin && test -s elf-report.json && test -s software-approved.txt
date -u +%FT%TZ > board-started.txt
# The linked ELF has already passed the Rust partition check. Preserve existing
# firmware and option/reset observations before changing the development board.
timeout 90s openocd -f board/st_nucleo_f4.cfg -c "adapter serial $SERIAL" -c init -c 'reset init' \
 -c "dump_image $E/preflash-full.bin 0x08000000 0x80000" \
 -c 'echo FLASH_OPTCR=[read_memory 0x40023c14 32 1]' \
 -c 'echo RCC_CSR=[read_memory 0x40023874 32 1]' \
 -c 'flash erase_sector 0 0 0' -c 'flash erase_sector 0 4 7' \
 -c "flash write_image $E/firmware.elf" -c "verify_image $E/firmware.elf" \
 -c "dump_image $E/firmware-sector0-before-app.bin 0x08000000 0x4000" \
 -c "dump_image $E/firmware-upper-before-app.bin 0x08010000 0x70000" \
 -c 'flash erase_sector 0 1 1' \
 -c "flash write_image $E/application.bin 0x08004000 bin" \
 -c "verify_image $E/application.bin 0x08004000 bin" \
 -c "dump_image $E/installed-application-sector.bin 0x08004000 0x4000" \
 -c "dump_image $E/firmware-sector0-after-app.bin 0x08000000 0x4000" \
 -c "dump_image $E/firmware-upper-after-app.bin 0x08010000 0x70000" \
 -c "dump_image $E/checkpoint-after-install.bin 0x08008000 0x8000" \
 -c 'mmw 0x40023840 0x10000000 0' \
 -c 'echo REINSTALL_RCC_APB1ENR=[read_memory 0x40023840 32 1]' \
 -c 'if {([lindex [read_memory 0x40023840 32 1] 0] & 0x10000000) == 0} { error "PWR clock enable failed" }' \
 -c 'mmw 0x40007000 0x100 0' \
 -c 'echo REINSTALL_PWR_CR=[read_memory 0x40007000 32 1]' \
 -c 'if {([lindex [read_memory 0x40007000 32 1] 0] & 0x100) == 0} { error "backup write enable failed" }' \
 -c 'echo PREVIOUS_BKP0R=[read_memory 0x40002850 32 1]' \
 -c 'echo PREVIOUS_BKP1R=[read_memory 0x40002854 32 1]' \
 -c 'mww 0x40002854 0' -c 'mww 0x40002850 0' \
 -c 'if {[lindex [read_memory 0x40002854 32 1] 0] != 0 || [lindex [read_memory 0x40002850 32 1] 0] != 0} { error "reinstall cookie clear failed" }' \
 -c 'echo REINSTALL_COOKIE_CLEARED=1' \
 -c shutdown > openocd-install.txt 2>&1
# The explicit reinstall clears only the recorded two-word firmware phase marker.
# PWR_CR readback after DBP follows RM0368 section 5.4.1; no backup-domain reset.
# Exact byte preservation across the separate application write and all installation.
cmp firmware-sector0-before-app.bin firmware-sector0-after-app.bin
cmp firmware-upper-before-app.bin firmware-upper-after-app.bin
dd if=preflash-full.bin of=checkpoint-before-install.bin bs=16384 skip=2 count=2 status=none
cmp checkpoint-before-install.bin checkpoint-after-install.bin
sha256sum preflash-full.bin firmware.elf application.bin installed-application-sector.bin > installed-sha256.txt
# Capture starts while the CPU is halted, before the only commanded run reset.
stty -F "$PORT" 230400 raw -echo -ixon -ixoff -crtscts
# Archive stale bytes separately while firmware is halted; none enter the oracle.
set +e
timeout 1s cat "$PORT" > uart-before-reset.txt 2> uart-drain-stderr.txt
drain_status=$?
set -e
test "$drain_status" = 124
set +e
timeout 25s cat "$PORT" > uart.txt 2> uart-capture-stderr.txt &
capture_pid=$!
set -e
trap 'kill "$capture_pid" 2>/dev/null || true' EXIT
timeout 90s openocd -f board/st_nucleo_f4.cfg -c "adapter serial $SERIAL" -c init -c 'reset run' -c shutdown > openocd-start.txt 2>&1
set +e
wait "$capture_pid"
capture_status=$?
set -e
trap - EXIT
printf '%s\n' "$capture_status" > capture-exit.txt
# cat timeout is the planned bounded acquisition, not a firmware pass/fail.
test "$capture_status" = 124
sha256sum uart.txt > uart-sha256.txt
date -u +%FT%TZ > board-finished.txt
