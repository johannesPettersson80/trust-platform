# B-R4 state allocation phase split independent review

Accepted by `/root/br1_static_registry`, source only. One-file identity 676938e31137035957ffec8b693300d8f2fb17aa8bff674e8ae7500c37f13f44; retained before-source hash matches run5's frozen engine/mod.rs. No edit, build, test, formatter, linker or board operation performed by reviewer.

The private non-inlined allocate_resource_state contains the old build prefix unchanged: service validation, resource lookup/clone, identical EngineState fields, entry deadline, checked doubled frame capacity, allocation/work charges and reservations. It returns before construct_resource executes any nested initialization. Existing build then invokes construct_resource(retained), checks completion deadline and returns exactly as before. No new allocation, Box, profile setting, field, limit or public API is introduced.

Fresh new and restart both continue calling the same build. Retained state remains immutable through allocation and enters construction at the same point; candidate publication/replacement remains in restart after successful build. Errors during shallow reservation drop the partial candidate; errors during deep construction or completion drop the unpublished fully reserved candidate. Moving the owned state between private functions does not clone its contents or change the charge sequence. Existing native initialization/restart/failure corpus remains applicable; no assertion changed.

The explicit boundary targets measured aggregate-construction temporaries previously retained in the1824-byte instantiate frame across nested callbacks. No native-frame reduction, flash saving or physical headroom is claimed before measurement. Final source freeze and linked-path review remain necessary.
