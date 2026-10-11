# B-R4 formatted source reconciliation

Accepted by `/root/br1_static_registry` for the single authorized validation batch. Reviewed all formatting diffs affecting the 13 call-continuation files and four firmware files pinned in the prior independent reviews. Every retained before-format source matches its prior accepted SHA; every final source matches the 1011-record frozen manifest `ec2cc5372aa76a691df42dc88035de28f7aa9ee38e676bf5f44d38b678d5e0b9`. The changes are formatting, import ordering, expanded match arms and optional trailing commas only. No semantic correction or test weakening was introduced by preparation.

The final start_call occupies lines 48 through 185 (138 lines including signature), below the architecture policy's 200-line threshold. No function-size waiver is needed for it. The initializer early-entry ownership correction and its rejection/reuse assertions remain present; firmware stack limits and failure guard logic remain intact.

Final 17-file review identity: `f53030839b1e6d4c4af0d1d47ba9e3c26bd5c33f934603b85fea2ee41b3dd180`. This is source acceptance only, not compilation, native-test or physical-board evidence. No build, test, formatter, linker or board command was run by this reviewer. Root separately reconciles the initializer-finalization slice authored by this reviewer.
