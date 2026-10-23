# Adapted board script acceptance

Independently accepted source by `/root/br1_static_registry`; exact board-batch.sh SHA 31ea2c3a7bc151d6fbbd1843829bef486f2993f5e6f8b8803385287974ccec6c. No script or board operation executed.

The reviewed OpenOCD commands are inserted after retained firmware/application/checkpoint dumps and before install shutdown. RCC reset flags were already logged earlier. PWREN and DBP are set using masked writes, read back and explicitly checked before cookie writes. The post-DBP read provides the required ordering barrier. The two previous words are logged; BKP1R is invalidated before BKP0R is cleared; a nonzero final pair aborts the script. No backup-domain reset, RTC calendar unlock or unrelated register clearing was added. Existing flash/checkpoint comparisons and all-PASS prerequisites remain before the subsequent bounded UART acquisition and only run reset.

The initial adaptation printed access readbacks but omitted fail checks. Those checks are now present, avoiding false clearance claims after inaccessible zero reads. This accepted script supersedes only the board-batch.sh hash in review-run2-scripts; all other reviewed setup inputs remain unchanged.
