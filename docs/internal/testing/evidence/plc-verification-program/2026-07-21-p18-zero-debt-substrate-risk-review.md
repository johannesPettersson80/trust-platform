# Phase 18 zero-debt substrate risk review

## Substrate risk boundary

The Phase 17 Python scanner conservatively marks mutation of local validation
collections, status/report construction, filesystem artifact reads and writes,
and CLI fault rendering as risk-bearing behavior. Every such Phase 18 fact is
bound to its exact reviewed body digest in the risk acknowledgement ledger.

The reviewed contract requires closed schemas, independently supplied source
identity, exact normalized input-closure digests, exact raw-artifact digests,
canonical output, nonzero contract-error exits, and a distinct strict-mode
open-debt exit. The focused negative fixtures remove or corrupt each catcher,
including one proof validator, and require rejection. These acknowledgements
recognize the scanner signals; they do not assert product correctness, waive a
finding, lower a proof denominator, or close any Phase 18 debt class.

## Burn-down ledger risk boundary

The P18-002 scanner conservatively marks construction of canonical rows and
summaries, set/collection partition checks, filesystem reads and artifact
writes, Git subprocess status handling, and CLI fault rendering as risk-bearing
verification behavior. Each acknowledgement is bound to the exact function
body digest and scanner signals. The reviewed contract requires a closed
schema, one slice per authoritative code/test identity, complete proof-budget
and blocker sets, exact source/input closure, canonical ordering, and fully
reconstructed counts. Focused negative fixtures remove identities, duplicate
rows, drift counters, add unknown fields, and weaken budget/environment/blocker
sets. These acknowledgements do not infer product semantics, close a debt row,
or permit omission, grouping, waiver, or denominator reduction.
