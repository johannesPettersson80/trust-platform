# Reviewed proposal only: not executed by the reviewer.
# Apply after reset init, with prior failure evidence already retained.
echo RCC_CSR_BEFORE_COOKIE_CLEAR=[read_memory 0x40023874 32 1]
echo RCC_APB1ENR_BEFORE_COOKIE_CLEAR=[read_memory 0x40023840 32 1]
mmw 0x40023840 0x10000000 0
set cookie_pwr_clock [lindex [read_memory 0x40023840 32 1] 0]
if {($cookie_pwr_clock & 0x10000000) == 0} { error "PWR clock did not enable" }
echo PWR_CR_BEFORE_COOKIE_CLEAR=[read_memory 0x40007000 32 1]
mmw 0x40007000 0x100 0
set cookie_pwr_access [lindex [read_memory 0x40007000 32 1] 0]
if {($cookie_pwr_access & 0x100) == 0} { error "Backup write access did not enable" }
echo RTC_COOKIE_BEFORE_CLEAR=[read_memory 0x40002850 32 2]
mww 0x40002854 0
mww 0x40002850 0
set cookie_after [read_memory 0x40002850 32 2]
echo RTC_COOKIE_AFTER_CLEAR=$cookie_after
if {[lindex $cookie_after 0] != 0 || [lindex $cookie_after 1] != 0} { error "Backup cookie did not clear" }
