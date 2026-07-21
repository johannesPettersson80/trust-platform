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
