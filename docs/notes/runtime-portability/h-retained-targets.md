# Installed retained management targets

Source-only H correction; native assertions are authored and unrun.

The installed engine stores retained declarations in ordinary globals/program
instances. The remaining cycle `Retain` target cases still indexed the legacy
retain bank, so valid control-protocol retained writes/forces could not reach
those declarations. DAP had separately translated them to normal destinations.

Immutable metadata now owns an insertion-ordered retained-label index. Borrowed
ASCII-insensitive queries do not allocate a normalized key or clone/scan the
presentation list. Exact control eval keeps exact matching. Every resolution
reads the current named program root and member from current storage; no cached
live instance ID crosses replacement. Cycle label lookup is charged to the
existing scan allowance before resolution, then the shared reference writer
checks declared type, constant policy, lifetime, deadline and remaining fuel.
Missing/non-retained/stale/type-invalid targets emit existing rejection events.
No installed operation falls back to a legacy retain bank.

Control queue/force/release acknowledgements, original labels, duplicate target
ordering and delayed application remain unchanged. Control eval and DAP single
lookup use the same metadata index; list projection remains declaration ordered.
Detached inspection fixtures may still explicitly carry the historical bank.

Native coverage includes global/program writes, persistent forces/release,
wrong type/non-retain/missing target refusals, stale detached roots, live root
identity change across replacement, shared lookup fuel, and actual Control
requests followed by engine execution. No metadata-only test is substituted for
these PLC value assertions.
