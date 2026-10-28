# Equipment library (retired)

Retired on 7 October 2026, before any release shipped it. The equipment, instruments,
controllers, groups and sequences are specified by the [truST Plant library](trust-plant-library.md),
a native object-oriented library with one implementation per block. Its device behaviour rules
(modes, interlocks and permissives, feedback supervision, safe state, counters and restart) carry
over from this specification in object-oriented form. The `Equipment` namespace, its compatibility
copy inside the Plant package, and its faceplates and pairs are removed.
